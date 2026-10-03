use crate::assets::Assets;
use crate::core::ir::{Inline, Style};
use crate::layout::RenderOp;
use crate::layout::TermInfo;
use crate::layout::{line_width, split_lines};
use crate::render::boxes::{emit_box, BoxDeco};
use crate::render::inline::emit_inlines;
use crate::render::paint::heading_style;

// A collapsible details box, sized to its widest line but never past the column. Summary and
// body wrap to fit, every summary row carries the click target for `id`, the body is drawn
// only when open, and a `+`/`-` marker shows the state.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render(
    id: usize,
    open: bool,
    summary: &[Inline],
    body: &[Vec<Inline>],
    term: &TermInfo,
    assets: &Assets,
    indent: usize,
    ops: &mut Vec<RenderOp>,
) {
    // A row costs the indent, the two border characters and their two spaces of inset, and
    // the marker column the body aligns past.
    let lead = 2;
    let avail = (term.cols as usize).saturating_sub(indent + 4).max(1);
    let text = avail.saturating_sub(lead).max(1);
    let lay = |inls: &[Inline], style: Style| {
        let mut sub = Vec::new();
        emit_inlines(inls, style, &term.with_cols(text), assets, 0, 0, &mut sub);
        split_lines(sub)
    };
    let marker = if open { "-" } else { "+" };
    let mut vlines: Vec<Vec<RenderOp>> = Vec::new();
    for (i, row) in lay(summary, heading_style()).into_iter().enumerate() {
        // every row of the summary toggles, not only the first
        let mark = if i == 0 {
            format!("{marker} ")
        } else {
            " ".repeat(lead)
        };
        let mut line = vec![
            RenderOp::ToggleTarget(id),
            RenderOp::Text(mark, heading_style()),
        ];
        line.extend(row);
        vlines.push(line);
    }
    if open {
        for b in body {
            // A blank body line lays out to no rows, and stands as a blank row of the box.
            let rows = lay(b, Style::default());
            if rows.is_empty() {
                vlines.push(Vec::new());
            }
            for row in rows {
                let mut line = vec![RenderOp::Text(" ".repeat(lead), Style::default())];
                line.extend(row);
                vlines.push(line);
            }
        }
    }
    let inner = vlines
        .iter()
        .map(|l| line_width(l))
        .max()
        .unwrap_or(0)
        .min(avail);
    let mut deco = BoxDeco::bordered(inner);
    // a closed box shimmers, an open one is a plain frame
    deco.frame.glow = !open;
    emit_box(vlines, inner, indent, &deco, ops);
}
