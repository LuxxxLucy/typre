use std::collections::HashSet;
use std::path::Path;

use crate::core::ir::{Block, RenderOp, Slide};
use crate::layout::{layout, TermInfo};
use crate::render::blocks::emit_block;
use crate::commands;
use crate::render::inline::link_hits;
use crate::render::paint::Hit;

// A normal slide: top padding, then each block at the content margin. Top-level
// details boxes toggle (their click target is returned); links are click targets.
pub(crate) fn render(
    slide: &Slide,
    term: &TermInfo,
    deck_dir: &Path,
    open: &HashSet<usize>,
) -> (Vec<RenderOp>, Vec<Hit>) {
    let mut ops = Vec::new();
    let mut hits = Vec::new();
    let (margin, content_w) = layout(term);
    let body = TermInfo {
        cols: (margin + content_w) as u16,
        rows: term.rows,
        cell_w_px: term.cell_w_px,
        cell_h_px: term.cell_h_px,
    };
    ops.push(RenderOp::LineBreak); // top padding
    let mut details_id = 0usize;
    for block in &slide.blocks {
        match block {
            Block::Details { summary, body: lines } => {
                let id = details_id;
                details_id += 1;
                if let Some(hit) = commands::details::render(
                    Some(id),
                    open.contains(&id),
                    summary,
                    lines,
                    &body,
                    margin,
                    &mut ops,
                ) {
                    hits.push(hit);
                }
            }
            _ => emit_block(block, &body, deck_dir, margin, &mut ops),
        }
        ops.push(RenderOp::LineBreak);
    }
    hits.extend(link_hits(&ops));
    (ops, hits)
}
