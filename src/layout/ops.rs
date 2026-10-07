use super::text::cell_width;
use crate::core::ir::Style;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum RenderOp {
    MoveTo(u16, u16),
    Text(String, Style),
    LineBreak,
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
