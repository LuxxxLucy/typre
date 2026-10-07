pub mod normal_slide;
pub mod title_slide;

use crate::assets::Assets;
use std::collections::HashSet;

use crate::core::ir::Slide;
use crate::layout::RenderOp;
use crate::layout::TermInfo;
use crate::render::paint::Hit;

pub fn render(
    slide: &Slide,
    term: &TermInfo,
    assets: &Assets,
    open: &HashSet<usize>,
) -> (Vec<RenderOp>, Vec<Hit>) {
    let (ops, hits) = if slide.is_title() {
        title_slide::render(slide, term, assets, open)
    } else {
        normal_slide::render(slide, term, assets, open)
    };
    (ops, hits)
}
