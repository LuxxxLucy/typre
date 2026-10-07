use crate::assets::Assets;
use crate::core::ir::{Inline, Style};
use crate::layout::RenderOp;
use crate::layout::TermInfo;
use crate::layout::{line_width, split_lines};
use crate::render::boxes::{emit_box, BoxStyle};
use crate::render::inline::emit_inlines;
use crate::render::paint::heading_style;

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
    let marker_width = 2;
    let inner_width = (term.cols as usize).saturating_sub(indent + 4).max(1);
    let text_width = inner_width.saturating_sub(marker_width).max(1);
    let layout_inlines = |inls: &[Inline], style: Style| {
        let mut inline_ops = Vec::new();
        emit_inlines(
            inls,
            style,
            &term.with_cols(text_width),
            assets,
            0,
            0,
            &mut inline_ops,
        );
        split_lines(inline_ops)
    };
    let marker = if open { "-" } else { "+" };
    let mut content_rows: Vec<Vec<RenderOp>> = Vec::new();
    for (i, row) in layout_inlines(summary, heading_style())
        .into_iter()
        .enumerate()
    {
        let marker_text = if i == 0 {
            format!("{marker} ")
        } else {
            " ".repeat(marker_width)
        };
        let mut line = vec![
            RenderOp::ToggleTarget(id),
            RenderOp::Text(marker_text, heading_style()),
        ];
        line.extend(row);
        content_rows.push(line);
    }
    if open {
        for body_line in body {
            let rows = layout_inlines(body_line, Style::default());
            if rows.is_empty() {
                content_rows.push(Vec::new());
            }
            for row in rows {
                let mut line = vec![RenderOp::Text(" ".repeat(marker_width), Style::default())];
                line.extend(row);
                content_rows.push(line);
            }
        }
    }
    let box_width = content_rows
        .iter()
        .map(|l| line_width(l))
        .max()
        .unwrap_or(0)
        .min(inner_width);
    let mut box_style = BoxStyle::bordered(box_width);
    box_style.frame.glow = !open;
    emit_box(content_rows, box_width, indent, &box_style, ops);
}
