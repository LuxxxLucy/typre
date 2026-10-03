use crate::assets::Assets;
use crate::core::ir::{Style, Width};
use crate::layout::RenderOp;
use crate::layout::{natural_ppi, TermInfo};
use crate::render::paint::place_image;

pub(crate) fn render_block(
    src: &str,
    width: Width,
    term: &TermInfo,
    assets: &Assets,
    indent: usize,
    ops: &mut Vec<RenderOp>,
) {
    match assets.fragment(src, natural_ppi(term), true) {
        Ok(png_path) => {
            place_image(ops, png_path, assets, term, indent, width);
        }
        Err(e) => {
            ops.push(RenderOp::Text(
                format!("{}[typst error: {e}]", " ".repeat(indent)),
                Style::default(),
            ));
            ops.push(RenderOp::LineBreak);
        }
    }
}
