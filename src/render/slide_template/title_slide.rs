use std::path::Path;

use crate::core::ir::{Block, Inline, RenderOp, Slide, Style};
use crate::layout::{layout, TermInfo};
use crate::render::inline::{disp_width, emit_inlines, uppercase_inlines};
use crate::render::paint::{dim_style, heading_style, hrule};

// Title slide: the heading sits in a bordered box at the normal slide margin and
// width; paragraphs render below the box as ordinary left-aligned dim text.
pub(crate) fn render(slide: &Slide, term: &TermInfo, deck_dir: &Path) -> Vec<RenderOp> {
    let (margin, content_w) = layout(term);
    let inner = content_w.saturating_sub(4);
    let pre = " ".repeat(margin);
    let mut ops = Vec::new();

    ops.push(RenderOp::LineBreak); // top padding
    ops.push(RenderOp::Text(
        format!("{pre}{}", hrule('┌', inner + 2, '┐')),
        Style::default(),
    ));
    ops.push(RenderOp::LineBreak);
    for block in &slide.blocks {
        let Block::Heading(_, inls) = block else {
            continue;
        };
        for line in split_breaks(&uppercase_inlines(inls)) {
            ops.push(RenderOp::Text(format!("{pre}│ "), Style::default()));
            emit_inlines(&line, heading_style(), term, deck_dir, 0, 0, &mut ops);
            let slack = inner.saturating_sub(disp_width(&line));
            ops.push(RenderOp::Text(format!("{} │", " ".repeat(slack)), Style::default()));
            ops.push(RenderOp::LineBreak);
        }
    }
    ops.push(RenderOp::Text(
        format!("{pre}{}", hrule('└', inner + 2, '┘')),
        Style::default(),
    ));
    ops.push(RenderOp::LineBreak);

    for block in &slide.blocks {
        let Block::Paragraph(inls) = block else {
            continue;
        };
        ops.push(RenderOp::LineBreak); // blank line above each paragraph
        emit_inlines(inls, dim_style(), term, deck_dir, margin, margin, &mut ops);
        ops.push(RenderOp::LineBreak);
    }
    ops
}

// Split an inline run into visual lines at soft/hard breaks.
fn split_breaks(inls: &[Inline]) -> Vec<Vec<Inline>> {
    let mut out = Vec::new();
    let mut start = 0;
    for i in 0..=inls.len() {
        if i == inls.len() || matches!(inls[i], Inline::SoftBreak | Inline::HardBreak) {
            out.push(inls[start..i].to_vec());
            start = i + 1;
        }
    }
    out
}
