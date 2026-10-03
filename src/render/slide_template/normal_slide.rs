use crate::assets::Assets;
use std::collections::HashSet;

use crate::core::ir::Slide;
use crate::layout::ops::hits as click_hits;
use crate::layout::RenderOp;
use crate::layout::{layout, TermInfo};
use crate::render::blocks::{emit_block, Toggles};
use crate::render::paint::Hit;

// A normal slide: top padding, then each block at the content margin. Details boxes and links
// are click targets.
pub(crate) fn render(
    slide: &Slide,
    term: &TermInfo,
    assets: &Assets,
    open: &HashSet<usize>,
) -> (Vec<RenderOp>, Vec<Hit>) {
    let mut ops = Vec::new();
    let mut hits: Vec<Hit> = Vec::new();
    let (margin, content_w) = layout(term);
    let body = term.with_cols(margin + content_w);
    ops.push(RenderOp::LineBreak); // top padding
    let mut tg = Toggles { open, next_id: 0 };
    for block in &slide.blocks {
        emit_block(block, &body, assets, margin, &mut tg, &mut ops);
        ops.push(RenderOp::LineBreak);
    }
    hits.extend(click_hits(&ops));
    (ops, hits)
}
