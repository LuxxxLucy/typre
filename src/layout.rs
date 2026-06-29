// Terminal resolution and the slide geometry derived from it.

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

// Match typst raster resolution to the terminal cell height, so math sits at text
// line-height and stays crisp on any display (144 ppi at a 16px cell).
pub(crate) fn natural_ppi(term: &TermInfo) -> u32 {
    (term.cell_h_px as u32 * 9).max(72)
}

// Rows the bottom status bar occupies (separator, footer, and a blank gap above).
pub(crate) const FOOTER_RESERVE: usize = 3;

const MAX_CONTENT_W: usize = 90;
const MARGIN: usize = 4;

// Zen column: a capped content width centered in the terminal, never closer than
// MARGIN to either edge. On a wide terminal the slack splits evenly left and right.
pub(crate) fn layout(term: &TermInfo) -> (usize, usize) {
    let cols = term.cols as usize;
    let content_w = cols.saturating_sub(MARGIN * 2).min(MAX_CONTENT_W).max(1);
    let margin = cols.saturating_sub(content_w) / 2;
    (margin, content_w)
}

// Body rows the bottom status bar leaves for content.
pub fn viewport(term: &TermInfo) -> usize {
    (term.rows as usize).saturating_sub(FOOTER_RESERVE)
}
