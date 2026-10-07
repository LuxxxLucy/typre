use super::text::cell_width;
use super::{Hit, RenderOp};

#[derive(Default)]
pub struct Body {
    pub rows: Vec<Vec<RenderOp>>,
    pub hits: Vec<Hit>,
}

impl Body {
    pub fn into_ops(self) -> Vec<RenderOp> {
        let capacity = self.rows.iter().map(|row| row.len() + 1).sum();
        let mut ops = Vec::with_capacity(capacity);
        for row in self.rows {
            ops.extend(row);
            ops.push(RenderOp::LineBreak);
        }
        ops
    }

    pub fn height(&self) -> usize {
        self.rows.len()
    }

    pub fn window(&self, scroll: usize, viewport: usize) -> (Vec<RenderOp>, Vec<Hit>) {
        let end = scroll.saturating_add(viewport).min(self.rows.len());
        let mut ops = Vec::new();
        for row in self.rows.iter().take(end).skip(scroll) {
            ops.extend(row.iter().cloned());
            ops.push(RenderOp::LineBreak);
        }
        let hits = self
            .hits
            .iter()
            .filter_map(|hit| {
                let row = hit.row as usize;
                if row < scroll || row >= end {
                    return None;
                }
                let mut hit = hit.clone();
                hit.row = (row - scroll) as u16;
                Some(hit)
            })
            .collect();
        (ops, hits)
    }
}

pub fn split_lines(ops: Vec<RenderOp>) -> Vec<Vec<RenderOp>> {
    let mut lines = vec![Vec::new()];
    for op in ops {
        if let RenderOp::LineBreak = op {
            lines.push(Vec::new());
        } else {
            lines.last_mut().unwrap().push(op);
        }
    }
    if lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    lines
}

pub fn line_width(line: &[RenderOp]) -> usize {
    line.iter()
        .map(|op| match op {
            RenderOp::Text(t, _) => cell_width(t),
            RenderOp::ImageRow { cols, .. } => *cols as usize,
            RenderOp::Link { label, .. } => cell_width(label),
            _ => 0,
        })
        .sum()
}
