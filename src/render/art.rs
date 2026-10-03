use super::blocks::{emit_block, Toggles};
use super::inline::{emit_tokens, flat_text, inline_tokens, tokens_width};
use super::paint::cell_width;
use crate::assets::Assets;
use crate::core::ir::{draws, ArtLine, ArtPart, Inline, Style};
use crate::layout::{split_lines, RenderOp, TermInfo};

// A figure two spaces in from the margin: its lines, the blocks its ◊ commands parsed to, and
// a caption under them if there is one. A nested block is rendered on its own, then every line
// of it is prefixed with the guides of the line the command sat on, so a details box under a
// `├─` branch stays under that branch.
pub(super) fn render(
    parts: &[ArtPart],
    caption: &[Inline],
    term: &TermInfo,
    assets: &Assets,
    indent: usize,
    tg: &mut Toggles,
    ops: &mut Vec<RenderOp>,
) {
    let pre = " ".repeat(indent + 2);
    let width = (term.cols as usize).saturating_sub(indent + 2).max(1);
    for part in parts {
        match part {
            ArtPart::Lines(lines) => {
                for l in lines {
                    let rows = art_rows(l, width, term, assets);
                    let cont = continue_guides(&l.guide);
                    prefix_rows(&pre, &l.guide, &cont, rows, ops);
                }
            }
            ArtPart::Nested { guide, block } => {
                let inner = term.with_cols(width.saturating_sub(cell_width(guide)));
                let mut sub = Vec::new();
                emit_block(block, &inner, assets, 0, tg, &mut sub);
                prefix_rows(&pre, guide, guide, split_lines(sub), ops);
            }
        }
    }
    if !caption.is_empty() {
        let line = ArtLine {
            guide: String::new(),
            inls: caption.to_vec(),
        };
        let rows = art_rows(&line, width, term, assets);
        prefix_rows(&pre, "", "", rows, ops);
    }
}

// The visual rows of one art line. A line that fits is drawn as
// written, and so is one that draws (a box side, an arrow), since reflowing it breaks the art.
// Prose wraps against the width available after its guides.
fn art_rows(line: &ArtLine, width: usize, term: &TermInfo, assets: &Assets) -> Vec<Vec<RenderOp>> {
    let tokens = inline_tokens(&line.inls, Style::default(), term, assets);
    let available = width.saturating_sub(cell_width(&line.guide)).max(1);
    let columns = if tokens_width(&tokens) <= available || flat_text(&line.inls).chars().any(draws)
    {
        usize::MAX
    } else {
        available
    };
    let mut ops = Vec::new();
    emit_tokens(&tokens, columns, 0, 0, &mut ops);
    split_lines(ops)
}

// Draw rows at the figure's margin: `first` leads the first row, `rest` the ones it continues
// onto. No rows at all is a blank art line, which still carries its guides.
fn prefix_rows(
    pre: &str,
    first: &str,
    rest: &str,
    rows: Vec<Vec<RenderOp>>,
    ops: &mut Vec<RenderOp>,
) {
    if rows.is_empty() {
        ops.push(RenderOp::Text(format!("{pre}{first}"), Style::default()));
        ops.push(RenderOp::LineBreak);
        return;
    }
    for (i, row) in rows.into_iter().enumerate() {
        let lead = if i == 0 { first } else { rest };
        ops.push(RenderOp::Text(format!("{pre}{lead}"), Style::default()));
        ops.extend(row);
        ops.push(RenderOp::LineBreak);
    }
}

// The prefix for the rows a wrapped art line continues onto. A guide that runs downwards
// continues as `│`; a branch or a horizontal run has already been drawn, so it goes blank.
fn continue_guides(guide: &str) -> String {
    guide
        .chars()
        .map(|c| {
            if matches!(c, '│' | '├' | '┌' | '┬' | '┼') {
                '│'
            } else {
                ' '
            }
        })
        .collect()
}
