use super::boxes::{emit_box, BoxDeco};
use super::commands;
use super::inline::{emit_inlines, emit_tokens, inline_tokens, tokens_width, uppercase_inlines};
use super::paint::{
    caption_style, cell_width, code_style, heading_style, indent_op, pad, place_image, quote_style,
};
use crate::assets::Assets;
use crate::core::ir::{Align, Block, Inline, Style, Width};
use crate::layout::{split_lines, RenderOp, TermInfo};
use std::collections::HashSet;

// Details boxes are numbered as they are emitted, at any nesting depth, so a box inside art
// toggles like one at the top of a slide. `open` holds the ids the user has expanded.
pub(crate) struct Toggles<'a> {
    pub open: &'a HashSet<usize>,
    pub next_id: usize,
}

impl Toggles<'_> {
    fn take_id(&mut self) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}

pub(crate) fn emit_block(
    block: &Block,
    term: &TermInfo,
    assets: &Assets,
    indent: usize,
    tg: &mut Toggles,
    ops: &mut Vec<RenderOp>,
) {
    match block {
        Block::Heading(level, inls) => {
            let owned: Vec<Inline> = if *level <= 2 {
                uppercase_inlines(inls)
            } else {
                inls.clone()
            };
            if *level == 1 {
                let avail = (term.cols as usize).saturating_sub(indent);
                let tokens = inline_tokens(&owned, heading_style(), term, assets);
                let eff = indent + avail.saturating_sub(tokens_width(&tokens)) / 2;
                emit_tokens(&tokens, term.cols as usize, eff, eff, ops);
            } else {
                // accent bar by level: ┃ for section titles, │ for subsections
                let accent = if *level == 2 {
                    "┃ "
                } else if *level == 3 {
                    "│ "
                } else {
                    ""
                };
                let mut hinls = Vec::new();
                if !accent.is_empty() {
                    hinls.push(Inline::Text(accent.to_string(), heading_style()));
                }
                hinls.extend(owned);
                emit_inlines(&hinls, heading_style(), term, assets, indent, indent, ops);
            }
            ops.push(RenderOp::LineBreak);
        }
        Block::Paragraph(inls) => {
            emit_inlines(inls, Style::default(), term, assets, indent, indent, ops);
            ops.push(RenderOp::LineBreak);
        }
        Block::List { ordered, items } => {
            emit_list(*ordered, items, "", term, assets, indent, tg, ops);
        }
        Block::Code { src, lang } => emit_code(src, lang.as_deref(), term, indent, ops),
        Block::Art { parts, caption } => {
            super::art::render(parts, caption, term, assets, indent, tg, ops)
        }
        Block::BlockTypst { src, width } => {
            commands::typst::render_block(src, *width, term, assets, indent, ops)
        }
        Block::Image { src, alt } => {
            if let Some(png_path) = assets.image(src) {
                place_image(ops, png_path, assets, term, indent, Width::Natural);
                if !alt.is_empty() {
                    let content_w = (term.cols as usize).saturating_sub(indent);
                    ops.push(indent_op(indent));
                    ops.push(RenderOp::Text(
                        pad(alt, content_w, Align::Center),
                        caption_style(),
                    ));
                    ops.push(RenderOp::LineBreak);
                }
            } else {
                ops.push(indent_op(indent));
                ops.push(RenderOp::Text(format!("[image: {alt}]"), Style::default()));
                ops.push(RenderOp::LineBreak);
            }
        }
        Block::Rule => {
            let w = (term.cols as usize).saturating_sub(indent);
            ops.push(RenderOp::Text(
                format!("{}{}", " ".repeat(indent), "═".repeat(w)),
                Style::default(),
            ));
            ops.push(RenderOp::LineBreak);
        }
        Block::Table { aligns, head, rows } => {
            super::table::render(aligns, head, rows, term, assets, indent, ops);
        }
        Block::Quote(inner) => emit_quote(inner, term, assets, indent, tg, ops),
        Block::Tree(nodes) => commands::tree::render(nodes, indent, ops),
        Block::Grid(cells) => commands::grid::render(cells, term, indent, ops),
        Block::Details { summary, body } => {
            let id = tg.take_id();
            let open = tg.open.contains(&id);
            commands::details::render(id, open, summary, body, term, assets, indent, ops);
        }
    }
}

fn code_label_style() -> Style {
    Style {
        bold: true,
        ..code_style()
    }
}

// A fenced code block: the language as a label when the fence names one, then every line on
// the panel background, padded to the column.
fn emit_code(
    src: &str,
    lang: Option<&str>,
    term: &TermInfo,
    indent: usize,
    ops: &mut Vec<RenderOp>,
) {
    let w = (term.cols as usize).saturating_sub(indent).max(1);
    if let Some(lang) = lang {
        ops.push(indent_op(indent));
        ops.push(RenderOp::Text(format!(" {lang} "), code_label_style()));
        ops.push(RenderOp::LineBreak);
    }
    for line in src.lines() {
        ops.push(indent_op(indent));
        ops.push(RenderOp::Text(
            pad(&format!("  {line}"), w, Align::Left),
            code_style(),
        ));
        ops.push(RenderOp::LineBreak);
    }
}

// A blockquote: a grey background box spanning the content width, one space of
// inset each side, with a blank row above and below.
fn emit_quote(
    inner: &[Block],
    term: &TermInfo,
    assets: &Assets,
    indent: usize,
    tg: &mut Toggles,
    ops: &mut Vec<RenderOp>,
) {
    let text_w = (term.cols as usize).saturating_sub(indent + 2).max(1);
    let inner_term = term.with_cols(text_w);
    let mut sub = Vec::new();
    for b in inner {
        emit_block(b, &inner_term, assets, 0, tg, &mut sub);
    }
    let deco = BoxDeco::shaded(text_w, quote_style());
    emit_box(split_lines(sub), text_w, indent, &deco, ops);
}

#[allow(clippy::too_many_arguments)]
fn emit_list(
    ordered: bool,
    items: &[Vec<Block>],
    prefix: &str,
    term: &TermInfo,
    assets: &Assets,
    indent: usize,
    tg: &mut Toggles,
    ops: &mut Vec<RenderOp>,
) {
    for (i, item) in items.iter().enumerate() {
        let marker = if ordered {
            format!("{prefix}{}. ", i + 1)
        } else {
            "▪ ".to_string()
        };
        let item_prefix = if ordered {
            format!("{prefix}{}.", i + 1)
        } else {
            String::new()
        };
        ops.push(RenderOp::Text(
            format!("{}{marker}", " ".repeat(indent)),
            Style::default(),
        ));
        let cont = indent + cell_width(&marker);
        for (j, b) in item.iter().enumerate() {
            match b {
                Block::Paragraph(inls) => {
                    let lead = if j == 0 { 0 } else { cont };
                    emit_inlines(inls, Style::default(), term, assets, lead, cont, ops);
                    ops.push(RenderOp::LineBreak);
                }
                Block::List { ordered: o, items } => {
                    emit_list(*o, items, &item_prefix, term, assets, cont, tg, ops);
                }
                nested => emit_block(nested, term, assets, cont, tg, ops),
            }
        }
    }
}
