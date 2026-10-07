use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use super::commands::{parse_command, ParsedCommand};
use crate::core::ir::{
    guides, plain_text, Align, ArtLine, ArtPart, Block, Deck, Inline, Meta, Slide, Style, TocEntry,
    Width,
};

pub fn parse(md: &str) -> Deck {
    let (md, commands) = extract_commands_from_markdown(md);
    let md = md.as_str();
    let opts = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS
        | Options::ENABLE_MATH;

    let mut meta = Meta::default();
    let mut slides: Vec<Slide> = vec![Slide::default()];

    let mut block_stack: Vec<Vec<Block>> = vec![Vec::new()];
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
                flush_paragraph(&mut block_stack, &mut inlines);
            }

            Event::Start(Tag::List(start)) => {
                flush_paragraph(&mut block_stack, &mut inlines);
                list_ordered.push(start.is_some());
            }
            Event::End(TagEnd::List(_)) => {
                list_ordered.pop();
            }
            Event::Start(Tag::Item) => block_stack.push(Vec::new()),
            Event::End(TagEnd::Item) => {
                flush_paragraph(&mut block_stack, &mut inlines);
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
                let block = Block::Code {
                    src,
                    lang: code_lang.take(),
                };
                push_block(&mut block_stack, block);
            }

            Event::Start(Tag::Image { dest_url, .. }) => {
                pending_image = Some((dest_url.to_string(), String::new()));
            }
            Event::End(TagEnd::Image) => {
                if let Some((src, alt)) = pending_image.take() {
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
                    push_text(&mut inlines, &t, style, &commands);
                }
            }
            Event::Code(t) => inlines.push(Inline::Code(t.to_string())),
            Event::InlineMath(t) => inlines.push(math_inline(&t, false, style)),
            Event::DisplayMath(t) => inlines.push(math_inline(&t, true, style)),
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

    if slides[0].is_title() {
        slides[0].toc = slides
            .iter()
            .enumerate()
            .filter_map(|(index, slide)| match slide.blocks.first() {
                Some(Block::Heading(2, inlines)) => Some(TocEntry {
                    index,
                    title: plain_text(inlines),
                }),
                _ => None,
            })
            .collect();
    }
    Deck { meta, slides }
}

fn flush_slide(slides: &mut Vec<Slide>, block_stack: &mut [Vec<Block>]) {
    let root = std::mem::take(&mut block_stack[0]);
    slides.last_mut().unwrap().blocks = root;
    slides.push(Slide::default());
}

fn push_block(stack: &mut [Vec<Block>], block: Block) {
    stack.last_mut().unwrap().push(block);
}

fn flush_paragraph(stack: &mut [Vec<Block>], inlines: &mut Vec<Inline>) {
    fn flush_inline_run(stack: &mut [Vec<Block>], run: &mut Vec<Inline>) {
        if run.len() == 1 && matches!(run[0], Inline::InlineTypst { display: true, .. }) {
            if let Inline::InlineTypst { src, width, .. } = run.pop().unwrap() {
                push_block(stack, Block::BlockTypst { src, width });
            }
        } else if !run.is_empty() {
            push_block(stack, Block::Paragraph(std::mem::take(run)));
        }
    }

    if !inlines
        .iter()
        .any(|inline| matches!(inline, Inline::BlockFragment(_)))
    {
        flush_inline_run(stack, inlines);
        return;
    }
    let mut run = Vec::new();
    for inline in std::mem::take(inlines) {
        match inline {
            Inline::BlockFragment(block) => {
                flush_inline_run(stack, &mut run);
                push_block(stack, *block);
            }
            inline => run.push(inline),
        }
    }
    flush_inline_run(stack, &mut run);
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

const COMMAND_MARKER: char = '\u{F8FF}';

fn extract_commands_from_markdown(md: &str) -> (String, Vec<ParsedCommand>) {
    let mut out = String::new();
    let mut commands = Vec::new();
    let mut code_fence: Option<(u8, usize)> = None;
    let mut pending_markdown = String::new();
    for line in md.split_inclusive('\n') {
        let trimmed = line.trim_start();
        let marker = trimmed.as_bytes().first().copied().unwrap_or(0);
        let count = trimmed.bytes().take_while(|&b| b == marker).count();
        if let Some((open, length)) = code_fence {
            if marker == open && count >= length && trimmed[count..].trim().is_empty() {
                code_fence = None;
            }
            out.push_str(line);
        } else if matches!(marker, b'`' | b'~')
            && count >= 3
            && (marker != b'`' || !trimmed[count..].contains('`'))
        {
            extract_commands(&pending_markdown, &mut out, &mut commands);
            pending_markdown.clear();
            code_fence = Some((marker, count));
            out.push_str(line);
        } else {
            if let Some(head) = missing_table_header(line, pending_markdown.lines().last()) {
                pending_markdown.push_str(&head);
            }
            pending_markdown.push_str(line);
        }
    }
    extract_commands(&pending_markdown, &mut out, &mut commands);
    (out, commands)
}

fn missing_table_header(line: &str, prev: Option<&str>) -> Option<String> {
    let t = line.trim();
    if !t.starts_with('|') || !t.contains('-') {
        return None;
    }
    if !t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' ')) {
        return None;
    }
    if prev.is_some_and(|p| p.contains('|')) {
        return None;
    }
    let cols = t.trim_matches('|').split('|').count();
    Some(format!("|{}\n", " |".repeat(cols)))
}

pub(crate) fn art_parts(src: &str) -> Vec<ArtPart> {
    let mut parts = Vec::new();
    let mut lines: Vec<ArtLine> = Vec::new();
    let raw: Vec<&str> = src.lines().collect();
    let mut i = 0;
    while i < raw.len() {
        match parse_standalone_art_command(&raw[i..]) {
            Some((guide, block, used)) => {
                if !lines.is_empty() {
                    parts.push(ArtPart::Lines(std::mem::take(&mut lines)));
                }
                parts.push(ArtPart::Nested {
                    guide,
                    block: Box::new(block),
                });
                i += used;
            }
            None => {
                lines.push(art_line(raw[i]));
                i += 1;
            }
        }
    }
    if !lines.is_empty() {
        parts.push(ArtPart::Lines(lines));
    }
    parts
}

fn art_line(line: &str) -> ArtLine {
    let split = line
        .char_indices()
        .find(|(_, c)| !guides(*c))
        .map_or(line.len(), |(i, _)| i);
    let (guide, text) = line.split_at(split);
    ArtLine {
        guide: guide.to_string(),
        inls: art_inlines(text),
    }
}

pub(crate) fn art_inlines(text: &str) -> Vec<Inline> {
    if text.is_empty() {
        return Vec::new();
    }
    match parse(text).slides.pop() {
        Some(mut slide) => match slide.blocks.pop() {
            Some(Block::Paragraph(inls)) if slide.blocks.is_empty() => inls,
            _ => vec![Inline::Text(text.to_string(), Style::default())],
        },
        None => vec![Inline::Text(text.to_string(), Style::default())],
    }
}

fn parse_standalone_art_command(raw: &[&str]) -> Option<(String, Block, usize)> {
    let first = raw[0];
    let start = first.find('◊')?;
    let guide = &first[..start];
    if !guide.chars().all(guides) {
        return None;
    }
    let mut body = String::new();
    let mut brackets = 0usize;
    let mut braces = 0usize;
    let mut used = 0;
    'lines: for line in raw {
        let line = line.strip_prefix(guide).unwrap_or(line);
        if used > 0 {
            body.push('\n');
        }
        body.push_str(line);
        used += 1;
        for c in line.chars() {
            match c {
                '[' if braces == 0 => brackets += 1,
                ']' if braces == 0 => brackets = brackets.checked_sub(1)?,
                '{' if brackets == 0 => braces += 1,
                '}' if brackets == 0 => {
                    braces = braces.checked_sub(1)?;
                    if braces == 0 {
                        break 'lines;
                    }
                }
                _ => {}
            }
        }
    }
    let open = '◊'.len_utf8();
    let (frag, consumed) = parse_command(&body[open..])?;
    if !body[open + consumed..].trim().is_empty() {
        return None;
    }
    let block = match frag {
        ParsedCommand::Block(b) => b,
        ParsedCommand::Inline { src, width } => Block::BlockTypst { src, width },
    };
    Some((guide.to_string(), block, used))
}

fn extract_commands(text: &str, out: &mut String, commands: &mut Vec<ParsedCommand>) {
    let mut rest = text;
    while !rest.is_empty() {
        let backtick_offset = rest.find('`');
        let command_offset = rest.find('◊');
        let code_precedes_command = match (backtick_offset, command_offset) {
            (Some(t), Some(l)) => t < l,
            (Some(_), None) => true,
            _ => false,
        };
        if code_precedes_command {
            let t = backtick_offset.unwrap();
            out.push_str(&rest[..t]);
            rest = &rest[t + copy_inline_code_span(&rest[t..], out)..];
        } else if let Some(p) = command_offset {
            out.push_str(&rest[..p]);
            let after = &rest[p + '◊'.len_utf8()..];
            match parse_command(after) {
                Some((frag, consumed)) => {
                    let idx = commands.len();
                    commands.push(frag);
                    out.push(COMMAND_MARKER);
                    out.push_str(&idx.to_string());
                    out.push(COMMAND_MARKER);
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

fn copy_inline_code_span(s: &str, out: &mut String) -> usize {
    let n = s.bytes().take_while(|&b| b == b'`').count();
    let mut pos = n;
    while let Some(rel) = s[pos..].find('`') {
        pos += rel;
        let length = s[pos..].bytes().take_while(|&b| b == b'`').count();
        if length == n {
            let end = pos + n;
            out.push_str(&s[..end]);
            return end;
        }
        pos += length;
    }
    out.push_str(&s[..n]);
    n
}

fn push_text(inlines: &mut Vec<Inline>, t: &str, style: Style, commands: &[ParsedCommand]) {
    if !t.contains(COMMAND_MARKER) {
        if !t.is_empty() {
            inlines.push(Inline::Text(t.to_string(), style));
        }
        return;
    }
    let mut buf = String::new();
    let mut chars = t.chars().peekable();
    while let Some(c) = chars.next() {
        if c != COMMAND_MARKER {
            buf.push(c);
            continue;
        }
        let mut num = String::new();
        while let Some(&d) = chars.peek() {
            chars.next();
            if d == COMMAND_MARKER {
                break;
            }
            num.push(d);
        }
        match num.parse::<usize>().ok().and_then(|i| commands.get(i)) {
            Some(frag) => {
                if !buf.is_empty() {
                    inlines.push(Inline::Text(std::mem::take(&mut buf), style));
                }
                inlines.push(match frag {
                    ParsedCommand::Inline { src, width } => Inline::InlineTypst {
                        src: src.clone(),
                        width: *width,
                        display: true,
                    },
                    ParsedCommand::Block(b) => Inline::BlockFragment(Box::new(b.clone())),
                });
            }
            None => {
                buf.push(COMMAND_MARKER);
                buf.push_str(&num);
            }
        }
    }
    if !buf.is_empty() {
        inlines.push(Inline::Text(buf, style));
    }
}

fn math_inline(latex: &str, display: bool, style: Style) -> Inline {
    match mitex::convert_math(latex, None) {
        Ok(src) => Inline::InlineTypst {
            src,
            width: Width::Natural,
            display,
        },
        Err(e) => Inline::Text(format!("[math error: {e}]"), style),
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
