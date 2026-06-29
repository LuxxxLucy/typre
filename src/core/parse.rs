use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::commands::{parse_command, Frag};
use crate::core::ir::{Align, Block, Deck, Inline, Meta, Slide, Style};

pub fn parse(md: &str) -> Deck {
    let (md, frags) = extract_typst(md);
    let md = md.as_str();
    let opts = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS;

    let mut meta = Meta::default();
    let mut slides: Vec<Slide> = vec![Slide::default()];

    // Container stack: nested block sequences (slide root + list items).
    let mut block_stack: Vec<Vec<Block>> = vec![Vec::new()];
    // Pending inline accumulator for the current leaf (heading/paragraph/item-paragraph).
    let mut inlines: Vec<Inline> = Vec::new();
    let mut style = Style::default();
    let mut list_ordered: Vec<bool> = Vec::new();
    let mut heading_level: Option<u8> = None;
    let mut in_code = false;
    let mut code_buf = String::new();
    let mut code_lang: Option<String> = None;
    let mut in_metadata = false;
    let mut meta_buf = String::new();
    let mut pending_image: Option<(String, String)> = None;
    let mut in_link: Option<(String, String)> = None;
    let mut table: Option<TableBuilder> = None;

    for ev in Parser::new_ext(md, opts) {
        match ev {
            Event::Start(Tag::MetadataBlock(_)) => in_metadata = true,
            Event::End(TagEnd::MetadataBlock(_)) => {
                in_metadata = false;
                extract_meta(&meta_buf, &mut meta);
            }

            Event::Start(Tag::HtmlBlock) | Event::Start(Tag::Paragraph) if in_metadata => {}

            Event::Start(Tag::Heading { level, .. }) => {
                let lvl = heading_to_u8(level);
                // H1/H2 at the slide root begin a new slide (a normal markdown doc paginates by section)
                if lvl <= 2 && block_stack.len() == 1 && !block_stack[0].is_empty() {
                    flush_slide(&mut slides, &mut block_stack);
                }
                heading_level = Some(lvl);
                inlines = Vec::new();
            }
            Event::End(TagEnd::Heading(_)) => {
                let lvl = heading_level.take().unwrap_or(1);
                push_block(
                    &mut block_stack,
                    Block::Heading(lvl, std::mem::take(&mut inlines)),
                );
            }

            Event::Start(Tag::Paragraph) => {
                inlines = Vec::new();
            }
            Event::End(TagEnd::Paragraph) => {
                let mut inl = std::mem::take(&mut inlines);
                // A paragraph that is a single ◊ fragment becomes its block: typst
                // turns into block math, a structured command into its own block.
                if inl.len() == 1 {
                    match inl.pop().unwrap() {
                        Inline::InlineTypst { src, width } => {
                            push_block(&mut block_stack, Block::BlockTypst { src, width })
                        }
                        Inline::BlockFragment(b) => push_block(&mut block_stack, *b),
                        other => push_block(&mut block_stack, Block::Paragraph(vec![other])),
                    }
                } else if !inl.is_empty() {
                    push_block(&mut block_stack, Block::Paragraph(inl));
                }
            }

            Event::Start(Tag::List(start)) => {
                // Flush a tight item's own text before descending into its nested list.
                let inl = std::mem::take(&mut inlines);
                if !inl.is_empty() {
                    push_block(&mut block_stack, Block::Paragraph(inl));
                }
                list_ordered.push(start.is_some());
            }
            Event::End(TagEnd::List(_)) => {
                list_ordered.pop();
            }
            Event::Start(Tag::Item) => block_stack.push(Vec::new()),
            Event::End(TagEnd::Item) => {
                // Flush accumulated inlines: tight items carry no Paragraph wrapper.
                let inl = std::mem::take(&mut inlines);
                if !inl.is_empty() {
                    push_block(&mut block_stack, Block::Paragraph(inl));
                }
                let item = block_stack.pop().unwrap();
                let ordered = *list_ordered.last().unwrap_or(&false);
                let blocks = block_stack.last_mut().unwrap();
                match blocks.last_mut() {
                    Some(Block::List { ordered: o, items }) if *o == ordered => items.push(item),
                    _ => blocks.push(Block::List {
                        ordered,
                        items: vec![item],
                    }),
                }
            }

            Event::Start(Tag::CodeBlock(kind)) => {
                in_code = true;
                code_buf.clear();
                code_lang = match kind {
                    CodeBlockKind::Fenced(info) => info
                        .split_whitespace()
                        .next()
                        .filter(|s| !s.is_empty())
                        .map(str::to_string),
                    CodeBlockKind::Indented => None,
                };
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code = false;
                let mut src = std::mem::take(&mut code_buf);
                if src.ends_with('\n') {
                    src.pop();
                }
                let lang = code_lang.take();
                push_block(&mut block_stack, Block::Code { src, lang });
            }

            Event::Start(Tag::Image { dest_url, .. }) => {
                pending_image = Some((dest_url.to_string(), String::new()));
            }
            Event::End(TagEnd::Image) => {
                if let Some((src, alt)) = pending_image.take() {
                    // Image is its own block; drop it from the paragraph inline stream.
                    push_block(&mut block_stack, Block::Image { src, alt });
                    inlines.clear();
                }
            }

            Event::Start(Tag::Strong) => style.bold = true,
            Event::End(TagEnd::Strong) => style.bold = false,
            Event::Start(Tag::Emphasis) => style.italic = true,
            Event::End(TagEnd::Emphasis) => style.italic = false,

            Event::Start(Tag::Link { dest_url, .. }) => {
                in_link = Some((dest_url.to_string(), String::new()));
            }
            Event::End(TagEnd::Link) => {
                if let Some((url, label)) = in_link.take() {
                    inlines.push(Inline::Link { label, url });
                }
            }

            Event::Start(Tag::Table(aligns)) => table = Some(TableBuilder::new(aligns)),
            Event::End(TagEnd::Table) => {
                if let Some(tb) = table.take() {
                    push_block(&mut block_stack, tb.finish());
                }
            }
            Event::Start(Tag::TableHead) => {
                if let Some(tb) = table.as_mut() {
                    tb.in_head = true;
                }
            }
            Event::End(TagEnd::TableHead) => {
                if let Some(tb) = table.as_mut() {
                    tb.end_row();
                    tb.in_head = false;
                }
            }
            Event::End(TagEnd::TableRow) => {
                if let Some(tb) = table.as_mut() {
                    tb.end_row();
                }
            }
            Event::Start(Tag::TableCell) => inlines = Vec::new(),
            Event::End(TagEnd::TableCell) => {
                if let Some(tb) = table.as_mut() {
                    tb.row.push(std::mem::take(&mut inlines));
                }
            }

            Event::Start(Tag::BlockQuote(_)) => block_stack.push(Vec::new()),
            Event::End(TagEnd::BlockQuote(_)) => {
                let inner = block_stack.pop().unwrap_or_default();
                push_block(&mut block_stack, Block::Quote(inner));
            }

            Event::Rule => push_block(&mut block_stack, Block::Rule),

            Event::Text(t) => {
                if in_metadata {
                    meta_buf.push_str(&t);
                } else if in_code {
                    code_buf.push_str(&t);
                } else if pending_image.is_some() {
                    if let Some((_, alt)) = pending_image.as_mut() {
                        alt.push_str(&t);
                    }
                } else if let Some((_, label)) = in_link.as_mut() {
                    label.push_str(&t);
                } else {
                    push_text(&mut inlines, &t, style, &frags);
                }
            }
            Event::Code(t) => inlines.push(Inline::Code(t.to_string())),
            Event::SoftBreak => inlines.push(Inline::SoftBreak),
            Event::HardBreak => inlines.push(Inline::HardBreak),

            _ => {}
        }
    }

    let root = block_stack.pop().unwrap_or_default();
    if let Some(last) = slides.last_mut() {
        last.blocks = root;
    }
    slides.retain(|s| !s.blocks.is_empty());
    if slides.is_empty() {
        slides.push(Slide::default());
    }

    Deck { meta, slides }
}

fn flush_slide(slides: &mut Vec<Slide>, block_stack: &mut Vec<Vec<Block>>) {
    let root = std::mem::take(&mut block_stack[0]);
    slides.last_mut().unwrap().blocks = root;
    slides.push(Slide::default());
}

fn push_block(stack: &mut [Vec<Block>], block: Block) {
    stack.last_mut().unwrap().push(block);
}

struct TableBuilder {
    aligns: Vec<Align>,
    in_head: bool,
    head: Vec<Vec<Inline>>,
    rows: Vec<Vec<Vec<Inline>>>,
    row: Vec<Vec<Inline>>,
}

impl TableBuilder {
    fn new(aligns: Vec<Alignment>) -> Self {
        TableBuilder {
            aligns: aligns.into_iter().map(map_align).collect(),
            in_head: false,
            head: Vec::new(),
            rows: Vec::new(),
            row: Vec::new(),
        }
    }

    fn end_row(&mut self) {
        let row = std::mem::take(&mut self.row);
        if row.is_empty() {
            return;
        }
        if self.in_head {
            self.head = row;
        } else {
            self.rows.push(row);
        }
    }

    fn finish(self) -> Block {
        Block::Table {
            aligns: self.aligns,
            head: self.head,
            rows: self.rows,
        }
    }
}

fn map_align(a: Alignment) -> Align {
    match a {
        Alignment::Right => Align::Right,
        Alignment::Center => Align::Center,
        _ => Align::Left,
    }
}

const SENTINEL: char = '\u{F8FF}';

// Pull `◊name{...}` commands out before markdown parsing (their bodies hold
// markdown-active characters) and leave a sentinel the parser restores. Fenced
// ``` blocks stay literal; command bodies may span lines.
fn extract_typst(md: &str) -> (String, Vec<Frag>) {
    let mut out = String::new();
    let mut frags = Vec::new();
    let mut fenced = false;
    let mut buf = String::new();
    for line in md.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            extract_commands(&buf, &mut out, &mut frags);
            buf.clear();
            fenced = !fenced;
            out.push_str(line);
        } else if fenced {
            out.push_str(line);
        } else {
            buf.push_str(line);
        }
    }
    extract_commands(&buf, &mut out, &mut frags);
    (out, frags)
}

fn extract_commands(text: &str, out: &mut String, frags: &mut Vec<Frag>) {
    let mut rest = text;
    while !rest.is_empty() {
        let tick = rest.find('`');
        let loz = rest.find('◊');
        // Copy an inline-code span verbatim so a documented `◊typst{...}` is not extracted.
        let take_tick = match (tick, loz) {
            (Some(t), Some(l)) => t < l,
            (Some(_), None) => true,
            _ => false,
        };
        if take_tick {
            let t = tick.unwrap();
            out.push_str(&rest[..t]);
            rest = &rest[t + copy_code_span(&rest[t..], out)..];
        } else if let Some(p) = loz {
            out.push_str(&rest[..p]);
            let after = &rest[p + '◊'.len_utf8()..];
            match parse_command(after) {
                Some((frag, consumed)) => {
                    let idx = frags.len();
                    frags.push(frag);
                    out.push(SENTINEL);
                    out.push_str(&idx.to_string());
                    out.push(SENTINEL);
                    rest = &after[consumed..];
                }
                None => {
                    out.push('◊');
                    rest = after;
                }
            }
        } else {
            out.push_str(rest);
            break;
        }
    }
}

// `s` starts with a backtick run; copy the inline-code span (through the matching
// run) verbatim and return its byte length. An unclosed run copies just the run.
fn copy_code_span(s: &str, out: &mut String) -> usize {
    let n = s.bytes().take_while(|&b| b == b'`').count();
    let fence = "`".repeat(n);
    match s[n..].find(&fence) {
        Some(rel) => {
            let end = n + rel + n;
            out.push_str(&s[..end]);
            end
        }
        None => {
            out.push_str(&s[..n]);
            n
        }
    }
}

fn push_text(inlines: &mut Vec<Inline>, t: &str, style: Style, frags: &[Frag]) {
    if !t.contains(SENTINEL) {
        if !t.is_empty() {
            inlines.push(Inline::Text(t.to_string(), style));
        }
        return;
    }
    let mut buf = String::new();
    let mut chars = t.chars().peekable();
    while let Some(c) = chars.next() {
        if c != SENTINEL {
            buf.push(c);
            continue;
        }
        let mut num = String::new();
        while let Some(&d) = chars.peek() {
            chars.next();
            if d == SENTINEL {
                break;
            }
            num.push(d);
        }
        match num.parse::<usize>().ok().and_then(|i| frags.get(i)) {
            Some(frag) => {
                if !buf.is_empty() {
                    inlines.push(Inline::Text(std::mem::take(&mut buf), style));
                }
                inlines.push(match frag {
                    Frag::Inline { src, width } => Inline::InlineTypst {
                        src: src.clone(),
                        width: *width,
                    },
                    Frag::Block(b) => Inline::BlockFragment(Box::new(b.clone())),
                });
            }
            None => {
                buf.push(SENTINEL);
                buf.push_str(&num);
            }
        }
    }
    if !buf.is_empty() {
        inlines.push(Inline::Text(buf, style));
    }
}

fn heading_to_u8(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn extract_meta(yaml: &str, meta: &mut Meta) {
    for line in yaml.lines() {
        let line = line.trim();
        if let Some(v) = line.strip_prefix("title:") {
            meta.title = Some(strip_quotes(v.trim()));
        } else if let Some(v) = line.strip_prefix("author:") {
            meta.author = Some(strip_quotes(v.trim()));
        }
    }
}

fn strip_quotes(s: &str) -> String {
    s.trim_matches(|c| c == '"' || c == '\'').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ir::{Block, Inline, Width};

    const SHOWCASE: &str = include_str!("../../examples/showcase.md");

    #[test]
    fn inline_typst_command() {
        let deck = parse("text ◊typst{a_b^*c*} end");
        let slide = &deck.slides[0];
        let inls = match &slide.blocks[0] {
            Block::Paragraph(i) => i,
            other => panic!("expected paragraph, got {other:?}"),
        };
        assert!(
            inls.iter()
                .any(|i| matches!(i, Inline::InlineTypst { src, .. } if src == "a_b^*c*")),
            "typst body with markdown-active chars survives verbatim"
        );
    }

    #[test]
    fn standalone_typst_is_block() {
        let deck = parse("◊typst{y = 1}\n");
        match &deck.slides[0].blocks[0] {
            Block::BlockTypst { src, width } => {
                assert!(src.contains("y = 1"));
                assert_eq!(*width, Width::Natural);
            }
            other => panic!("expected block typst, got {other:?}"),
        }
    }

    #[test]
    fn width_directive_sizes_block() {
        let deck = parse("◊width[70%]{y = 1}\n");
        match &deck.slides[0].blocks[0] {
            Block::BlockTypst { width, .. } => assert_eq!(*width, Width::Percent(70)),
            other => panic!("expected block typst, got {other:?}"),
        }
        let deck = parse("◊width[60]{y = 1}\n");
        match &deck.slides[0].blocks[0] {
            Block::BlockTypst { width, .. } => assert_eq!(*width, Width::Cols(60)),
            other => panic!("expected block typst, got {other:?}"),
        }
        let deck = parse("◊width[full]{y = 1}\n");
        match &deck.slides[0].blocks[0] {
            Block::BlockTypst { width, .. } => assert_eq!(*width, Width::Percent(100)),
            other => panic!("expected block typst, got {other:?}"),
        }
    }

    #[test]
    fn typst_in_inline_code_is_literal() {
        let deck = parse("the `◊typst{x}` command and a real ◊typst{y}\n");
        let inls = match &deck.slides[0].blocks[0] {
            Block::Paragraph(i) => i,
            other => panic!("expected paragraph, got {other:?}"),
        };
        assert!(
            inls.iter().any(|i| matches!(i, Inline::Code(s) if s == "◊typst{x}")),
            "backticked command stays literal"
        );
        assert!(
            inls.iter().any(|i| matches!(i, Inline::InlineTypst { src, .. } if src == "y")),
            "the real command outside backticks still extracts"
        );
    }

    #[test]
    fn multiline_typst_body() {
        let deck = parse("◊typst{\na = 1\nb = 2\n}\n");
        match &deck.slides[0].blocks[0] {
            Block::BlockTypst { src, .. } => {
                assert!(src.contains("a = 1") && src.contains("b = 2"));
            }
            other => panic!("expected block typst, got {other:?}"),
        }
    }

    #[test]
    fn slide_split_count() {
        let deck = parse(SHOWCASE);
        assert_eq!(deck.slides.len(), 7, "showcase has 7 slides");
    }

    #[test]
    fn front_matter_title_and_author() {
        let deck = parse(SHOWCASE);
        assert_eq!(deck.meta.title.as_deref(), Some("typre showcase"));
        assert_eq!(deck.meta.author.as_deref(), Some("luxxxlucy"));
    }

    #[test]
    fn heading_parsed() {
        let deck = parse("# Title\n\nbody");
        match &deck.slides[0].blocks[0] {
            Block::Heading(1, _) => {}
            other => panic!("expected H1, got {other:?}"),
        }
    }

    #[test]
    fn list_parsed() {
        let deck = parse("- a\n- b\n");
        match &deck.slides[0].blocks[0] {
            Block::List { ordered, items } => {
                assert!(!ordered);
                assert_eq!(items.len(), 2);
                // tight-item text must be captured, not dropped
                assert!(matches!(
                    items[0].as_slice(),
                    [Block::Paragraph(p)] if matches!(p.as_slice(), [Inline::Text(t, _)] if t == "a")
                ));
            }
            other => panic!("expected list, got {other:?}"),
        }
    }

    #[test]
    fn headings_split_slides() {
        let deck = parse("# Title\n\nlead\n\n## A\n\nx\n\n## B\n\ny\n");
        assert_eq!(deck.slides.len(), 3, "H1 title slide + two H2 slides");
        assert!(matches!(deck.slides[0].blocks[0], Block::Heading(1, _)));
        assert!(matches!(deck.slides[1].blocks[0], Block::Heading(2, _)));
    }

    #[test]
    fn blockquote_parsed() {
        let deck = parse("> quoted\n");
        match &deck.slides[0].blocks[0] {
            Block::Quote(inner) => assert!(matches!(inner.as_slice(), [Block::Paragraph(_)])),
            other => panic!("expected quote, got {other:?}"),
        }
    }

    #[test]
    fn code_block_parsed() {
        let deck = parse("```rust\nfn main() {}\n```\n");
        match &deck.slides[0].blocks[0] {
            Block::Code { src, lang } => {
                assert_eq!(src, "fn main() {}");
                assert_eq!(lang.as_deref(), Some("rust"));
            }
            other => panic!("expected code, got {other:?}"),
        }
    }

    #[test]
    fn link_parsed() {
        let deck = parse("see [docs](https://x.test) now");
        let inls = match &deck.slides[0].blocks[0] {
            Block::Paragraph(i) => i,
            other => panic!("expected paragraph, got {other:?}"),
        };
        assert!(inls.iter().any(|i| matches!(
            i,
            Inline::Link { label, url } if label == "docs" && url == "https://x.test"
        )));
    }

    #[test]
    fn table_parsed() {
        let deck = parse("| A | B |\n|:--|--:|\n| 1 | 2 |\n");
        match &deck.slides[0].blocks[0] {
            Block::Table { aligns, head, rows } => {
                assert_eq!(aligns.len(), 2);
                assert_eq!(head.len(), 2);
                assert_eq!(rows.len(), 1);
            }
            other => panic!("expected table, got {other:?}"),
        }
    }

    #[test]
    fn tree_parsed() {
        let deck = parse("◊tree{\nroot\n  a\n    a1\n  b\n}\n");
        match &deck.slides[0].blocks[0] {
            Block::Tree(nodes) => {
                assert_eq!(nodes.len(), 1);
                assert_eq!(nodes[0].label, "root");
                assert_eq!(nodes[0].children.len(), 2);
                assert_eq!(nodes[0].children[0].children.len(), 1);
            }
            other => panic!("expected tree, got {other:?}"),
        }
    }

    #[test]
    fn grid_parsed() {
        let deck = parse("◊grid{\n1\n2\n3\n}\n");
        match &deck.slides[0].blocks[0] {
            Block::Grid(cells) => assert_eq!(cells, &["1", "2", "3"]),
            other => panic!("expected grid, got {other:?}"),
        }
    }

    #[test]
    fn figure_parsed() {
        let deck = parse("◊figure[My caption]{\nart\n}\n");
        match &deck.slides[0].blocks[0] {
            Block::Figure { body, caption } => {
                assert_eq!(body, "art");
                assert_eq!(caption, "My caption");
            }
            other => panic!("expected figure, got {other:?}"),
        }
    }

    #[test]
    fn details_parsed() {
        let deck = parse("◊details[Summary text]{\nbody\n}\n");
        match &deck.slides[0].blocks[0] {
            Block::Details { summary, body } => {
                assert_eq!(summary, "Summary text");
                assert_eq!(body, &["body"]);
            }
            other => panic!("expected details, got {other:?}"),
        }
    }

    #[test]
    fn showcase_has_block_typst_slides() {
        let deck = parse(SHOWCASE);
        let block_typst = deck
            .slides
            .iter()
            .filter(|s| s.blocks.iter().any(|b| matches!(b, Block::BlockTypst { .. })))
            .count();
        assert_eq!(block_typst, 2, "the math slide and the sized-figure slide");
    }
}
