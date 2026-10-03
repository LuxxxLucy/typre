use crate::assets::Assets;
use crate::core::ir::{plain_text as flat_text, Align, Inline, Style, Width};
#[cfg(test)]
use crate::layout::{line_width, split_lines};
use crate::layout::{natural_ppi, RenderOp, TermInfo};
use crate::render::paint::{
    cell_width, heading_style, image_cells, indent_op, pad, place_image, wrap_text,
};
use std::path::PathBuf;

// A table cell: the text it wraps, and the figure a `◊typst` or `◊width` command in
// it renders to. The figure takes the top lines of the cell, the text wraps under it.
#[derive(Default)]
struct Cell {
    text: String,
    fig: Option<Fig>,
}

struct Fig {
    png: PathBuf,
    width: Width,
    cols: u16,
    rows: u16,
}

// Render the first typst fragment in the cell and size it against `avail` columns.
fn cell_fig(inls: &[Inline], term: &TermInfo, assets: &Assets, avail: usize) -> Option<Fig> {
    let (src, width) = inls.iter().find_map(|i| match i {
        Inline::InlineTypst { src, width, .. } => Some((src, *width)),
        _ => None,
    })?;
    let png = assets.fragment(src, natural_ppi(term), true).ok()?;
    let (cols, rows) = image_cells(assets.dimensions(&png), &term.with_cols(avail), 0, width);
    Some(Fig {
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
    let ncol = head.len().max(rows.iter().map(Vec::len).max().unwrap_or(0));
    if ncol == 0 {
        return;
    }
    let align_of = |c: usize| aligns.get(c).copied().unwrap_or(Align::Left);
    let content_w = (term.cols as usize).saturating_sub(indent).max(1);
    let build = |inls: Option<&Vec<Inline>>| -> Cell {
        let inls = match inls {
            Some(i) => i,
            None => return Cell::default(),
        };
        Cell {
            text: flat_text(inls),
            fig: cell_fig(inls, term, assets, content_w),
        }
    };
    let mut head_cells: Vec<Cell> = (0..ncol).map(|c| build(head.get(c))).collect();
    let mut body_cells: Vec<Vec<Cell>> = rows
        .iter()
        .map(|row| (0..ncol).map(|c| build(row.get(c))).collect())
        .collect();

    if content_w < 1 + 4 * ncol {
        for (index, row) in std::iter::once(&head_cells).chain(&body_cells).enumerate() {
            if index == 0 && head.iter().all(Vec::is_empty) {
                continue;
            }
            let style = if index == 0 {
                heading_style()
            } else {
                Style::default()
            };
            for cell in row {
                if let Some(fig) = &cell.fig {
                    place_image(ops, fig.png.clone(), assets, term, indent, fig.width);
                }
                for line in wrap_text(&cell.text, content_w) {
                    ops.push(indent_op(indent));
                    ops.push(RenderOp::Text(line, style));
                    ops.push(RenderOp::LineBreak);
                }
            }
            ops.push(RenderOp::LineBreak);
        }
        return;
    }

    // A column is as wide as its widest text, or its widest figure.
    let mut widths = vec![0usize; ncol];
    for c in 0..ncol {
        let mut w = cell_width(&head_cells[c].text);
        w = w.max(head_cells[c].fig.as_ref().map_or(0, |f| f.cols as usize));
        for row in &body_cells {
            w = w.max(cell_width(&row[c].text));
            w = w.max(row[c].fig.as_ref().map_or(0, |f| f.cols as usize));
        }
        widths[c] = w;
    }

    // Fit within the terminal: each column costs its width plus a ` … │` of 3, and
    // the table opens with one `│`. Cap the widest columns so that the row fits, then
    // cells wrap into their capped width instead of overrunning the right edge.
    let budget = (term.cols as usize)
        .saturating_sub(indent + 1 + 3 * ncol)
        .max(ncol);
    fit_widths(&mut widths, budget);

    // Re-fit each figure to the column it ended up with.
    for c in 0..ncol {
        for cell in
            std::iter::once(&mut head_cells[c]).chain(body_cells.iter_mut().map(|r| &mut r[c]))
        {
            if let Some(f) = cell.fig.as_mut() {
                let (cols, rows) = image_cells(
                    assets.dimensions(&f.png),
                    &term.with_cols(widths[c]),
                    0,
                    f.width,
                );
                f.cols = cols;
                f.rows = rows;
            }
        }
    }

    let pre = " ".repeat(indent);
    let border = |l: char, m: char, r: char| {
        let mut s = String::new();
        s.push(l);
        for (c, w) in widths.iter().enumerate() {
            s.push_str(&"─".repeat(w + 2));
            s.push(if c + 1 == ncol { r } else { m });
        }
        s
    };
    let push_line = |s: String, ops: &mut Vec<RenderOp>| {
        ops.push(RenderOp::Text(format!("{pre}{s}"), Style::default()));
        ops.push(RenderOp::LineBreak);
    };
    let emit_row = |cells: &[Cell], style: Style, ops: &mut Vec<RenderOp>| {
        let wrapped: Vec<Vec<String>> = (0..ncol)
            .map(|c| wrap_text(&cells[c].text, widths[c]))
            .collect();
        let fig_rows = |c: usize| cells[c].fig.as_ref().map_or(0, |f| f.rows as usize);
        let height = (0..ncol)
            .map(|c| fig_rows(c) + wrapped[c].len())
            .max()
            .unwrap_or(1)
            .max(1);
        for r in 0..height {
            ops.push(RenderOp::Text(pre.clone(), Style::default()));
            for c in 0..ncol {
                ops.push(RenderOp::Text("│ ".to_string(), Style::default()));
                if r < fig_rows(c) {
                    // The figure line carries no text, so pad around it by hand to
                    // keep the right border in its column.
                    let f = cells[c].fig.as_ref().unwrap();
                    let slack = widths[c].saturating_sub(f.cols as usize);
                    let left = slack / 2;
                    ops.push(RenderOp::Text(" ".repeat(left), style));
                    ops.push(RenderOp::ImageRow {
                        png_path: f.png.clone(),
                        cols: f.cols,
                        rows: f.rows,
                        row: r as u16,
                    });
                    ops.push(RenderOp::Text(" ".repeat(slack - left + 1), style));
                } else {
                    let seg = wrapped[c]
                        .get(r - fig_rows(c))
                        .map(String::as_str)
                        .unwrap_or("");
                    ops.push(RenderOp::Text(
                        format!("{} ", pad(seg, widths[c], align_of(c))),
                        style,
                    ));
                }
            }
            ops.push(RenderOp::Text("│".to_string(), Style::default()));
            ops.push(RenderOp::LineBreak);
        }
    };

    // A blank line between body rows, so entries read apart without a rule.
    let gap = {
        let mut s = String::new();
        for w in &widths {
            s.push_str("│ ");
            s.push_str(&" ".repeat(w + 1));
        }
        s.push('│');
        s
    };

    push_line(border('┌', '┬', '┐'), ops);
    // Markdown has no headerless table: the delimiter row is required and the first row is
    // the header. A header left empty is how one is asked for, so the row and the rule under
    // it are dropped, and the top rule opens the body.
    if head.iter().any(|cell| !cell.is_empty()) {
        emit_row(&head_cells, heading_style(), ops);
        push_line(border('├', '┼', '┤'), ops);
    }
    for (i, row) in body_cells.iter().enumerate() {
        if i > 0 {
            push_line(gap.clone(), ops);
        }
        emit_row(row, Style::default(), ops);
    }
    push_line(border('└', '┴', '┘'), ops);
}

fn fit_widths(widths: &mut [usize], budget: usize) {
    if widths.iter().sum::<usize>() <= budget {
        return;
    }
    let (mut low, mut high) = (0, widths.iter().copied().max().unwrap_or(0));
    while low < high {
        let cap = low + (high - low).div_ceil(2);
        if widths.iter().map(|width| (*width).min(cap)).sum::<usize>() <= budget {
            low = cap;
        } else {
            high = cap - 1;
        }
    }
    let mut spare = budget - widths.iter().map(|width| (*width).min(low)).sum::<usize>();
    for width in widths {
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
                for c in 0..8 {
                    for budget in 3..24 {
                        let mut expected = vec![a, b, c];
                        while expected.iter().sum::<usize>() > budget {
                            let widest = (0..3).max_by_key(|&i| expected[i]).unwrap();
                            if expected[widest] <= 1 {
                                break;
                            }
                            expected[widest] -= 1;
                        }
                        let mut actual = vec![a, b, c];
                        fit_widths(&mut actual, budget);
                        assert_eq!(actual, expected, "widths={a},{b},{c}; budget={budget}");
                    }
                }
            }
        }
    }
}
