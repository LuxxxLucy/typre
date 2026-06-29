use crate::core::ir::{Align, Block, RenderOp, Style};
use crate::layout::TermInfo;
use crate::render::paint::{current_row, heading_style, hrule, pad, Hit, HitAction};

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

// A collapsible details box. `id` is Some for a top-level toggle (returns its
// click target) and None for an always-open nested box (no target). The body is
// drawn only when open; a `▸`/`▾` marker shows the state.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render(
    id: Option<usize>,
    open: bool,
    summary: &str,
    body: &[String],
    term: &TermInfo,
    indent: usize,
    ops: &mut Vec<RenderOp>,
) -> Option<Hit> {
    let avail = (term.cols as usize).saturating_sub(indent + 2);
    let marker = if open { "▾" } else { "▸" };
    let summary_line = format!("{marker} {summary}");
    // body lines align under the summary text, past the marker and its space
    let body_indent = 2;
    let mut widest = summary_line.chars().count();
    if open {
        widest = widest.max(
            body.iter()
                .map(|l| l.chars().count() + body_indent)
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
    let hit = id.map(|id| Hit {
        row: current_row(ops) as u16,
        cols: 0..u16::MAX,
        action: HitAction::ToggleDetails(id),
    });
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
    hit
}
