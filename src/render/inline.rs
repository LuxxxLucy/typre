use crate::assets::Assets;

use crate::core::ir::{Inline, Style};
use crate::layout::RenderOp;
use crate::layout::{natural_ppi, TermInfo};
use crate::render::paint::{break_units, cell_width, code_style, indent_op};

// Lay inline content into a column of `term.cols - hang`, wrapping on spaces.
// The first line starts at `lead`; wrapped lines align at the hanging indent
// `hang` (for a list item the marker already fills the gap before the first line,
// so it passes lead 0 and hang past the marker).
pub(crate) fn emit_inlines(
    inls: &[Inline],
    base: Style,
    term: &TermInfo,
    assets: &Assets,
    lead: usize,
    hang: usize,
    ops: &mut Vec<RenderOp>,
) {
    let tokens = inline_tokens(inls, base, term, assets);
    emit_tokens(&tokens, term.cols as usize, lead, hang, ops);
}

pub(crate) fn emit_tokens(
    tokens: &[Tok],
    cols: usize,
    lead: usize,
    hang: usize,
    ops: &mut Vec<RenderOp>,
) {
    let width = cols.saturating_sub(hang).max(1);
    let mut col = 0usize;
    if lead > 0 {
        ops.push(indent_op(lead));
    }
    let newline = |ops: &mut Vec<RenderOp>| {
        ops.push(RenderOp::LineBreak);
        if hang > 0 {
            ops.push(indent_op(hang));
        }
    };
    for token in tokens {
        if let Tok::Break = token {
            newline(ops);
            col = 0;
            continue;
        }
        let token_width = tok_width(token);
        if col > 0 && col + token_width > width {
            newline(ops);
            col = 0;
            if matches!(token, Tok::Space(_))
                || matches!(token, Tok::Link { label, .. } if label == " ")
            {
                continue;
            }
        }
        let text = match token {
            Tok::Text(text, _) => Some(text.as_str()),
            Tok::Link { label, .. } => Some(label.as_str()),
            _ => None,
        };
        if let Some(text) = text {
            for part in crate::layout::text::split_word(text, width) {
                let part_width = cell_width(part);
                if col > 0 && col + part_width > width {
                    newline(ops);
                    col = 0;
                }
                match token {
                    Tok::Text(_, style) => ops.push(RenderOp::Text(part.to_string(), *style)),
                    Tok::Link { url, style, .. } => ops.push(RenderOp::Link {
                        label: part.to_string(),
                        url: url.clone(),
                        style: *style,
                    }),
                    _ => unreachable!(),
                }
                col += part_width;
            }
        } else {
            match token {
                Tok::Space(style) => ops.push(RenderOp::Text(" ".to_string(), *style)),
                Tok::Img { png, cols } => ops.push(RenderOp::ImageRow {
                    png_path: png.clone(),
                    cols: (*cols as usize).min(width) as u16,
                    rows: 1,
                    row: 0,
                }),
                _ => unreachable!(),
            }
            col += token_width.min(width);
        }
    }
}

pub(crate) fn tokens_width(tokens: &[Tok]) -> usize {
    let mut widest = 0;
    let mut width = 0;
    for token in tokens {
        if matches!(token, Tok::Break) {
            widest = widest.max(width);
            width = 0;
        } else {
            width += tok_width(token);
        }
    }
    widest.max(width)
}

pub(crate) enum Tok {
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

pub(crate) fn inline_tokens(
    inls: &[Inline],
    base: Style,
    term: &TermInfo,
    assets: &Assets,
) -> Vec<Tok> {
    let mut toks = Vec::new();
    for inl in inls {
        match inl {
            Inline::Text(t, s) => push_words(&mut toks, t, merge(base, *s)),
            // One contiguous chip with a space of padding each side, so the
            // background reads like the code block's panel, not a bare word.
            Inline::Code(t) => toks.push(Tok::Text(format!(" {t} "), merge(base, code_style()))),
            Inline::Link { label, url } => {
                let style = merge(
                    base,
                    Style {
                        underline: true,
                        ..Style::default()
                    },
                );
                for (unit, _) in break_units(label) {
                    toks.push(Tok::Link {
                        label: unit.to_string(),
                        url: url.clone(),
                        style,
                    });
                }
            }
            Inline::InlineTypst { src, .. } => match assets.fragment(src, natural_ppi(term), false)
            {
                Ok(png) => {
                    let cw = term.cell_w_px.max(1) as f32;
                    let ch = term.cell_h_px.max(1) as f32;
                    let (w, h) = assets.dimensions(&png).unwrap_or((cw as u32, ch as u32));
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
            toks.push(Tok::Text(unit.to_string(), style));
        }
    }
}

fn merge(a: Style, b: Style) -> Style {
    Style {
        glow: a.glow || b.glow,
        bold: a.bold || b.bold,
        italic: a.italic || b.italic,
        underline: a.underline || b.underline,
        dim: a.dim || b.dim,
        code: a.code || b.code,
        quote: a.quote || b.quote,
    }
}

pub(crate) fn uppercase_inlines(inls: &[Inline]) -> Vec<Inline> {
    inls.iter()
        .map(|inl| match inl {
            Inline::Text(t, s) => Inline::Text(t.to_uppercase(), *s),
            other => other.clone(),
        })
        .collect()
}

pub(crate) use crate::core::ir::plain_text as flat_text;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::ops::{hits, positions};
    use crate::render::blocks::split_lines;
    use crate::render::paint::wrap_text;

    #[test]
    fn plain_and_styled_text_wrap_identically() {
        for text in [
            "abcdefgh",
            "one two three",
            "e\u{301}e\u{301}e\u{301}",
            "中文内容",
            "👩‍💻👩‍💻",
        ] {
            for width in 1..8 {
                let mut tokens = Vec::new();
                push_words(&mut tokens, text, Style::default());
                let mut ops = Vec::new();
                emit_tokens(&tokens, width, 0, 0, &mut ops);
                let lines: Vec<String> = split_lines(ops)
                    .iter()
                    .map(|row| {
                        row.iter()
                            .filter_map(|op| match op {
                                RenderOp::Text(text, _) => Some(text.as_str()),
                                _ => None,
                            })
                            .collect()
                    })
                    .collect();
                assert_eq!(lines, wrap_text(text, width), "text={text}; width={width}");
            }
        }
    }

    #[test]
    fn cursor_positions_follow_absolute_moves() {
        let ops = vec![
            RenderOp::MoveTo(4, 7),
            RenderOp::Text("x".to_string(), Style::default()),
        ];
        let positions: Vec<_> = positions(&ops).collect();
        assert_eq!((positions[1].0, positions[1].1), (7, 4));
    }

    #[test]
    fn links_wrap_at_spaces_and_keep_click_targets() {
        let assets = Assets::new(std::path::Path::new("."));
        let term = TermInfo {
            cols: 5,
            rows: 10,
            cell_w_px: 8,
            cell_h_px: 16,
        };
        let inlines = vec![Inline::Link {
            label: "one two".into(),
            url: "https://example.com".into(),
        }];
        let mut ops = Vec::new();
        emit_inlines(&inlines, Style::default(), &term, &assets, 0, 0, &mut ops);
        let targets = hits(&ops);
        assert!(targets.iter().any(|hit| hit.row == 1 && hit.cols == (0..3)));
        assert!(split_lines(ops)
            .iter()
            .all(|line| crate::render::blocks::line_width(line) <= 5));
    }
}
