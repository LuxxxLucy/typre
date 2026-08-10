use std::path::Path;

use crate::core::ir::{Inline, RenderOp, Style};
use crate::commands::typst;
use crate::layout::{natural_ppi, TermInfo};
use crate::render::paint::{
    break_units, cell_width, code_style, image_dims, indent_op, Hit, HitAction,
};

// Lay inline content into a column of `term.cols - hang`, wrapping on spaces.
// The first line starts at `lead`; wrapped lines align at the hanging indent
// `hang` (for a list item the marker already fills the gap before the first line,
// so it passes lead 0 and hang past the marker).
pub(crate) fn emit_inlines(
    inls: &[Inline],
    base: Style,
    term: &TermInfo,
    deck_dir: &Path,
    lead: usize,
    hang: usize,
    ops: &mut Vec<RenderOp>,
) {
    let width = (term.cols as usize).saturating_sub(hang).max(1);
    let mut col = 0usize;
    if lead > 0 {
        ops.push(indent_op(lead));
    }
    for tok in inline_tokens(inls, base, term, deck_dir) {
        if let Tok::Break = tok {
            ops.push(RenderOp::LineBreak);
            if hang > 0 {
                ops.push(indent_op(hang));
            }
            col = 0;
            continue;
        }
        let w = tok_width(&tok);
        if col + w > width && col > 0 {
            ops.push(RenderOp::LineBreak);
            if hang > 0 {
                ops.push(indent_op(hang));
            }
            col = 0;
            if let Tok::Space(_) = tok {
                continue;
            }
        }
        match tok {
            Tok::Text(t, s) => ops.push(RenderOp::Text(t, s)),
            Tok::Space(s) => ops.push(RenderOp::Text(" ".to_string(), s)),
            Tok::Img { png, cols } => ops.push(RenderOp::InlineImage {
                png_path: png,
                cols,
                rows: 1,
                row: 0,
            }),
            Tok::Link { label, url, style } => ops.push(RenderOp::Link { label, url, style }),
            Tok::Break => {}
        }
        col += w;
    }
}

enum Tok {
    Text(String, Style),
    Space(Style),
    Img {
        png: std::path::PathBuf,
        cols: u16,
    },
    Link {
        label: String,
        url: String,
        style: Style,
    },
    Break,
}

fn tok_width(t: &Tok) -> usize {
    match t {
        Tok::Text(s, _) => cell_width(s),
        Tok::Space(_) => 1,
        Tok::Img { cols, .. } => *cols as usize,
        Tok::Link { label, .. } => cell_width(label),
        Tok::Break => 0,
    }
}

fn inline_tokens(inls: &[Inline], base: Style, term: &TermInfo, deck_dir: &Path) -> Vec<Tok> {
    let mut toks = Vec::new();
    for inl in inls {
        match inl {
            Inline::Text(t, s) => push_words(&mut toks, t, merge(base, *s)),
            // One contiguous chip with a space of padding each side, so the
            // background reads like the code block's panel, not a bare word.
            Inline::Code(t) => toks.push(Tok::Text(format!(" {t} "), merge(base, code_style()))),
            Inline::Link { label, url } => toks.push(Tok::Link {
                label: label.clone(),
                url: url.clone(),
                style: merge(
                    base,
                    Style {
                        underline: true,
                        ..Style::default()
                    },
                ),
            }),
            Inline::InlineTypst { src, .. } => match typst::render_fragment(src, deck_dir, natural_ppi(term), false) {
                Ok(png) => {
                    let cw = term.cell_w_px.max(1) as f32;
                    let ch = term.cell_h_px.max(1) as f32;
                    let (w, h) = image_dims(&png).unwrap_or((cw as u32, ch as u32));
                    // One cell tall (r=1); the placeholder fit preserves aspect, so round
                    // the width up. A box narrower than the fragment's aspect shrinks it
                    // below one cell, leaving fragments at inconsistent heights.
                    let cols = ((w as f32 / h as f32) * ch / cw).ceil().max(1.0) as u16;
                    toks.push(Tok::Img { png, cols });
                }
                Err(e) => push_words(&mut toks, &format!("[typst error: {e}]"), base),
            },
            Inline::SoftBreak => toks.push(Tok::Space(base)),
            Inline::HardBreak => toks.push(Tok::Break),
            Inline::BlockFragment(_) => {}
        }
    }
    toks
}

// One Tok per break unit, so the inline flow and the plain-text wrapper agree on where a
// line may break.
fn push_words(toks: &mut Vec<Tok>, text: &str, style: Style) {
    for (unit, _) in break_units(text) {
        if unit == " " {
            toks.push(Tok::Space(style));
        } else {
            toks.push(Tok::Text(unit, style));
        }
    }
}

fn merge(a: Style, b: Style) -> Style {
    Style {
        bold: a.bold || b.bold,
        italic: a.italic || b.italic,
        underline: a.underline || b.underline,
        dim: a.dim || b.dim,
        code: a.code || b.code,
        quote: a.quote || b.quote,
    }
}

// Click targets, found by replaying the body's cursor motion so each target's row and column
// span are known without threading state through emit. A details box marks its summary row
// with a ToggleTarget op, so a box at any nesting depth lands on the right row.
pub(crate) fn hits(ops: &[RenderOp]) -> Vec<Hit> {
    let mut hits = Vec::new();
    let (mut row, mut col) = (0u16, 0u16);
    for op in ops {
        match op {
            RenderOp::LineBreak => {
                row += 1;
                col = 0;
            }
            RenderOp::Text(t, _) => col += cell_width(t) as u16,
            RenderOp::InlineImage { cols, .. } => col += cols,
            RenderOp::Link { label, url, .. } => {
                let w = cell_width(label) as u16;
                hits.push(Hit {
                    row,
                    cols: col..col + w,
                    action: HitAction::OpenUrl(url.clone()),
                });
                col += w;
            }
            RenderOp::ToggleTarget(id) => hits.push(Hit {
                row,
                cols: 0..u16::MAX,
                action: HitAction::ToggleDetails(*id),
            }),
            _ => {}
        }
    }
    hits
}

pub(crate) fn uppercase_inlines(inls: &[Inline]) -> Vec<Inline> {
    inls.iter()
        .map(|inl| match inl {
            Inline::Text(t, s) => Inline::Text(t.to_uppercase(), *s),
            other => other.clone(),
        })
        .collect()
}

// Display width of inline content in cells (images and breaks count as their tokens do).
pub(crate) fn disp_width(inls: &[Inline]) -> usize {
    inls.iter()
        .map(|inl| match inl {
            Inline::Text(t, _) => cell_width(t),
            Inline::Code(t) => cell_width(t),
            Inline::Link { label, .. } => cell_width(label),
            _ => 0,
        })
        .sum()
}

pub(crate) fn flat_text(inls: &[Inline]) -> String {
    inls.iter()
        .map(|inl| match inl {
            Inline::Text(t, _) => t.clone(),
            Inline::Code(t) => t.clone(),
            Inline::Link { label, .. } => label.clone(),
            _ => String::new(),
        })
        .collect()
}
