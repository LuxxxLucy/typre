pub mod normal_slide;
pub mod title_slide;

use std::collections::HashSet;
use std::path::Path;

use crate::core::ir::{Block, RenderOp, Slide};
use crate::layout::TermInfo;
use crate::render::paint::{current_row, Hit};

// A title slide leads with an H1; a normal slide leads with an H2.
fn is_title_slide(slide: &Slide) -> bool {
    matches!(slide.blocks.first(), Some(Block::Heading(1, _)))
}

// The slide body from row 0 (no screen clear, no chrome): ops, click targets, height.
pub fn render(
    slide: &Slide,
    term: &TermInfo,
    deck_dir: &Path,
    open: &HashSet<usize>,
) -> (Vec<RenderOp>, Vec<Hit>, usize) {
    let (ops, hits) = if is_title_slide(slide) {
        title_slide::render(slide, term, deck_dir)
    } else {
        normal_slide::render(slide, term, deck_dir, open)
    };
    let height = current_row(&ops);
    (ops, hits, height)
}
