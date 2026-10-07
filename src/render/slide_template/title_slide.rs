use crate::assets::Assets;
use std::collections::HashSet;

use crate::core::ir::{Block, Inline, Slide, TocEntry};
use crate::layout::ops::hits as click_hits;
use crate::layout::RenderOp;
use crate::layout::{layout, TermInfo};
use crate::layout::{line_width, split_lines};
use crate::render::blocks::{emit_block, DetailsState};
use crate::render::boxes::{emit_box, BoxStyle};
use crate::render::inline::{emit_inlines, uppercase_inlines};
use crate::render::paint::{
    cell_width, current_row, dim_style, heading_style, indent_op, truncate, Hit, HitAction,
};

pub(crate) fn render(
    slide: &Slide,
    term: &TermInfo,
    assets: &Assets,
    open: &HashSet<usize>,
) -> (Vec<RenderOp>, Vec<Hit>) {
    let (margin, content_width) = layout(term);

    let lines: Vec<Vec<Inline>> = slide
        .blocks
        .iter()
        .filter_map(|b| match b {
            Block::Heading(_, inls) => Some(uppercase_inlines(inls)),
            _ => None,
        })
        .flat_map(|inls| split_breaks(&inls))
        .collect();
    let title_width_limit = content_width.saturating_sub(4).max(1);
    let box_term = term.with_cols(title_width_limit);
    let title_rows: Vec<Vec<RenderOp>> = lines
        .iter()
        .flat_map(|line| {
            let mut title_ops = Vec::new();
            emit_inlines(
                line,
                heading_style(),
                &box_term,
                assets,
                0,
                0,
                &mut title_ops,
            );
            split_lines(title_ops)
        })
        .collect();
    let title_width = title_rows
        .iter()
        .map(|v| line_width(v))
        .max()
        .unwrap_or(0)
        .max(1);

    let mut ops = Vec::new();
    ops.push(RenderOp::LineBreak);
    let box_style = BoxStyle::bordered(title_width);
    emit_box(title_rows, title_width, margin, &box_style, &mut ops);

    let mut hits = emit_toc(&slide.toc, content_width, margin, &mut ops);

    let body = term.with_cols(margin + content_width);
    let mut details = DetailsState { open, next_id: 0 };
    for block in &slide.blocks {
        if matches!(block, Block::Heading(_, _)) {
            continue;
        }
        ops.push(RenderOp::LineBreak);
        emit_block(block, &body, assets, margin, &mut details, &mut ops);
        ops.push(RenderOp::LineBreak);
    }

    hits.extend(click_hits(&ops));
    (ops, hits)
}

fn emit_toc(
    toc: &[TocEntry],
    content_width: usize,
    margin: usize,
    ops: &mut Vec<RenderOp>,
) -> Vec<Hit> {
    let mut hits = Vec::new();
    if toc.is_empty() {
        return hits;
    }
    ops.push(RenderOp::LineBreak);
    ops.push(indent_op(margin));
    ops.push(RenderOp::Text(
        truncate("CONTENTS", content_width),
        heading_style(),
    ));
    ops.push(RenderOp::LineBreak);
    let number_width = toc.len().to_string().len();
    let first_row = current_row(ops);
    for (n, entry) in toc.iter().enumerate() {
        let label = truncate(
            &format!("{:>number_width$}.  {}", n + 1, entry.title),
            content_width,
        );
        let row = (first_row + n) as u16;
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
