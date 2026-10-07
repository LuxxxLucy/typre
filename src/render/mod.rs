mod art;
pub(crate) mod blocks;
mod boxes;
mod commands;
pub mod frame;
pub mod inline;
pub mod paint;
pub mod slide_template;
mod table;

use std::collections::HashSet;

use crate::assets::Assets;
use crate::core::ir::Slide;
use crate::layout::Body;
use crate::layout::RenderOp;
use crate::layout::TermInfo;

pub fn body(slide: &Slide, term: &TermInfo, assets: &Assets, open: &HashSet<usize>) -> Body {
    let (ops, hits) = slide_template::render(slide, term, assets, open);
    Body {
        rows: crate::layout::split_lines(ops),
        hits,
    }
}

pub fn render(slide: &Slide, term: &TermInfo, assets: &Assets) -> Vec<RenderOp> {
    let body = body(slide, term, assets, &HashSet::new());
    let mut ops = vec![RenderOp::ClearImages, RenderOp::MoveTo(0, 0)];
    ops.extend(body.into_ops());
    ops
}
