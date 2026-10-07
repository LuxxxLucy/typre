use crate::assets::Assets;
use crate::core::ir::{plain_text as flat_text, Align, Inline, Style, Width};
#[cfg(test)]
use crate::layout::{line_width, split_lines};
use crate::layout::{natural_ppi, RenderOp, TermInfo};
use crate::render::paint::{
    cell_width, heading_style, image_cells, indent_op, pad, place_image, wrap_text,
};
use std::path::PathBuf;

#[derive(Default)]
struct TableEntry {
    text: String,
    figure: Option<TableFigure>,
}

struct TableFigure {
    png: PathBuf,
    width: Width,
    cols: u16,
    rows: u16,
}

fn prepare_figure(
    inls: &[Inline],
    term: &TermInfo,
    assets: &Assets,
    available_columns: usize,
) -> Option<TableFigure> {
    let (src, width) = inls.iter().find_map(|i| match i {
        Inline::InlineTypst { src, width, .. } => Some((src, *width)),
        _ => None,
    })?;
    let png = assets.fragment(src, natural_ppi(term), true).ok()?;
    let (cols, rows) = image_cells(
        assets.dimensions(&png),
        &term.with_cols(available_columns),
        0,
        width,
    );
    Some(TableFigure {
        png,
        width,
        cols,
        rows,
    })
}

pub(super) fn render(
    aligns: &[Align],
    head: &[Vec<Inline>],
    rows: &[Vec<Vec<Inline>>],
    term: &TermInfo,
    assets: &Assets,
    indent: usize,
    ops: &mut Vec<RenderOp>,
) {
    let column_count = head.len().max(rows.iter().map(Vec::len).max().unwrap_or(0));
    if column_count == 0 {
        return;
    }
    let column_alignment = |column: usize| aligns.get(column).copied().unwrap_or(Align::Left);
    let content_width = (term.cols as usize).saturating_sub(indent).max(1);
    let prepare_entry = |inls: Option<&Vec<Inline>>| -> TableEntry {
        let inls = match inls {
            Some(i) => i,
            None => return TableEntry::default(),
        };
        TableEntry {
            text: flat_text(inls),
            figure: prepare_figure(inls, term, assets, content_width),
        }
    };
    let mut header_entries: Vec<TableEntry> = (0..column_count)
        .map(|column| prepare_entry(head.get(column)))
        .collect();
    let mut body_entries: Vec<Vec<TableEntry>> = rows
        .iter()
        .map(|row| {
            (0..column_count)
                .map(|column| prepare_entry(row.get(column)))
                .collect()
        })
        .collect();

    let frame_columns = 1 + 3 * column_count;
    if content_width < frame_columns + column_count {
        for (index, row) in std::iter::once(&header_entries)
            .chain(&body_entries)
            .enumerate()
        {
            if index == 0 && head.iter().all(Vec::is_empty) {
                continue;
            }
            let style = if index == 0 {
                heading_style()
            } else {
                Style::default()
            };
            for entry in row {
                if let Some(figure) = &entry.figure {
                    place_image(ops, figure.png.clone(), assets, term, indent, figure.width);
                }
                for line in wrap_text(&entry.text, content_width) {
                    ops.push(indent_op(indent));
                    ops.push(RenderOp::Text(line, style));
                    ops.push(RenderOp::LineBreak);
                }
            }
            ops.push(RenderOp::LineBreak);
        }
        return;
    }

    let mut column_widths = vec![0usize; column_count];
    for column in 0..column_count {
        let mut width = cell_width(&header_entries[column].text);
        width = width.max(
            header_entries[column]
                .figure
                .as_ref()
                .map_or(0, |figure| figure.cols as usize),
        );
        for row in &body_entries {
            width = width.max(cell_width(&row[column].text));
            width = width.max(
                row[column]
                    .figure
                    .as_ref()
                    .map_or(0, |figure| figure.cols as usize),
            );
        }
        column_widths[column] = width;
    }

    let budget = (term.cols as usize)
        .saturating_sub(indent + frame_columns)
        .max(column_count);
    fit_widths(&mut column_widths, budget);

    for column in 0..column_count {
        for entry in std::iter::once(&mut header_entries[column])
            .chain(body_entries.iter_mut().map(|row| &mut row[column]))
        {
            if let Some(figure) = entry.figure.as_mut() {
                let (cols, rows) = image_cells(
                    assets.dimensions(&figure.png),
                    &term.with_cols(column_widths[column]),
                    0,
                    figure.width,
                );
                figure.cols = cols;
                figure.rows = rows;
            }
        }
    }

    let indent_text = " ".repeat(indent);
    let border = |left: char, junction: char, right: char| {
        let mut line = String::new();
        line.push(left);
        for (column, width) in column_widths.iter().enumerate() {
            line.push_str(&"─".repeat(width + 2));
            line.push(if column + 1 == column_count {
                right
            } else {
                junction
            });
        }
        line
    };
    let push_line = |s: String, ops: &mut Vec<RenderOp>| {
        ops.push(RenderOp::Text(
            format!("{indent_text}{s}"),
            Style::default(),
        ));
        ops.push(RenderOp::LineBreak);
    };
    let emit_row = |entries: &[TableEntry], style: Style, ops: &mut Vec<RenderOp>| {
        let wrapped_text: Vec<Vec<String>> = (0..column_count)
            .map(|column| wrap_text(&entries[column].text, column_widths[column]))
            .collect();
        let figure_rows = |column: usize| {
            entries[column]
                .figure
                .as_ref()
                .map_or(0, |figure| figure.rows as usize)
        };
        let height = (0..column_count)
            .map(|column| figure_rows(column) + wrapped_text[column].len())
            .max()
            .unwrap_or(1)
            .max(1);
        for row in 0..height {
            ops.push(RenderOp::Text(indent_text.clone(), Style::default()));
            for column in 0..column_count {
                ops.push(RenderOp::Text("│ ".to_string(), Style::default()));
                if row < figure_rows(column) {
                    let figure = entries[column].figure.as_ref().unwrap();
                    let figure_padding = column_widths[column].saturating_sub(figure.cols as usize);
                    let left = figure_padding / 2;
                    ops.push(RenderOp::Text(" ".repeat(left), style));
                    ops.push(RenderOp::ImageRow {
                        png_path: figure.png.clone(),
                        cols: figure.cols,
                        rows: figure.rows,
                        row: row as u16,
                    });
                    ops.push(RenderOp::Text(" ".repeat(figure_padding - left + 1), style));
                } else {
                    let text_line = wrapped_text[column]
                        .get(row - figure_rows(column))
                        .map(String::as_str)
                        .unwrap_or("");
                    ops.push(RenderOp::Text(
                        format!(
                            "{} ",
                            pad(text_line, column_widths[column], column_alignment(column))
                        ),
                        style,
                    ));
                }
            }
            ops.push(RenderOp::Text("│".to_string(), Style::default()));
            ops.push(RenderOp::LineBreak);
        }
    };

    let row_separator = {
        let mut s = String::new();
        for width in &column_widths {
            s.push_str("│ ");
            s.push_str(&" ".repeat(width + 1));
        }
        s.push('│');
        s
    };

    push_line(border('┌', '┬', '┐'), ops);
    if head.iter().any(|entry| !entry.is_empty()) {
        emit_row(&header_entries, heading_style(), ops);
        push_line(border('├', '┼', '┤'), ops);
    }
    for (i, row) in body_entries.iter().enumerate() {
        if i > 0 {
            push_line(row_separator.clone(), ops);
        }
        emit_row(row, Style::default(), ops);
    }
    push_line(border('└', '┴', '┘'), ops);
}

fn fit_widths(column_widths: &mut [usize], budget: usize) {
    if column_widths.iter().sum::<usize>() <= budget {
        return;
    }
    let (mut low, mut high) = (0, column_widths.iter().copied().max().unwrap_or(0));
    while low < high {
        let cap = low + (high - low).div_ceil(2);
        if column_widths
            .iter()
            .map(|width| (*width).min(cap))
            .sum::<usize>()
            <= budget
        {
            low = cap;
        } else {
            high = cap - 1;
        }
    }
    let mut spare = budget
        - column_widths
            .iter()
            .map(|width| (*width).min(low))
            .sum::<usize>();
    for width in column_widths {
        if *width > low {
            *width = low;
            if spare > 0 {
                *width += 1;
                spare -= 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn narrow_tables_keep_all_text_within_the_terminal() {
        let head = vec![vec![Inline::Text("head".into(), Style::default())]; 3];
        let rows = vec![vec![
            vec![Inline::Text(
                "abcdefgh中文".into(),
                Style::default()
            )];
            3
        ]];
        let assets = Assets::new(std::path::Path::new("."));
        for cols in 1..20 {
            let term = TermInfo {
                cols,
                rows: 30,
                cell_w_px: 8,
                cell_h_px: 16,
            };
            let mut ops = Vec::new();
            render(&[], &head, &rows, &term, &assets, 0, &mut ops);
            assert!(
                split_lines(ops)
                    .iter()
                    .all(|line| line_width(line) <= cols as usize),
                "columns={cols}"
            );
        }
    }

    #[test]
    fn table_widths_preserve_existing_distribution() {
        for a in 0..8 {
            for b in 0..8 {
                for column in 0..8 {
                    for budget in 3..24 {
                        let mut expected = vec![a, b, column];
                        while expected.iter().sum::<usize>() > budget {
                            let widest = (0..3).max_by_key(|&i| expected[i]).unwrap();
                            if expected[widest] <= 1 {
                                break;
                            }
                            expected[widest] -= 1;
                        }
                        let mut actual = vec![a, b, column];
                        fit_widths(&mut actual, budget);
                        assert_eq!(
                            actual, expected,
                            "column_widths={a},{b},{column}; budget={budget}"
                        );
                    }
                }
            }
        }
    }
}
