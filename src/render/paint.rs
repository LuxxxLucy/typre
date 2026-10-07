use crate::assets::Assets;
use std::path::PathBuf;

pub(crate) use crate::layout::text::{break_units, cell_width, truncate, wrap_text};

use crate::core::ir::{Align, Style, Width};
use crate::layout::RenderOp;
use crate::layout::TermInfo;

pub use crate::layout::{Hit, HitAction};

pub(crate) fn indent_op(indent: usize) -> RenderOp {
    RenderOp::Text(" ".repeat(indent), Style::default())
}

pub(crate) fn current_row(ops: &[RenderOp]) -> usize {
    ops.iter()
        .filter(|o| matches!(o, RenderOp::LineBreak))
        .count()
}

pub(crate) fn image_cells(
    dimensions: Option<(u32, u32)>,
    term: &TermInfo,
    indent: usize,
    width: Width,
) -> (u16, u16) {
    let cell_width_px = term.cell_w_px.max(1) as f32;
    let cell_height_px = term.cell_h_px.max(1) as f32;
    let (image_width_px, image_height_px) =
        dimensions.unwrap_or((cell_width_px as u32, cell_height_px as u32));
    let natural_columns = (image_width_px as f32 / cell_width_px).max(1.0);
    let natural_rows = (image_height_px as f32 / cell_height_px).max(1.0);
    let content_width = (term.cols as usize).saturating_sub(indent).max(1) as f32;
    let target_columns = match width {
        Width::Natural => natural_columns.min(content_width),
        Width::Percent(p) => content_width * (p as f32 / 100.0),
        Width::Cols(c) => (c as f32).min(content_width),
    };
    let scale = target_columns / natural_columns;
    let cols = (natural_columns * scale).round().max(1.0) as u16;
    let rows = (natural_rows * scale).round().max(1.0) as u16;
    (cols, rows)
}

pub(crate) fn place_image(
    ops: &mut Vec<RenderOp>,
    png_path: PathBuf,
    assets: &Assets,
    term: &TermInfo,
    indent: usize,
    width: Width,
) -> (u16, u16) {
    let (cols, rows) = image_cells(assets.dimensions(&png_path), term, indent, width);
    let content_width = (term.cols as usize).saturating_sub(indent);
    let padding_columns = content_width.saturating_sub(cols as usize) / 2;
    for row in 0..rows {
        ops.push(indent_op(indent + padding_columns));
        ops.push(RenderOp::ImageRow {
            png_path: png_path.clone(),
            cols,
            rows,
            row,
        });
        ops.push(RenderOp::LineBreak);
    }
    (cols, rows)
}

pub(crate) fn heading_style() -> Style {
    Style {
        bold: true,
        ..Style::default()
    }
}

pub(crate) fn dim_style() -> Style {
    Style {
        dim: true,
        ..Style::default()
    }
}

pub(crate) fn caption_style() -> Style {
    Style {
        italic: true,
        ..Style::default()
    }
}

pub(crate) fn code_style() -> Style {
    Style {
        code: true,
        ..Style::default()
    }
}

pub(crate) fn quote_style() -> Style {
    Style {
        quote: true,
        ..Style::default()
    }
}

pub(crate) fn hrule(left: char, width: usize, right: char) -> String {
    format!("{left}{}{right}", "─".repeat(width))
}

pub(crate) fn pad(s: &str, width: usize, align: Align) -> String {
    let len = cell_width(s);
    if len >= width {
        return s.to_string();
    }
    let padding_columns = width - len;
    match align {
        Align::Left => format!("{s}{}", " ".repeat(padding_columns)),
        Align::Right => format!("{}{s}", " ".repeat(padding_columns)),
        Align::Center => {
            let left = padding_columns / 2;
            format!(
                "{}{s}{}",
                " ".repeat(left),
                " ".repeat(padding_columns - left)
            )
        }
    }
}
