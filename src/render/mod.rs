pub mod blocks;
pub mod inline;
pub mod paint;
pub mod slide_template;

use std::collections::HashSet;
use std::path::Path;

use crate::core::ir::{RenderOp, Slide};
use crate::layout::TermInfo;

pub use slide_template::render as body;

// Full-frame body with all details closed (used by --dump-ops / snapshot).
pub fn render(slide: &Slide, term: &TermInfo, deck_dir: &Path) -> Vec<RenderOp> {
    let (b, _, _) = body(slide, term, deck_dir, &HashSet::new());
    let mut ops = vec![RenderOp::ClearImages, RenderOp::MoveTo(0, 0)];
    ops.extend(b);
    ops
}
