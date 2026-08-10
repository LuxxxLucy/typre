use crate::core::ir::{Align, Block, RenderOp, Style};
use crate::layout::TermInfo;
use crate::render::paint::{cell_width, heading_style, hrule, pad};

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

// A collapsible details box. Its summary row carries the click target for `id`; the body is
// drawn only when open, and a `⊕`/`⊖` marker shows the state.
pub(crate) fn render(
    id: usize,
    open: bool,
    summary: &str,
    body: &[String],
    term: &TermInfo,
    indent: usize,
    ops: &mut Vec<RenderOp>,
) {
    let avail = (term.cols as usize).saturating_sub(indent + 2);
    let marker = if open { "⊖" } else { "⊕" };
    let summary_line = format!("{marker} {summary}");
    // body lines align under the summary text, past the marker and its space
    let body_indent = 2;
    let mut widest = cell_width(&summary_line);
    if open {
        widest = widest.max(
            body.iter()
                .map(|l| cell_width(l) + body_indent)
                .max()
                .unwrap_or(0),
        );
    }
    let content_w = widest.min(avail);
    let pre = " ".repeat(indent);
    let line = |s: String, style: Style, ops: &mut Vec<RenderOp>| {
        ops.push(RenderOp::Text(format!("{pre}{s}"), style));
        ops.push(RenderOp::LineBreak);
    };
    line(hrule('┌', content_w + 2, '┐'), Style::default(), ops);
    ops.push(RenderOp::ToggleTarget(id));
    ops.push(RenderOp::Text(format!("{pre}│ "), Style::default()));
    ops.push(RenderOp::Text(
        pad(&summary_line, content_w, Align::Left),
        heading_style(),
    ));
    ops.push(RenderOp::Text(" │".to_string(), Style::default()));
    ops.push(RenderOp::LineBreak);
    if open {
        for b in body {
            line(
                format!(
                    "│ {} │",
                    pad(&format!("{}{b}", " ".repeat(body_indent)), content_w, Align::Left)
                ),
                Style::default(),
                ops,
            );
        }
    }
    line(hrule('└', content_w + 2, '┘'), Style::default(), ops);
}
