use crate::core::ir::{Block, RenderOp, Style};
use crate::layout::TermInfo;
use crate::render::blocks::{emit_box, line_width, BoxDeco};
use crate::render::paint::{fit, heading_style};

use super::{bracket_cmd, Frag};

pub(crate) fn parse(after: &str) -> Option<(Frag, usize)> {
    let (summary, body, used) = bracket_cmd(after, "details")?;
    Some((Frag::Block(build(&summary, &body)), used))
}

fn build(summary: &str, body: &str) -> Block {
    Block::Details {
        summary: summary.to_string(),
        body: body.trim_matches('\n').lines().map(str::to_string).collect(),
    }
}

// A collapsible details box, sized to its widest line but never past the column. Summary and
// body wrap to fit, every summary row carries the click target for `id`, the body is drawn
// only when open, and a `⊕`/`⊖` marker shows the state.
pub(crate) fn render(
    id: usize,
    open: bool,
    summary: &str,
    body: &[String],
    term: &TermInfo,
    indent: usize,
    ops: &mut Vec<RenderOp>,
) {
    // A row costs the indent, the two border characters and their two spaces of inset.
    let avail = (term.cols as usize).saturating_sub(indent + 4).max(1);
    let marker = if open { "⊖" } else { "⊕" };
    // body lines align under the summary text, past the marker and its space
    let body_indent = 2;
    let mut vlines: Vec<Vec<RenderOp>> = Vec::new();
    for s in fit(&format!("{marker} {summary}"), avail) {
        // every row of the summary toggles, not only the first
        vlines.push(vec![
            RenderOp::ToggleTarget(id),
            RenderOp::Text(s, heading_style()),
        ]);
    }
    if open {
        let pre = " ".repeat(body_indent);
        for b in body {
            for l in fit(b, avail.saturating_sub(body_indent).max(1)) {
                vlines.push(vec![RenderOp::Text(format!("{pre}{l}"), Style::default())]);
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
