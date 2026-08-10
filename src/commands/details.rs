use crate::core::ir::{Align, Block, RenderOp, Style};
use crate::layout::TermInfo;
use crate::render::blocks::wrap_text;
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
    let summary_lines = fit(&format!("{marker} {summary}"), avail);
    let body_lines: Vec<String> = if open {
        body.iter()
            .flat_map(|l| fit(l, avail - body_indent.min(avail - 1)))
            .map(|l| format!("{}{l}", " ".repeat(body_indent)))
            .collect()
    } else {
        Vec::new()
    };
    let content_w = summary_lines
        .iter()
        .chain(&body_lines)
        .map(|l| cell_width(l))
        .max()
        .unwrap_or(0)
        .min(avail);
    let pre = " ".repeat(indent);
    let line = |s: String, style: Style, ops: &mut Vec<RenderOp>| {
        ops.push(RenderOp::Text(format!("{pre}{s}"), style));
        ops.push(RenderOp::LineBreak);
    };
    line(hrule('┌', content_w + 2, '┐'), Style::default(), ops);
    for s in &summary_lines {
        // every row of the summary toggles, not only the first
        ops.push(RenderOp::ToggleTarget(id));
        ops.push(RenderOp::Text(format!("{pre}│ "), Style::default()));
        ops.push(RenderOp::Text(pad(s, content_w, Align::Left), heading_style()));
        ops.push(RenderOp::Text(" │".to_string(), Style::default()));
        ops.push(RenderOp::LineBreak);
    }
    for b in &body_lines {
        line(
            format!("│ {} │", pad(b, content_w, Align::Left)),
            Style::default(),
            ops,
        );
    }
    line(hrule('└', content_w + 2, '┘'), Style::default(), ops);
}

// A line that fits the box, or the lines it wraps to. Wrapping is unconditional past the
// width: a drawing wider than the box would break the frame either way, and a square frame
// reads better than a straight line running through the border.
fn fit(line: &str, w: usize) -> Vec<String> {
    if cell_width(line) <= w {
        vec![line.to_string()]
    } else {
        wrap_text(line, w)
    }
}
