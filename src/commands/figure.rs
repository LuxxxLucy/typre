use crate::core::ir::{Block, RenderOp, Style};

use super::{bracket_cmd, Frag};

pub(crate) fn parse(after: &str) -> Option<(Frag, usize)> {
    let (caption, body, used) = bracket_cmd(after, "figure")?;
    Some((Frag::Block(build(&caption, &body)), used))
}

fn build(caption: &str, body: &str) -> Block {
    Block::Figure {
        body: body.trim_matches('\n').to_string(),
        caption: caption.to_string(),
    }
}

pub(crate) fn render(body: &str, caption: &str, indent: usize, ops: &mut Vec<RenderOp>) {
    let pre = " ".repeat(indent + 2);
    for line in body.lines() {
        ops.push(RenderOp::Text(format!("{pre}{line}"), Style::default()));
        ops.push(RenderOp::LineBreak);
    }
    if !caption.is_empty() {
        ops.push(RenderOp::Text(
            format!("{pre}{caption}"),
            Style {
                italic: true,
                ..Style::default()
            },
        ));
        ops.push(RenderOp::LineBreak);
    }
}
