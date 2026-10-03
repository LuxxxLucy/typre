use super::paint::{hrule, indent_op};
use crate::core::ir::Style;
use crate::layout::{line_width, RenderOp};

// A single-column box: content wrapped into `vlines` framed at width `inner`, with
// a top and bottom row. `deco` decides the framing (border vs background fill); the
// left margin `indent` stays default-styled outside the box.
pub(crate) struct BoxDeco {
    pub top: String,
    pub bottom: String,
    pub left: String,
    pub right: String,
    pub frame: Style,
    pub content_bg: Style,
}

impl BoxDeco {
    // A single-line frame with one space of inset each side.
    pub(crate) fn bordered(inner: usize) -> BoxDeco {
        BoxDeco {
            top: hrule('┌', inner + 2, '┐'),
            bottom: hrule('└', inner + 2, '┘'),
            left: "│ ".to_string(),
            right: " │".to_string(),
            frame: Style::default(),
            content_bg: Style::default(),
        }
    }

    // No frame, `style` as the background of the whole box, one space of inset each side.
    pub(crate) fn shaded(inner: usize, style: Style) -> BoxDeco {
        BoxDeco {
            top: " ".repeat(inner + 2),
            bottom: " ".repeat(inner + 2),
            left: " ".to_string(),
            right: " ".to_string(),
            frame: style,
            content_bg: style,
        }
    }
}

pub(crate) fn emit_box(
    vlines: Vec<Vec<RenderOp>>,
    inner: usize,
    indent: usize,
    deco: &BoxDeco,
    ops: &mut Vec<RenderOp>,
) {
    let row = |body: &str, ops: &mut Vec<RenderOp>| {
        ops.push(indent_op(indent));
        ops.push(RenderOp::Text(body.to_string(), deco.frame));
        ops.push(RenderOp::LineBreak);
    };
    row(&deco.top, ops);
    for line in vlines {
        let w = line_width(&line);
        ops.push(indent_op(indent));
        ops.push(RenderOp::Text(deco.left.clone(), deco.frame));
        for op in line {
            ops.push(shade(op, deco.content_bg));
        }
        ops.push(RenderOp::Text(
            format!("{}{}", " ".repeat(inner.saturating_sub(w)), deco.right),
            deco.frame,
        ));
        ops.push(RenderOp::LineBreak);
    }
    row(&deco.bottom, ops);
}

// Add the box background to a text or link op; other ops pass through unstyled.
fn shade(op: RenderOp, bg: Style) -> RenderOp {
    match op {
        RenderOp::Text(t, s) => RenderOp::Text(
            t,
            Style {
                quote: bg.quote,
                ..s
            },
        ),
        RenderOp::Link { label, url, style } => RenderOp::Link {
            label,
            url,
            style: Style {
                quote: bg.quote,
                ..style
            },
        },
        other => other,
    }
}
