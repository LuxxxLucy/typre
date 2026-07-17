use std::path::Path;

use crate::core::ir::{Block, Inline, RenderOp, Slide, Style, TocEntry};
use crate::layout::{layout, TermInfo};
use crate::render::blocks::{emit_block, line_width, split_lines};
use crate::render::inline::{emit_inlines, link_hits, uppercase_inlines};
use crate::render::paint::{cell_width, current_row, dim_style, heading_style, hrule, indent_op, Hit, HitAction};

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

    // Wrap the title to the zen column, then size the box to the widest wrapped line.
    let lines: Vec<Vec<Inline>> = slide
        .blocks
        .iter()
        .filter_map(|b| match b {
            Block::Heading(_, inls) => Some(uppercase_inlines(inls)),
            _ => None,
        })
        .flat_map(|inls| split_breaks(&inls))
        .collect();
    let cap = content_w.saturating_sub(4).max(1);
    let box_term = term.with_cols(cap);
    let vlines: Vec<Vec<RenderOp>> = lines
        .iter()
        .flat_map(|line| {
            let mut sub = Vec::new();
            emit_inlines(line, heading_style(), &box_term, deck_dir, 0, 0, &mut sub);
            split_lines(sub)
        })
        .collect();
    let inner = vlines.iter().map(|v| line_width(v)).max().unwrap_or(0).max(1);

    let mut ops = Vec::new();
    ops.push(RenderOp::LineBreak); // top padding
    ops.push(RenderOp::Text(
        format!("{pre}{}", hrule('┌', inner + 2, '┐')),
        Style::default(),
    ));
    ops.push(RenderOp::LineBreak);
    for line in vlines {
        let w = line_width(&line);
        ops.push(RenderOp::Text(format!("{pre}│ "), Style::default()));
        ops.extend(line);
        ops.push(RenderOp::Text(format!("{} │", " ".repeat(inner - w)), Style::default()));
        ops.push(RenderOp::LineBreak);
    }
    ops.push(RenderOp::Text(
        format!("{pre}{}", hrule('└', inner + 2, '┘')),
        Style::default(),
    ));
    ops.push(RenderOp::LineBreak);

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
    let num_w = toc.len().to_string().len();
    for (n, entry) in toc.iter().enumerate() {
        let label: String = format!("{:>num_w$}.  {}", n + 1, entry.title)
            .chars()
            .take(content_w)
            .collect();
        let row = current_row(ops) as u16;
        let start = margin as u16;
        let end = start + cell_width(&label) as u16;
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
