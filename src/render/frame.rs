use crate::core::ir::{Align, Meta, Style};
use crate::layout::Body;
use crate::layout::RenderOp;
use crate::layout::{layout, viewport, TermInfo};
use crate::render::paint::{
    cell_width, dim_style, heading_style, hrule, pad, truncate, Hit, HitAction,
};

#[derive(Debug)]
pub struct Frame {
    pub ops: Vec<RenderOp>,
    pub hits: Vec<Hit>,
    pub height: usize,
    pub scroll: usize,
}

#[derive(Default)]
pub struct View {
    pub index: usize,
    pub total: usize,
    pub scroll: usize,
    pub help: bool,
}

pub fn compose(body: &Body, term: &TermInfo, meta: &Meta, view: &View) -> Frame {
    let height = body.height();
    let vp = viewport(term);
    let scroll = view.scroll.min(height.saturating_sub(vp));
    let (visible, mut hits) = body.window(scroll, vp);
    let mut ops = vec![RenderOp::ClearImages, RenderOp::MoveTo(0, 0)];
    ops.extend(visible);
    let (bar, home) = status_bar(term, view.index, view.total.max(1), meta);
    ops.extend(bar);
    hits.push(home);
    ops.extend(scrollbar(term, scroll, vp, height));
    if view.help {
        ops.extend(help_overlay(term));
        hits.clear();
    }
    Frame {
        ops,
        hits,
        height,
        scroll,
    }
}

fn status_bar(term: &TermInfo, idx: usize, total: usize, meta: &Meta) -> (Vec<RenderOp>, Hit) {
    let (margin, content_w) = layout(term);
    let content_w = content_w.min(term.cols as usize);
    let x = margin as u16;
    let row = term.rows.saturating_sub(1);
    let right = truncate(&format!("{} / {}", idx + 1, total), content_w);
    let left_w = content_w.saturating_sub(cell_width(&right) + 1);
    let button = truncate("⌂ Contents", left_w);
    let meta_text = match (&meta.title, &meta.author) {
        (Some(title), Some(author)) => format!("   {title} / {author}"),
        (Some(title), None) => format!("   {title}"),
        (None, Some(author)) => format!("   {author}"),
        _ => String::new(),
    };
    let rest = left_w.saturating_sub(cell_width(&button));
    let hint = if rest >= 9 { "   ? help" } else { "" };
    let meta_text = truncate(&meta_text, rest.saturating_sub(cell_width(hint)));
    let used = cell_width(&button) + cell_width(&meta_text) + cell_width(hint) + cell_width(&right);
    let ops = vec![
        RenderOp::MoveTo(x, term.rows.saturating_sub(2)),
        RenderOp::Text("─".repeat(content_w), dim_style()),
        RenderOp::MoveTo(x, row),
        RenderOp::Text(button.clone(), heading_style()),
        RenderOp::Text(meta_text, dim_style()),
        RenderOp::Text(hint.to_string(), dim_style()),
        RenderOp::Text(" ".repeat(content_w.saturating_sub(used)), Style::default()),
        RenderOp::Text(right, heading_style()),
    ];
    let hit = Hit {
        row,
        cols: x..x + cell_width(&button) as u16,
        action: HitAction::Goto(0),
    };
    (ops, hit)
}

fn centered_box(term: &TermInfo, lines: &[(String, Style)]) -> Vec<RenderOp> {
    let cols = term.cols as usize;
    let rows = term.rows as usize;
    if cols == 0 || rows == 0 {
        return Vec::new();
    }
    if cols < 5 || rows < 3 {
        return lines
            .iter()
            .take(rows)
            .enumerate()
            .flat_map(|(row, (line, style))| {
                [
                    RenderOp::MoveTo(0, row as u16),
                    RenderOp::Text(truncate(line, cols), *style),
                ]
            })
            .collect();
    }
    let lines = &lines[..lines.len().min(rows - 2)];
    let width = lines
        .iter()
        .map(|(line, _)| cell_width(line))
        .max()
        .unwrap_or(0)
        .min(cols - 4);
    let x = ((cols - width - 4) / 2) as u16;
    let y = (rows - lines.len() - 2) / 2;
    let mut ops = vec![
        RenderOp::MoveTo(x, y as u16),
        RenderOp::Text(hrule('┌', width + 2, '┐'), Style::default()),
    ];
    for (i, (line, style)) in lines.iter().enumerate() {
        ops.push(RenderOp::MoveTo(x, (y + 1 + i) as u16));
        ops.push(RenderOp::Text("│ ".to_string(), Style::default()));
        ops.push(RenderOp::Text(
            pad(&truncate(line, width), width, Align::Left),
            *style,
        ));
        ops.push(RenderOp::Text(" │".to_string(), Style::default()));
    }
    ops.push(RenderOp::MoveTo(x, (y + 1 + lines.len()) as u16));
    ops.push(RenderOp::Text(hrule('└', width + 2, '┘'), Style::default()));
    ops
}

fn help_overlay(term: &TermInfo) -> Vec<RenderOp> {
    let lines = [
        "Keys",
        "",
        "→  l  j  space   next slide",
        "←  h  k          previous slide",
        "g  /  G       first / last slide",
        "<n> G         go to slide n",
        "wheel         scroll a long slide",
        "click +       expand a details box",
        "click link    open in browser",
        "Shift+drag    select text",
        "?             toggle this help",
        "Esc           close help",
        "q  Ctrl-C     quit",
    ];
    let styled: Vec<(String, Style)> = lines
        .iter()
        .enumerate()
        .map(|(i, l)| {
            (
                l.to_string(),
                if i == 0 {
                    heading_style()
                } else {
                    Style::default()
                },
            )
        })
        .collect();
    centered_box(term, &styled)
}

pub(crate) fn loading_frame(term: &TermInfo, label: &str) -> Vec<RenderOp> {
    let mut ops = vec![RenderOp::ClearImages];
    ops.extend(centered_box(
        term,
        &[
            ("typre".to_string(), heading_style()),
            (String::new(), Style::default()),
            (label.to_string(), Style::default()),
        ],
    ));
    ops
}

pub(crate) fn error_prompt(term: &TermInfo, errors: &[(usize, String)]) -> Vec<RenderOp> {
    let max_w = (term.cols as usize).saturating_sub(8).max(20);
    let rows = term.rows as usize;
    let mut lines: Vec<(String, Style)> = vec![
        ("Typst compile errors".to_string(), heading_style()),
        (String::new(), Style::default()),
    ];
    let budget = rows.saturating_sub(8).max(2);
    for (shown, (n, msg)) in errors.iter().enumerate() {
        if shown >= budget {
            lines.push((format!("… and {} more", errors.len() - shown), dim_style()));
            break;
        }
        let first = msg.lines().next().unwrap_or("");
        lines.push((
            truncate(&format!("slide {n}: {first}"), max_w),
            Style::default(),
        ));
    }
    lines.push((String::new(), Style::default()));
    lines.push((
        "Continue opening?   [y]es    [n]o / quit".to_string(),
        heading_style(),
    ));
    centered_box(term, &lines)
}

fn scrollbar(term: &TermInfo, scroll: usize, vp: usize, height: usize) -> Vec<RenderOp> {
    if height <= vp || vp == 0 {
        return Vec::new();
    }
    let col = term.cols.saturating_sub(1);
    let thumb = (vp * vp / height).clamp(1, vp);
    let pos = scroll * (vp - thumb) / (height - vp);
    let mut ops = Vec::new();
    for r in 0..vp {
        let (ch, style) = if r >= pos && r < pos + thumb {
            ("█", Style::default())
        } else {
            ("░", dim_style())
        };
        ops.push(RenderOp::MoveTo(col, r as u16));
        ops.push(RenderOp::Text(ch.to_string(), style));
    }
    ops
}
