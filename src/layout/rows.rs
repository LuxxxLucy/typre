use super::{Hit, RenderOp};

#[derive(Default)]
pub struct Body {
    pub rows: Vec<Vec<RenderOp>>,
    pub hits: Vec<Hit>,
}

impl Body {
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
