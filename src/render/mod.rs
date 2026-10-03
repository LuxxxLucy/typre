pub mod blocks;
mod commands;
pub mod frame;
pub mod inline;
pub mod paint;
pub mod slide_template;

use std::collections::HashSet;

use crate::assets::Assets;
use crate::core::ir::Slide;
use crate::layout::Body;
use crate::layout::RenderOp;
use crate::layout::TermInfo;

pub fn body(slide: &Slide, term: &TermInfo, assets: &Assets, open: &HashSet<usize>) -> Body {
    let (ops, hits) = slide_template::render(slide, term, assets, open);
    Body {
        rows: blocks::split_lines(ops),
        hits,
    }
}

pub fn render(slide: &Slide, term: &TermInfo, assets: &Assets) -> Vec<RenderOp> {
    let body = body(slide, term, assets, &HashSet::new());
    let mut ops = vec![RenderOp::ClearImages, RenderOp::MoveTo(0, 0)];
    ops.extend(body.window(0, body.height()).0);
    ops
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ir::{Style, Width};
    use crate::layout::Hit;
    use crate::render::paint::{place_image, HitAction};

    #[test]
    fn scrolling_preserves_visible_image_rows() {
        let temp = tempfile::tempdir().unwrap();
        let png = temp.path().join("tall.png");
        image::RgbaImage::new(8, 64).save(&png).unwrap();
        let term = TermInfo {
            cols: 10,
            rows: 5,
            cell_w_px: 8,
            cell_h_px: 16,
        };
        let mut ops = Vec::new();
        place_image(
            &mut ops,
            png,
            &Assets::new(temp.path()),
            &term,
            0,
            Width::Natural,
        );
        ops.push(RenderOp::Text("after".to_string(), Style::default()));
        ops.push(RenderOp::LineBreak);
        let body = Body {
            rows: blocks::split_lines(ops),
            hits: vec![Hit {
                row: 2,
                cols: 1..3,
                action: HitAction::Goto(1),
            }],
        };
        assert_eq!(body.height(), 5);
        let (visible, hits) = body.window(1, 2);
        let slices: Vec<_> = visible
            .iter()
            .filter_map(|op| match op {
                RenderOp::ImageRow { row, rows, .. } => Some((*row, *rows)),
                _ => None,
            })
            .collect();
        assert_eq!(slices, [(1, 4), (2, 4)]);
        assert_eq!(
            visible
                .iter()
                .filter(|op| matches!(op, RenderOp::LineBreak))
                .count(),
            2
        );
        assert_eq!(hits[0].row, 1);
        assert_eq!(body.height(), 5);
        assert!(body.window(5, 2).0.is_empty());
    }
}
