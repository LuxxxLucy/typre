// Terminal resolution and the slide geometry derived from it.

pub struct TermInfo {
    pub cols: u16,
    pub rows: u16,
    pub cell_w_px: u16,
    pub cell_h_px: u16,
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

// Zen column: a fixed left margin and a capped, left-aligned content width.
pub(crate) fn layout(term: &TermInfo) -> (usize, usize) {
    let cols = term.cols as usize;
    let margin = MARGIN.min(cols / 4);
    let content_w = cols.saturating_sub(margin * 2).min(MAX_CONTENT_W).max(1);
    (margin, content_w)
}

// Body rows the bottom status bar leaves for content.
pub fn viewport(term: &TermInfo) -> usize {
    (term.rows as usize).saturating_sub(FOOTER_RESERVE)
}
