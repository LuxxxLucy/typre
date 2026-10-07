use super::paint::{hrule, indent_op};
use crate::core::ir::Style;
use crate::layout::{line_width, RenderOp};

pub(crate) struct BoxStyle {
    pub top: String,
    pub bottom: String,
    pub left: String,
    pub right: String,
    pub frame: Style,
    pub content_style: Style,
}

impl BoxStyle {
    pub(crate) fn bordered(content_width: usize) -> BoxStyle {
        BoxStyle {
            top: hrule('┌', content_width + 2, '┐'),
            bottom: hrule('└', content_width + 2, '┘'),
            left: "│ ".to_string(),
            right: " │".to_string(),
            frame: Style::default(),
            content_style: Style::default(),
        }
    }

    pub(crate) fn shaded(content_width: usize, style: Style) -> BoxStyle {
        BoxStyle {
            top: " ".repeat(content_width + 2),
            bottom: " ".repeat(content_width + 2),
            left: " ".to_string(),
            right: " ".to_string(),
            frame: style,
            content_style: style,
        }
    }
}

pub(crate) fn emit_box(
    content_rows: Vec<Vec<RenderOp>>,
    content_width: usize,
    indent: usize,
    box_style: &BoxStyle,
    ops: &mut Vec<RenderOp>,
) {
    let row = |body: &str, ops: &mut Vec<RenderOp>| {
        ops.push(indent_op(indent));
        ops.push(RenderOp::Text(body.to_string(), box_style.frame));
        ops.push(RenderOp::LineBreak);
    };
    row(&box_style.top, ops);
    for line in content_rows {
        let row_width = line_width(&line);
        ops.push(indent_op(indent));
        ops.push(RenderOp::Text(box_style.left.clone(), box_style.frame));
        for op in line {
            ops.push(apply_background(op, box_style.content_style));
        }
        ops.push(RenderOp::Text(
            format!(
                "{}{}",
                " ".repeat(content_width.saturating_sub(row_width)),
                box_style.right
            ),
            box_style.frame,
        ));
        ops.push(RenderOp::LineBreak);
    }
    row(&box_style.bottom, ops);
}

fn apply_background(op: RenderOp, background: Style) -> RenderOp {
    match op {
        RenderOp::Text(t, s) => RenderOp::Text(
            t,
            Style {
                quote: background.quote,
                ..s
            },
        ),
        RenderOp::Link { label, url, style } => RenderOp::Link {
            label,
            url,
            style: Style {
                quote: background.quote,
                ..style
            },
        },
        other => other,
    }
}
