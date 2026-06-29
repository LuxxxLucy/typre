// Drawing vocabulary: click targets, render-op helpers, styles, and box rules.
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::core::ir::{Align, RenderOp, Style, Width};
use crate::layout::{viewport, TermInfo};

// A left-click target: the screen row, the column span it covers, and what the
// click does.
#[derive(Debug)]
pub struct Hit {
    pub row: u16,
    pub cols: std::ops::Range<u16>,
    pub action: HitAction,
}

#[derive(Debug, Clone)]
pub enum HitAction {
    ToggleDetails(usize),
    OpenUrl(String),
    Goto(usize),
}

pub(crate) fn indent_op(indent: usize) -> RenderOp {
    RenderOp::Text(" ".repeat(indent), Style::default())
}

// A block image is placed with kitty C=1 (the cursor does not move), so advance
// past it by its row count to keep the next block from overlapping it.
pub(crate) fn advance_rows(ops: &mut Vec<RenderOp>, rows: u16) {
    for _ in 0..rows {
        ops.push(RenderOp::LineBreak);
    }
}

// Current cursor row in the body flow: one per line break emitted so far.
pub(crate) fn current_row(ops: &[RenderOp]) -> usize {
    ops.iter()
        .filter(|o| matches!(o, RenderOp::LineBreak))
        .count()
}

// Size an image in cells. Natural shrinks to fit the column; Percent/Cols target
// that width. Both axes scale by one factor (aspect preserved), bounded by
// `avail_rows` so the image clears the footer.
pub(crate) fn image_cells(
    png_path: &Path,
    term: &TermInfo,
    indent: usize,
    width: Width,
    avail_rows: usize,
) -> (u16, u16) {
    let cell_w = term.cell_w_px.max(1) as f32;
    let cell_h = term.cell_h_px.max(1) as f32;
    let (w, h) = image_dims(png_path).unwrap_or((cell_w as u32, cell_h as u32));
    let nat_cols = (w as f32 / cell_w).max(1.0);
    let nat_rows = (h as f32 / cell_h).max(1.0);
    let content_w = (term.cols as usize).saturating_sub(indent).max(1) as f32;
    let max_rows = avail_rows.max(1) as f32;
    let target_cols = match width {
        Width::Natural => nat_cols.min(content_w),
        Width::Percent(p) => content_w * (p as f32 / 100.0),
        Width::Cols(c) => (c as f32).min(content_w),
    };
    let mut scale = target_cols / nat_cols;
    if nat_rows * scale > max_rows {
        scale = max_rows / nat_rows;
    }
    let cols = (nat_cols * scale).round().max(1.0) as u16;
    let rows = (nat_rows * scale).round().max(1.0) as u16;
    (cols, rows)
}

// Image pixel dimensions, memoized. PNG cache paths are content-hashed, so a path
// maps to fixed bytes for the process lifetime and never needs re-reading.
pub(crate) fn image_dims(png_path: &Path) -> Option<(u32, u32)> {
    thread_local! {
        static CACHE: RefCell<HashMap<PathBuf, (u32, u32)>> = RefCell::new(HashMap::new());
    }
    if let Some(d) = CACHE.with(|c| c.borrow().get(png_path).copied()) {
        return Some(d);
    }
    let d = image::image_dimensions(png_path).ok()?;
    CACHE.with(|c| c.borrow_mut().insert(png_path.to_path_buf(), d));
    Some(d)
}

// Size a block image, place it at the indent, and advance past its rows.
pub(crate) fn place_image(
    ops: &mut Vec<RenderOp>,
    png_path: PathBuf,
    term: &TermInfo,
    indent: usize,
    width: Width,
) -> (u16, u16) {
    let avail = viewport(term).saturating_sub(current_row(ops));
    let (cols, rows) = image_cells(&png_path, term, indent, width, avail);
    ops.push(indent_op(indent));
    ops.push(RenderOp::Image { png_path, cols, rows });
    advance_rows(ops, rows);
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

pub(crate) fn code_style() -> Style {
    Style {
        code: true,
        ..Style::default()
    }
}

// A horizontal box rule: a left corner, `w` dashes, a right corner.
pub(crate) fn hrule(left: char, w: usize, right: char) -> String {
    format!("{left}{}{right}", "─".repeat(w))
}

pub(crate) fn pad(s: &str, w: usize, align: Align) -> String {
    let len = s.chars().count();
    if len >= w {
        return s.to_string();
    }
    let slack = w - len;
    match align {
        Align::Left => format!("{s}{}", " ".repeat(slack)),
        Align::Right => format!("{}{s}", " ".repeat(slack)),
        Align::Center => {
            let left = slack / 2;
            format!("{}{s}{}", " ".repeat(left), " ".repeat(slack - left))
        }
    }
}
