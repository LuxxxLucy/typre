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
    let rows = term.rows as usize;
    let mx = margin as u16;
    let row = rows.saturating_sub(1) as u16;
    let dim = dim_style();
    let bold = heading_style();
    let mut ops = vec![
        RenderOp::MoveTo(mx, rows.saturating_sub(2) as u16),
        RenderOp::Text("─".repeat(content_w), dim),
        RenderOp::MoveTo(mx, row),
    ];
    let button = "⌂ Contents";
    let meta_text = match (&meta.title, &meta.author) {
        (Some(t), Some(a)) => format!("   {t} — {a}"),
        (Some(t), None) => format!("   {t}"),
        (None, Some(a)) => format!("   {a}"),
        _ => String::new(),
    };
    let hint = "? help";
    let right = format!("{} / {}", idx + 1, total);
    let used =
        cell_width(button) + cell_width(&meta_text) + cell_width(hint) + cell_width(&right) + 4;
    let gap = content_w.saturating_sub(used).max(1);
    ops.push(RenderOp::Text(button.to_string(), bold));
    ops.push(RenderOp::Text(meta_text, dim));
    ops.push(RenderOp::Text(" ".repeat(gap), Style::default()));
    ops.push(RenderOp::Text(format!("{hint}    "), dim));
    ops.push(RenderOp::Text(right, bold));
    let hit = Hit {
        row,
        cols: mx..mx + cell_width(button) as u16,
        action: HitAction::Goto(0),
    };
    (ops, hit)
}

// A centered, bordered box; each line carries its own style.
fn centered_box(term: &TermInfo, lines: &[(String, Style)]) -> Vec<RenderOp> {
    let w = lines.iter().map(|(l, _)| cell_width(l)).max().unwrap_or(0);
    let cols = term.cols as usize;
    let rows = term.rows as usize;
    let x = (cols.saturating_sub(w + 4) / 2) as u16;
    let y0 = rows.saturating_sub(lines.len() + 2) / 2;
    let mut ops = vec![
        RenderOp::MoveTo(x, y0 as u16),
        RenderOp::Text(hrule('┌', w + 2, '┐'), Style::default()),
    ];
    for (i, (l, style)) in lines.iter().enumerate() {
        ops.push(RenderOp::MoveTo(x, (y0 + 1 + i) as u16));
        ops.push(RenderOp::Text("│ ".to_string(), Style::default()));
        ops.push(RenderOp::Text(pad(l, w, Align::Left), *style));
        ops.push(RenderOp::Text(" │".to_string(), Style::default()));
    }
    ops.push(RenderOp::MoveTo(x, (y0 + 1 + lines.len()) as u16));
    ops.push(RenderOp::Text(hrule('└', w + 2, '┘'), Style::default()));
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

// A centered status box shown while the deck's typst fragments compile.
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

// A centered gate listing typst compile errors with a yes/no prompt.
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
