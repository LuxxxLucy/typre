use std::path::Path;

use crate::core::ir::{Align, Block, Inline, RenderOp, Style, Width};
use crate::layout::TermInfo;
use crate::commands;
use crate::render::inline::{disp_width, emit_inlines, flat_text, uppercase_inlines};
use crate::render::paint::{code_style, heading_style, indent_op, pad, place_image};

pub(crate) fn emit_block(
    block: &Block,
    term: &TermInfo,
    deck_dir: &Path,
    indent: usize,
    ops: &mut Vec<RenderOp>,
) {
    match block {
        Block::Heading(level, inls) => {
            let owned: Vec<Inline> = if *level <= 2 {
                uppercase_inlines(inls)
            } else {
                inls.clone()
            };
            if *level == 1 {
                let avail = (term.cols as usize).saturating_sub(indent);
                let eff = indent + avail.saturating_sub(disp_width(&owned)) / 2;
                emit_inlines(&owned, heading_style(), term, deck_dir, eff, eff, ops);
            } else {
                // accent bar by level: ┃ for section titles, │ for subsections
                let accent = if *level == 2 {
                    "┃ "
                } else if *level == 3 {
                    "│ "
                } else {
                    ""
                };
                let mut hinls = Vec::new();
                if !accent.is_empty() {
                    hinls.push(Inline::Text(accent.to_string(), heading_style()));
                }
                hinls.extend(owned);
                emit_inlines(&hinls, heading_style(), term, deck_dir, indent, indent, ops);
            }
            ops.push(RenderOp::LineBreak);
        }
        Block::Paragraph(inls) => {
            emit_inlines(inls, Style::default(), term, deck_dir, indent, indent, ops);
            ops.push(RenderOp::LineBreak);
        }
        Block::List { ordered, items } => {
            emit_list(*ordered, items, "", term, deck_dir, indent, ops);
        }
        Block::Code { src, lang } => {
            if let Some(lang) = lang {
                ops.push(indent_op(indent));
                ops.push(RenderOp::Text(format!(" {lang} "), code_label_style()));
                ops.push(RenderOp::LineBreak);
            }
            let w = (term.cols as usize).saturating_sub(indent);
            for line in src.lines() {
                ops.push(indent_op(indent));
                ops.push(RenderOp::Text(
                    pad(&format!("  {line}"), w, Align::Left),
                    code_style(),
                ));
                ops.push(RenderOp::LineBreak);
            }
        }
        Block::BlockTypst { src, width } => {
            commands::typst::render_block(src, *width, term, deck_dir, indent, ops)
        }
        Block::Image { src, alt } => {
            let png_path = deck_dir.join(src);
            if png_path.exists() {
                let (cols, _) = place_image(ops, png_path, term, indent, Width::Natural);
                if !alt.is_empty() {
                    ops.push(indent_op(indent));
                    ops.push(RenderOp::Text(
                        pad(alt, cols as usize, Align::Center),
                        Style {
                            italic: true,
                            ..Style::default()
                        },
                    ));
                    ops.push(RenderOp::LineBreak);
                }
            } else {
                ops.push(indent_op(indent));
                ops.push(RenderOp::Text(format!("[image: {alt}]"), Style::default()));
                ops.push(RenderOp::LineBreak);
            }
        }
        Block::Rule => {
            let w = (term.cols as usize).saturating_sub(indent);
            ops.push(RenderOp::Text(
                format!("{}{}", " ".repeat(indent), "═".repeat(w)),
                Style::default(),
            ));
            ops.push(RenderOp::LineBreak);
        }
        Block::Table { aligns, head, rows } => {
            emit_table(aligns, head, rows, indent, ops);
        }
        Block::Quote(inner) => emit_quote(inner, term, deck_dir, indent, ops),
        Block::Tree(nodes) => commands::tree::render(nodes, indent, ops),
        Block::Grid(cells) => commands::grid::render(cells, term, indent, ops),
        Block::Figure { body, caption } => commands::figure::render(body, caption, indent, ops),
        // Details nested in a quote/list render always-open (only top-level toggles).
        Block::Details { summary, body } => {
            commands::details::render(None, true, summary, body, term, indent, ops);
        }
    }
}

fn code_label_style() -> Style {
    Style {
        bold: true,
        ..code_style()
    }
}

// A blockquote: render the inner blocks, then prefix every line with a `│ ` bar.
fn emit_quote(inner: &[Block], term: &TermInfo, deck_dir: &Path, indent: usize, ops: &mut Vec<RenderOp>) {
    let mut sub = Vec::new();
    for b in inner {
        emit_block(b, term, deck_dir, 0, &mut sub);
    }
    let bar = || RenderOp::Text(format!("{}│ ", " ".repeat(indent)), Style::default());
    let n = sub.len();
    ops.push(bar());
    for (k, op) in sub.into_iter().enumerate() {
        if let RenderOp::LineBreak = op {
            ops.push(RenderOp::LineBreak);
            if k + 1 != n {
                ops.push(bar());
            }
        } else {
            ops.push(op);
        }
    }
}

fn emit_list(
    ordered: bool,
    items: &[Vec<Block>],
    prefix: &str,
    term: &TermInfo,
    deck_dir: &Path,
    indent: usize,
    ops: &mut Vec<RenderOp>,
) {
    for (i, item) in items.iter().enumerate() {
        let marker = if ordered {
            format!("{prefix}{}. ", i + 1)
        } else {
            "▪ ".to_string()
        };
        let item_prefix = if ordered {
            format!("{prefix}{}.", i + 1)
        } else {
            String::new()
        };
        ops.push(RenderOp::Text(
            format!("{}{marker}", " ".repeat(indent)),
            Style::default(),
        ));
        let cont = indent + marker.chars().count();
        for (j, b) in item.iter().enumerate() {
            match b {
                Block::Paragraph(inls) => {
                    let lead = if j == 0 { 0 } else { cont };
                    emit_inlines(inls, Style::default(), term, deck_dir, lead, cont, ops);
                    ops.push(RenderOp::LineBreak);
                }
                Block::List { ordered: o, items } => {
                    emit_list(*o, items, &item_prefix, term, deck_dir, cont, ops);
                }
                nested => emit_block(nested, term, deck_dir, cont, ops),
            }
        }
    }
}

fn emit_table(
    aligns: &[Align],
    head: &[Vec<Inline>],
    rows: &[Vec<Vec<Inline>>],
    indent: usize,
    ops: &mut Vec<RenderOp>,
) {
    let ncol = head.len().max(rows.iter().map(Vec::len).max().unwrap_or(0));
    if ncol == 0 {
        return;
    }
    let align_of = |c: usize| aligns.get(c).copied().unwrap_or(Align::Left);
    let cell_text = |inls: Option<&Vec<Inline>>| inls.map(|i| flat_text(i)).unwrap_or_default();

    let mut widths = vec![0usize; ncol];
    for c in 0..ncol {
        let mut w = disp_width(head.get(c).map(Vec::as_slice).unwrap_or(&[]));
        for row in rows {
            w = w.max(disp_width(row.get(c).map(Vec::as_slice).unwrap_or(&[])));
        }
        widths[c] = w;
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

    push_line(border('┌', '┬', '┐'), ops);
    ops.push(RenderOp::Text(pre.clone(), Style::default()));
    for c in 0..ncol {
        ops.push(RenderOp::Text("│ ".to_string(), Style::default()));
        let txt = pad(&cell_text(head.get(c)), widths[c], align_of(c));
        ops.push(RenderOp::Text(format!("{txt} "), heading_style()));
    }
    ops.push(RenderOp::Text("│".to_string(), Style::default()));
    ops.push(RenderOp::LineBreak);
    push_line(border('├', '┼', '┤'), ops);
    for row in rows {
        let mut line = String::from("│");
        for c in 0..ncol {
            let txt = pad(&cell_text(row.get(c)), widths[c], align_of(c));
            line.push_str(&format!(" {txt} │"));
        }
        push_line(line, ops);
    }
    push_line(border('└', '┴', '┘'), ops);
}
