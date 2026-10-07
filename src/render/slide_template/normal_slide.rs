use crate::assets::Assets;
use std::collections::HashSet;

use crate::core::ir::Slide;
use crate::layout::ops::hits as click_hits;
use crate::layout::RenderOp;
use crate::layout::{layout, TermInfo};
use crate::render::blocks::{emit_block, DetailsState};
use crate::render::paint::Hit;

pub(crate) fn render(
    slide: &Slide,
    term: &TermInfo,
    assets: &Assets,
    open: &HashSet<usize>,
) -> (Vec<RenderOp>, Vec<Hit>) {
    let mut ops = Vec::new();
    let mut hits: Vec<Hit> = Vec::new();
    let (margin, content_width) = layout(term);
    let body = term.with_cols(margin + content_width);
    ops.push(RenderOp::LineBreak);
    let mut details = DetailsState { open, next_id: 0 };
    for block in &slide.blocks {
        emit_block(block, &body, assets, margin, &mut details, &mut ops);
        ops.push(RenderOp::LineBreak);
    }
    hits.extend(click_hits(&ops));
    (ops, hits)
}
