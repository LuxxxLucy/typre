use std::path::Path;

use crate::core::ir::{Block, Inline, RenderOp, Slide, Style, TocEntry};
use crate::layout::{layout, TermInfo};
use crate::render::blocks::emit_block;
use crate::render::inline::{disp_width, emit_inlines, link_hits, uppercase_inlines};
use crate::render::paint::{current_row, dim_style, heading_style, hrule, indent_op, Hit, HitAction};

// Title slide: the heading sits in a bordered box at the normal slide margin and
// width. The opening title slide lists the deck's sections (slide.toc) as jump
// links directly below the box, ahead of the rest of the body.
pub(crate) fn render(
    slide: &Slide,
    term: &TermInfo,
    deck_dir: &Path,
) -> (Vec<RenderOp>, Vec<Hit>) {
    let (margin, content_w) = layout(term);
    let pre = " ".repeat(margin);

    // The box fits the widest title line, capped to the zen column.
    let lines: Vec<Vec<Inline>> = slide
        .blocks
        .iter()
        .filter_map(|b| match b {
            Block::Heading(_, inls) => Some(uppercase_inlines(inls)),
            _ => None,
        })
        .flat_map(|inls| split_breaks(&inls))
        .collect();
    let inner = lines
        .iter()
        .map(|l| disp_width(l))
        .max()
        .unwrap_or(0)
        .min(content_w.saturating_sub(4))
        .max(1);

    let mut ops = Vec::new();
    ops.push(RenderOp::LineBreak); // top padding
    ops.push(RenderOp::Text(
        format!("{pre}{}", hrule('┌', inner + 2, '┐')),
        Style::default(),
    ));
    ops.push(RenderOp::LineBreak);
    for line in &lines {
        ops.push(RenderOp::Text(format!("{pre}│ "), Style::default()));
        emit_inlines(line, heading_style(), term, deck_dir, 0, 0, &mut ops);
        let slack = inner.saturating_sub(disp_width(line));
        ops.push(RenderOp::Text(format!("{} │", " ".repeat(slack)), Style::default()));
        ops.push(RenderOp::LineBreak);
    }
    ops.push(RenderOp::Text(
        format!("{pre}{}", hrule('└', inner + 2, '┘')),
        Style::default(),
    ));
    ops.push(RenderOp::LineBreak);

    // section list directly below the box, before the rest of the body
    let mut hits = emit_toc(&slide.toc, content_w, margin, &mut ops);

    let body = term.with_cols(margin + content_w);
    for block in &slide.blocks {
        if matches!(block, Block::Heading(_, _)) {
            continue;
        }
        ops.push(RenderOp::LineBreak); // blank line above each block
        emit_block(block, &body, deck_dir, margin, &mut ops);
        ops.push(RenderOp::LineBreak);
    }

    hits.extend(link_hits(&ops));
    (ops, hits)
}

// The table of contents: a "CONTENTS" label then one clickable line per section,
// numbered in order. Each line is a Goto hit covering its text.
fn emit_toc(toc: &[TocEntry], content_w: usize, margin: usize, ops: &mut Vec<RenderOp>) -> Vec<Hit> {
    let mut hits = Vec::new();
    if toc.is_empty() {
        return hits;
    }
    ops.push(RenderOp::LineBreak);
    ops.push(indent_op(margin));
    ops.push(RenderOp::Text("CONTENTS".to_string(), heading_style()));
    ops.push(RenderOp::LineBreak);
    for (n, entry) in toc.iter().enumerate() {
        let label: String = format!("{}.  {}", n + 1, entry.title)
            .chars()
            .take(content_w)
            .collect();
        let row = current_row(ops) as u16;
        let start = margin as u16;
        let end = start + label.chars().count() as u16;
        ops.push(indent_op(margin));
        ops.push(RenderOp::Text(label, dim_style()));
        ops.push(RenderOp::LineBreak);
        hits.push(Hit {
            row,
            cols: start..end,
            action: HitAction::Goto(entry.index),
        });
    }
    hits
}

// Split an inline run into visual lines at soft/hard breaks.
fn split_breaks(inls: &[Inline]) -> Vec<Vec<Inline>> {
    let mut out = Vec::new();
    let mut start = 0;
    for i in 0..=inls.len() {
        if i == inls.len() || matches!(inls[i], Inline::SoftBreak | Inline::HardBreak) {
            out.push(inls[start..i].to_vec());
            start = i + 1;
        }
    }
    out
}
