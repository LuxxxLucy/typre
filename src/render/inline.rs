use crate::assets::Assets;

use crate::core::ir::{Inline, Style};
use crate::layout::RenderOp;
use crate::layout::{natural_ppi, TermInfo};
use crate::render::paint::{break_units, cell_width, code_style, indent_op};

pub(crate) fn emit_inlines(
    inls: &[Inline],
    base: Style,
    term: &TermInfo,
    assets: &Assets,
    first_indent: usize,
    continuation_indent: usize,
    ops: &mut Vec<RenderOp>,
) {
    let tokens = inline_tokens(inls, base, term, assets);
    emit_tokens(
        &tokens,
        term.cols as usize,
        first_indent,
        continuation_indent,
        ops,
    );
}

pub(crate) fn emit_tokens(
    tokens: &[InlineToken],
    cols: usize,
    first_indent: usize,
    continuation_indent: usize,
    ops: &mut Vec<RenderOp>,
) {
    let width = cols.saturating_sub(continuation_indent).max(1);
    let mut col = 0usize;
    if first_indent > 0 {
        ops.push(indent_op(first_indent));
    }
    let newline = |ops: &mut Vec<RenderOp>| {
        ops.push(RenderOp::LineBreak);
        if continuation_indent > 0 {
            ops.push(indent_op(continuation_indent));
        }
    };
    for token in tokens {
        if let InlineToken::Break = token {
            newline(ops);
            col = 0;
            continue;
        }
        let token_width = token_width(token);
        if col > 0 && col + token_width > width {
            newline(ops);
            col = 0;
            if matches!(token, InlineToken::Space(_))
                || matches!(token, InlineToken::Link { label, .. } if label == " ")
            {
                continue;
            }
        }
        let text = match token {
            InlineToken::Text(text, _) => Some(text.as_str()),
            InlineToken::Link { label, .. } => Some(label.as_str()),
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
                    InlineToken::Text(_, style) => {
                        ops.push(RenderOp::Text(part.to_string(), *style))
                    }
                    InlineToken::Link { url, style, .. } => ops.push(RenderOp::Link {
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
                InlineToken::Space(style) => ops.push(RenderOp::Text(" ".to_string(), *style)),
                InlineToken::Image { png, cols } => ops.push(RenderOp::ImageRow {
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

pub(crate) fn tokens_width(tokens: &[InlineToken]) -> usize {
    let mut widest = 0;
    let mut width = 0;
    for token in tokens {
        if matches!(token, InlineToken::Break) {
            widest = widest.max(width);
            width = 0;
        } else {
            width += token_width(token);
        }
    }
    widest.max(width)
}

pub(crate) enum InlineToken {
    Text(String, Style),
    Space(Style),
    Image {
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

fn token_width(t: &InlineToken) -> usize {
    match t {
        InlineToken::Text(s, _) => cell_width(s),
        InlineToken::Space(_) => 1,
        InlineToken::Image { cols, .. } => *cols as usize,
        InlineToken::Link { label, .. } => cell_width(label),
        InlineToken::Break => 0,
    }
}

pub(crate) fn inline_tokens(
    inls: &[Inline],
    base: Style,
    term: &TermInfo,
    assets: &Assets,
) -> Vec<InlineToken> {
    let mut tokens = Vec::new();
    for inl in inls {
        match inl {
            Inline::Text(t, s) => push_words(&mut tokens, t, merge(base, *s)),
            Inline::Code(t) => tokens.push(InlineToken::Text(
                format!(" {t} "),
                merge(base, code_style()),
            )),
            Inline::Link { label, url } => {
                let style = merge(
                    base,
                    Style {
                        underline: true,
                        ..Style::default()
                    },
                );
                for (unit, _) in break_units(label) {
                    tokens.push(InlineToken::Link {
                        label: unit.to_string(),
                        url: url.clone(),
                        style,
                    });
                }
            }
            Inline::InlineTypst { src, .. } => match assets.fragment(src, natural_ppi(term), false)
            {
                Ok(png) => {
                    let cell_width_px = term.cell_w_px.max(1) as f32;
                    let cell_height_px = term.cell_h_px.max(1) as f32;
                    let (image_width_px, image_height_px) = assets
                        .dimensions(&png)
                        .unwrap_or((cell_width_px as u32, cell_height_px as u32));
                    let cols = ((image_width_px as f32 / image_height_px as f32) * cell_height_px
                        / cell_width_px)
                        .ceil()
                        .max(1.0) as u16;
                    tokens.push(InlineToken::Image { png, cols });
                }
                Err(e) => push_words(&mut tokens, &format!("[typst error: {e}]"), base),
            },
            Inline::SoftBreak => tokens.push(InlineToken::Space(base)),
            Inline::HardBreak => tokens.push(InlineToken::Break),
            Inline::BlockFragment(_) => {}
        }
    }
    tokens
}

fn push_words(tokens: &mut Vec<InlineToken>, text: &str, style: Style) {
    for (unit, _) in break_units(text) {
        if unit == " " {
            tokens.push(InlineToken::Space(style));
        } else {
            tokens.push(InlineToken::Text(unit.to_string(), style));
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
    use crate::layout::split_lines;
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
            .all(|line| crate::layout::line_width(line) <= 5));
    }
}
