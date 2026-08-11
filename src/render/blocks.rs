use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::core::ir::{draws, ArtLine, ArtPart, Align, Block, Inline, RenderOp, Style, Width};
use crate::layout::{natural_ppi, TermInfo};
use crate::commands;
use crate::render::inline::{disp_width, emit_inlines, flat_text, uppercase_inlines};
use crate::render::paint::{
    caption_style, cell_width, code_style, heading_style, hrule, image_cells, indent_op, pad,
    place_image, quote_style, wrap_text,
};

// Details boxes are numbered as they are emitted, at any nesting depth, so a box inside art
// toggles like one at the top of a slide. `open` holds the ids the user has expanded.
pub(crate) struct Toggles<'a> {
    pub open: &'a HashSet<usize>,
    pub next_id: usize,
}

impl Toggles<'_> {
    fn take_id(&mut self) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}

pub(crate) fn emit_block(
    block: &Block,
    term: &TermInfo,
    deck_dir: &Path,
    indent: usize,
    tg: &mut Toggles,
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
            emit_list(*ordered, items, "", term, deck_dir, indent, tg, ops);
        }
        Block::Code { src, lang } => emit_code(src, lang.as_deref(), term, indent, ops),
        Block::Art { parts, caption } => {
            emit_art(parts, caption, term, deck_dir, indent, tg, ops)
        }
        Block::BlockTypst { src, width } => {
            commands::typst::render_block(src, *width, term, deck_dir, indent, ops)
        }
        Block::Image { src, alt } => {
            let png_path = deck_dir.join(src);
            if png_path.exists() {
                place_image(ops, png_path, term, indent, Width::Natural);
                if !alt.is_empty() {
                    let content_w = (term.cols as usize).saturating_sub(indent);
                    ops.push(indent_op(indent));
                    ops.push(RenderOp::Text(
                        pad(alt, content_w, Align::Center),
                        caption_style(),
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
            emit_table(aligns, head, rows, term, deck_dir, indent, ops);
        }
        Block::Quote(inner) => emit_quote(inner, term, deck_dir, indent, tg, ops),
        Block::Tree(nodes) => commands::tree::render(nodes, indent, ops),
        Block::Grid(cells) => commands::grid::render(cells, term, indent, ops),
        Block::Details { summary, body } => {
            let id = tg.take_id();
            let open = tg.open.contains(&id);
            commands::details::render(id, open, summary, body, term, deck_dir, indent, ops);
        }
    }
}

fn code_label_style() -> Style {
    Style {
        bold: true,
        ..code_style()
    }
}

// A fenced code block: the language as a label when the fence names one, then every line on
// the panel background, padded to the column.
fn emit_code(
    src: &str,
    lang: Option<&str>,
    term: &TermInfo,
    indent: usize,
    ops: &mut Vec<RenderOp>,
) {
    let w = (term.cols as usize).saturating_sub(indent).max(1);
    if let Some(lang) = lang {
        ops.push(indent_op(indent));
        ops.push(RenderOp::Text(format!(" {lang} "), code_label_style()));
        ops.push(RenderOp::LineBreak);
    }
    for line in src.lines() {
        ops.push(indent_op(indent));
        ops.push(RenderOp::Text(
            pad(&format!("  {line}"), w, Align::Left),
            code_style(),
        ));
        ops.push(RenderOp::LineBreak);
    }
}

// A figure two spaces in from the margin: its lines, the blocks its ◊ commands parsed to, and
// a caption under them if there is one. A nested block is rendered on its own, then every line
// of it is prefixed with the guides of the line the command sat on, so a details box under a
// `├─` branch stays under that branch.
fn emit_art(
    parts: &[ArtPart],
    caption: &[Inline],
    term: &TermInfo,
    deck_dir: &Path,
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
                    let rows = art_rows(l, width, term, deck_dir);
                    let cont = continue_guides(&l.guide);
                    prefix_rows(&pre, &l.guide, &cont, rows, ops);
                }
            }
            ArtPart::Nested { guide, block } => {
                let inner = term.with_cols(width.saturating_sub(cell_width(guide)));
                let mut sub = Vec::new();
                emit_block(block, &inner, deck_dir, 0, tg, &mut sub);
                prefix_rows(&pre, guide, guide, split_lines(sub), ops);
            }
        }
    }
    if !caption.is_empty() {
        let line = ArtLine {
            guide: String::new(),
            inls: caption.to_vec(),
        };
        let rows = art_rows(&line, width, term, deck_dir);
        prefix_rows(&pre, "", "", rows, ops);
    }
}

// The visual rows of one art line. Laid out unwrapped first: a line that fits is drawn as
// written, and so is one that draws (a box side, an arrow), since reflowing it breaks the art.
// Prose past the column is laid out again against what the guides leave.
fn art_rows(
    line: &ArtLine,
    width: usize,
    term: &TermInfo,
    deck_dir: &Path,
) -> Vec<Vec<RenderOp>> {
    let lay = |cols: usize| {
        let mut sub = Vec::new();
        let inner = term.with_cols(cols);
        emit_inlines(&line.inls, Style::default(), &inner, deck_dir, 0, 0, &mut sub);
        split_lines(sub)
    };
    let gw = cell_width(&line.guide);
    let rows = lay(usize::from(u16::MAX));
    let w = gw + rows.iter().map(|r| line_width(r)).max().unwrap_or(0);
    if w <= width || flat_text(&line.inls).chars().any(draws) {
        return rows;
    }
    lay(width.saturating_sub(gw).max(1))
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
        .map(|c| if matches!(c, '│' | '├' | '┌' | '┬' | '┼') { '│' } else { ' ' })
        .collect()
}

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

// A blockquote: a grey background box spanning the content width, one space of
// inset each side, with a blank row above and below.
fn emit_quote(
    inner: &[Block],
    term: &TermInfo,
    deck_dir: &Path,
    indent: usize,
    tg: &mut Toggles,
    ops: &mut Vec<RenderOp>,
) {
    let text_w = (term.cols as usize).saturating_sub(indent + 2).max(1);
    let inner_term = term.with_cols(text_w);
    let mut sub = Vec::new();
    for b in inner {
        emit_block(b, &inner_term, deck_dir, 0, tg, &mut sub);
    }
    let deco = BoxDeco::shaded(text_w, quote_style());
    emit_box(split_lines(sub), text_w, indent, &deco, ops);
}

// Split an op stream into visual lines at LineBreaks, dropping the trailing empty
// line every block leaves behind.
pub(crate) fn split_lines(ops: Vec<RenderOp>) -> Vec<Vec<RenderOp>> {
    let mut lines = vec![Vec::new()];
    for op in ops {
        if let RenderOp::LineBreak = op {
            lines.push(Vec::new());
        } else {
            lines.last_mut().unwrap().push(op);
        }
    }
    if lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    lines
}

pub(crate) fn line_width(line: &[RenderOp]) -> usize {
    line.iter()
        .map(|op| match op {
            RenderOp::Text(t, _) => cell_width(t),
            RenderOp::InlineImage { cols, .. } => *cols as usize,
            RenderOp::Link { label, .. } => cell_width(label),
            _ => 0,
        })
        .sum()
}

// Add the box background to a text or link op; other ops pass through unstyled.
fn shade(op: RenderOp, bg: Style) -> RenderOp {
    match op {
        RenderOp::Text(t, s) => RenderOp::Text(t, Style { quote: bg.quote, ..s }),
        RenderOp::Link { label, url, style } => RenderOp::Link {
            label,
            url,
            style: Style { quote: bg.quote, ..style },
        },
        other => other,
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_list(
    ordered: bool,
    items: &[Vec<Block>],
    prefix: &str,
    term: &TermInfo,
    deck_dir: &Path,
    indent: usize,
    tg: &mut Toggles,
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
        let cont = indent + cell_width(&marker);
        for (j, b) in item.iter().enumerate() {
            match b {
                Block::Paragraph(inls) => {
                    let lead = if j == 0 { 0 } else { cont };
                    emit_inlines(inls, Style::default(), term, deck_dir, lead, cont, ops);
                    ops.push(RenderOp::LineBreak);
                }
                Block::List { ordered: o, items } => {
                    emit_list(*o, items, &item_prefix, term, deck_dir, cont, tg, ops);
                }
                nested => emit_block(nested, term, deck_dir, cont, tg, ops),
            }
        }
    }
}

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
fn cell_fig(inls: &[Inline], term: &TermInfo, deck_dir: &Path, avail: usize) -> Option<Fig> {
    let (src, width) = inls.iter().find_map(|i| match i {
        Inline::InlineTypst { src, width, .. } => Some((src, *width)),
        _ => None,
    })?;
    let png = commands::typst::render_fragment(src, deck_dir, natural_ppi(term), true).ok()?;
    let (cols, rows) = image_cells(&png, &term.with_cols(avail), 0, width);
    Some(Fig {
        png,
        width,
        cols,
        rows,
    })
}

fn emit_table(
    aligns: &[Align],
    head: &[Vec<Inline>],
    rows: &[Vec<Vec<Inline>>],
    term: &TermInfo,
    deck_dir: &Path,
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
            fig: cell_fig(inls, term, deck_dir, content_w),
        }
    };
    let mut head_cells: Vec<Cell> = (0..ncol).map(|c| build(head.get(c))).collect();
    let mut body_cells: Vec<Vec<Cell>> = rows
        .iter()
        .map(|row| (0..ncol).map(|c| build(row.get(c))).collect())
        .collect();

    // A column is as wide as its widest text, or its widest figure.
    let mut widths = vec![0usize; ncol];
    for c in 0..ncol {
        let mut w = disp_width(head.get(c).map(Vec::as_slice).unwrap_or(&[]));
        w = w.max(head_cells[c].fig.as_ref().map_or(0, |f| f.cols as usize));
        for (r, row) in rows.iter().enumerate() {
            w = w.max(disp_width(row.get(c).map(Vec::as_slice).unwrap_or(&[])));
            w = w.max(body_cells[r][c].fig.as_ref().map_or(0, |f| f.cols as usize));
        }
        widths[c] = w;
    }

    // Fit within the terminal: each column costs its width plus a ` … │` of 3, and
    // the table opens with one `│`. Shave the widest column until the row fits, then
    // cells wrap into their capped width instead of overrunning the right edge.
    let budget = (term.cols as usize)
        .saturating_sub(indent + 1 + 3 * ncol)
        .max(ncol);
    while widths.iter().sum::<usize>() > budget {
        let widest = (0..ncol).max_by_key(|&c| widths[c]).unwrap();
        if widths[widest] <= 1 {
            break;
        }
        widths[widest] -= 1;
    }

    // Re-fit each figure to the column it ended up with.
    for c in 0..ncol {
        for cell in std::iter::once(&mut head_cells[c]).chain(body_cells.iter_mut().map(|r| &mut r[c]))
        {
            if let Some(f) = cell.fig.as_mut() {
                let (cols, rows) = image_cells(&f.png, &term.with_cols(widths[c]), 0, f.width);
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
                    ops.push(RenderOp::InlineImage {
                        png_path: f.png.clone(),
                        cols: f.cols,
                        rows: f.rows,
                        row: r as u16,
                    });
                    ops.push(RenderOp::Text(" ".repeat(slack - left + 1), style));
                } else {
                    let seg = wrapped[c].get(r - fig_rows(c)).map(String::as_str).unwrap_or("");
                    ops.push(RenderOp::Text(format!("{} ", pad(seg, widths[c], align_of(c))), style));
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

