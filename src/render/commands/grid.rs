use crate::core::ir::{Align, Style};
use crate::layout::RenderOp;
use crate::layout::TermInfo;
use crate::render::paint::{cell_width, hrule, pad, truncate, wrap_text};

pub(crate) fn render(cells: &[String], term: &TermInfo, indent: usize, ops: &mut Vec<RenderOp>) {
    let avail = (term.cols as usize).saturating_sub(indent);
    if cells.is_empty() || avail == 0 {
        return;
    }
    let pre = " ".repeat(indent);
    let mut emit = |line: String| {
        ops.push(RenderOp::Text(format!("{pre}{line}"), Style::default()));
        ops.push(RenderOp::LineBreak);
    };
    if avail < 5 {
        for text in cells {
            for line in wrap_text(text, avail) {
                emit(truncate(&line, avail));
            }
        }
        return;
    }
    let columns = cells.len().min((avail + 1) / 6).max(1);
    for group in cells.chunks(columns) {
        let count = group.len();
        let inner = (avail - (count - 1) - 4 * count) / count;
        let rows: Vec<_> = group.iter().map(|text| wrap_text(text, inner)).collect();
        emit(vec![hrule('┌', inner + 2, '┐'); count].join(" "));
        for row in 0..rows.iter().map(Vec::len).max().unwrap_or(0) {
            let line = rows
                .iter()
                .map(|lines| {
                    let text = lines.get(row).map(String::as_str).unwrap_or("");
                    let text = if cell_width(text) > inner {
                        truncate(text, inner)
                    } else {
                        text.to_string()
                    };
                    format!("│ {} │", pad(&text, inner, Align::Center))
                })
                .collect::<Vec<_>>()
                .join(" ");
            emit(line);
        }
        emit(vec![hrule('└', inner + 2, '┘'); count].join(" "));
    }
}
