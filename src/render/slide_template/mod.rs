pub mod normal_slide;
pub mod title_slide;

use std::collections::HashSet;
use std::path::Path;

use crate::core::ir::{RenderOp, Slide};
use crate::layout::TermInfo;
use crate::render::paint::{current_row, Hit};

// The slide body from row 0 (no screen clear, no chrome): ops, click targets, height.
pub fn render(
    slide: &Slide,
    term: &TermInfo,
    deck_dir: &Path,
    open: &HashSet<usize>,
) -> (Vec<RenderOp>, Vec<Hit>, usize) {
    let (ops, hits) = if slide.is_title() {
        title_slide::render(slide, term, deck_dir)
    } else {
        normal_slide::render(slide, term, deck_dir, open)
    };
    let height = current_row(&ops);
    (ops, hits, height)
}
