use std::path::Path;

use crate::core::ir::{Inline, RenderOp, Style};
use crate::commands::typst;
use crate::layout::{natural_ppi, TermInfo};
use crate::render::paint::{indent_op, Hit, HitAction};

// Lay inline content into a column of `term.cols - indent`, wrapping on spaces.
pub(crate) fn emit_inlines(
    inls: &[Inline],
    base: Style,
    term: &TermInfo,
    deck_dir: &Path,
    indent: usize,
    ops: &mut Vec<RenderOp>,
) {
    let width = (term.cols as usize).saturating_sub(indent).max(1);
    let mut col = 0usize;
    if indent > 0 {
        ops.push(indent_op(indent));
    }
    for tok in inline_tokens(inls, base, term, deck_dir) {
        if let Tok::Break = tok {
            ops.push(RenderOp::LineBreak);
            if indent > 0 {
                ops.push(indent_op(indent));
            }
            col = 0;
            continue;
        }
        let w = tok_width(&tok);
        if col + w > width && col > 0 {
            ops.push(RenderOp::LineBreak);
            if indent > 0 {
                ops.push(indent_op(indent));
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
        Tok::Text(s, _) => s.chars().count(),
        Tok::Space(_) => 1,
        Tok::Img { cols, .. } => *cols as usize,
        Tok::Link { label, .. } => label.chars().count(),
        Tok::Break => 0,
    }
}

fn inline_tokens(inls: &[Inline], base: Style, term: &TermInfo, deck_dir: &Path) -> Vec<Tok> {
    let mut toks = Vec::new();
    for inl in inls {
        match inl {
            Inline::Text(t, s) => push_words(&mut toks, t, merge(base, *s)),
            Inline::Code(t) => push_words(
                &mut toks,
                t,
                merge(
                    base,
                    Style {
                        code: true,
                        ..Style::default()
                    },
                ),
            ),
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
                    let (w, h) = image::image_dimensions(&png).unwrap_or((cw as u32, ch as u32));
                    // Displayed one cell tall (r=1); width preserves aspect, so it is
                    // independent of the raster resolution.
                    let cols = ((w as f32 / h as f32) * ch / cw).round().max(1.0) as u16;
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

fn push_words(toks: &mut Vec<Tok>, text: &str, style: Style) {
    for w in split_keep_spaces(text) {
        if w == " " {
            toks.push(Tok::Space(style));
        } else {
            toks.push(Tok::Text(w, style));
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
    }
}

fn split_keep_spaces(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in s.chars() {
        if c == ' ' {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            out.push(" ".to_string());
        } else {
            cur.push(c);
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

// Click targets for hyperlinks, found by replaying the body's cursor motion so
// each link's row and column span are known without threading state through emit.
pub(crate) fn link_hits(ops: &[RenderOp]) -> Vec<Hit> {
    let mut hits = Vec::new();
    let (mut row, mut col) = (0u16, 0u16);
    for op in ops {
        match op {
            RenderOp::LineBreak => {
                row += 1;
                col = 0;
            }
            RenderOp::Text(t, _) => col += t.chars().count() as u16,
            RenderOp::InlineImage { cols, .. } => col += cols,
            RenderOp::Link { label, url, .. } => {
                let w = label.chars().count() as u16;
                hits.push(Hit {
                    row,
                    cols: col..col + w,
                    action: HitAction::OpenUrl(url.clone()),
                });
                col += w;
            }
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
            Inline::Text(t, _) => t.chars().count(),
            Inline::Code(t) => t.chars().count(),
            Inline::Link { label, .. } => label.chars().count(),
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
