use super::text::cell_width;
use crate::core::ir::Style;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum RenderOp {
    MoveTo(u16, u16),
    Text(String, Style),
    LineBreak,
    // One placeholder line of a virtual placement `rows` tall: `row` selects the
    // slice. A table cell emits one op per line so borders stay in the text grid.
    ImageRow {
        png_path: PathBuf,
        cols: u16,
        rows: u16,
        row: u16,
    },
    Link {
        label: String,
        url: String,
        style: Style,
    },
    // Zero-width: the row it lands on toggles details box `id` when clicked.
    ToggleTarget(usize),
    ClearImages,
}

#[derive(Debug, Clone)]
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

// Replay the cursor motion of an op stream: where on screen each op lands. Both the click
// targets and the glowing runs are read off this, so neither has to track rows itself.
pub(crate) fn positions(ops: &[RenderOp]) -> impl Iterator<Item = (u16, u16, &RenderOp)> {
    ops.iter().scan((0u16, 0u16), |(row, col), op| {
        let position = (*row, *col, op);
        match op {
            RenderOp::MoveTo(c, r) => {
                *col = *c;
                *row = *r;
            }
            RenderOp::LineBreak => {
                *row = row.saturating_add(1);
                *col = 0;
            }
            RenderOp::Text(t, _) => *col = col.saturating_add(cell_width(t) as u16),
            RenderOp::ImageRow { cols, .. } => *col = col.saturating_add(*cols),
            RenderOp::Link { label, .. } => *col = col.saturating_add(cell_width(label) as u16),
            _ => {}
        }
        Some(position)
    })
}

// A link's label span, and a details box's summary row, are what a click can land on.
pub(crate) fn hits(ops: &[RenderOp]) -> Vec<Hit> {
    positions(ops)
        .filter_map(|(row, col, op)| match op {
            RenderOp::Link { label, url, .. } => Some(Hit {
                row,
                cols: col..col + cell_width(label) as u16,
                action: HitAction::OpenUrl(url.clone()),
            }),
            RenderOp::ToggleTarget(id) => Some(Hit {
                row,
                cols: 0..u16::MAX,
                action: HitAction::ToggleDetails(*id),
            }),
            _ => None,
        })
        .collect()
}

// One run of glowing text and where it sits, so a frame can be recoloured in place without
// laying the slide out again or transmitting its images a second time.
pub struct GlowRun {
    pub row: u16,
    pub col: u16,
    pub text: String,
}

pub(crate) fn glow_runs(ops: &[RenderOp]) -> Vec<GlowRun> {
    positions(ops)
        .filter_map(|(row, col, op)| match op {
            RenderOp::Text(t, style) if style.glow => Some(GlowRun {
                row,
                col,
                text: t.clone(),
            }),
            _ => None,
        })
        .collect()
}
