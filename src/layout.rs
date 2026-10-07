pub mod ops;
mod rows;
pub use ops::{GlowRun, Hit, HitAction, RenderOp};
pub use rows::{line_width, split_lines, Body};

pub(crate) mod text;

pub struct TermInfo {
    pub cols: u16,
    pub rows: u16,
    pub cell_w_px: u16,
    pub cell_h_px: u16,
}

impl TermInfo {
    pub(crate) fn with_cols(&self, cols: usize) -> TermInfo {
        TermInfo {
            cols: cols as u16,
            rows: self.rows,
            cell_w_px: self.cell_w_px,
            cell_h_px: self.cell_h_px,
        }
    }
}

pub(crate) fn natural_ppi(term: &TermInfo) -> u32 {
    const PPI_PER_CELL_HEIGHT_PIXEL: u32 = 9;
    const MIN_IMAGE_PPI: u32 = 72;
    (term.cell_h_px as u32 * PPI_PER_CELL_HEIGHT_PIXEL).max(MIN_IMAGE_PPI)
}

pub(crate) const FOOTER_RESERVE: usize = 3;

const MAX_CONTENT_COLUMNS: usize = 90;
const MIN_SIDE_MARGIN: usize = 4;

pub(crate) fn layout(term: &TermInfo) -> (usize, usize) {
    let cols = term.cols as usize;
    let content_width = cols
        .saturating_sub(MIN_SIDE_MARGIN * 2)
        .clamp(1, MAX_CONTENT_COLUMNS);
    let margin = cols.saturating_sub(content_width) / 2;
    (margin, content_width)
}

pub fn viewport(term: &TermInfo) -> usize {
    (term.rows as usize).saturating_sub(FOOTER_RESERVE)
}
