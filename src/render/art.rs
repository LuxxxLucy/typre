use super::blocks::{emit_block, DetailsState};
use super::inline::{emit_tokens, flat_text, inline_tokens, tokens_width};
use super::paint::cell_width;
use crate::assets::Assets;
use crate::core::ir::{draws, ArtLine, ArtPart, Inline, Style};
use crate::layout::{split_lines, RenderOp, TermInfo};

pub(super) fn render(
    parts: &[ArtPart],
    caption: &[Inline],
    term: &TermInfo,
    assets: &Assets,
    indent: usize,
    details: &mut DetailsState,
    ops: &mut Vec<RenderOp>,
) {
    let margin_prefix = " ".repeat(indent + 2);
    let width = (term.cols as usize).saturating_sub(indent + 2).max(1);
    for part in parts {
        match part {
            ArtPart::Lines(lines) => {
                for line in lines {
                    let rows = art_rows(line, width, term, assets);
                    let continuation_guide = continue_guides(&line.guide);
                    prefix_rows(&margin_prefix, &line.guide, &continuation_guide, rows, ops);
                }
            }
            ArtPart::Nested { guide, block } => {
                let nested_term = term.with_cols(width.saturating_sub(cell_width(guide)));
                let mut nested_ops = Vec::new();
                emit_block(block, &nested_term, assets, 0, details, &mut nested_ops);
                prefix_rows(&margin_prefix, guide, guide, split_lines(nested_ops), ops);
            }
        }
    }
    if !caption.is_empty() {
        let line = ArtLine {
            guide: String::new(),
            inls: caption.to_vec(),
        };
        let rows = art_rows(&line, width, term, assets);
        prefix_rows(&margin_prefix, "", "", rows, ops);
    }
}

fn art_rows(line: &ArtLine, width: usize, term: &TermInfo, assets: &Assets) -> Vec<Vec<RenderOp>> {
    let tokens = inline_tokens(&line.inls, Style::default(), term, assets);
    let text_columns = width.saturating_sub(cell_width(&line.guide)).max(1);
    let contains_drawing = flat_text(&line.inls).chars().any(draws);
    let columns = if tokens_width(&tokens) <= text_columns || contains_drawing {
        usize::MAX
    } else {
        text_columns
    };
    let mut ops = Vec::new();
    emit_tokens(&tokens, columns, 0, 0, &mut ops);
    split_lines(ops)
}

fn prefix_rows(
    margin_prefix: &str,
    first_guide: &str,
    continuation_guide: &str,
    rows: Vec<Vec<RenderOp>>,
    ops: &mut Vec<RenderOp>,
) {
    if rows.is_empty() {
        ops.push(RenderOp::Text(
            format!("{margin_prefix}{first_guide}"),
            Style::default(),
        ));
        ops.push(RenderOp::LineBreak);
        return;
    }
    for (i, row) in rows.into_iter().enumerate() {
        let guide = if i == 0 {
            first_guide
        } else {
            continuation_guide
        };
        ops.push(RenderOp::Text(
            format!("{margin_prefix}{guide}"),
            Style::default(),
        ));
        ops.extend(row);
        ops.push(RenderOp::LineBreak);
    }
}

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
