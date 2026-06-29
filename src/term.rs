use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};

use anyhow::Result;
use base64::Engine;
use crossterm::style::{Attribute, Color, Print, SetAttribute, SetBackgroundColor};
use crossterm::{cursor, queue, terminal};

use crate::diacritics::DIACRITICS;
use crate::core::ir::{RenderOp, Style};
use crate::layout::TermInfo;

// A fresh image id per emit. Ghostty's image-id reuse is broken (#6711), so a kept
// image cannot be re-placed by id; every draw transmits the PNG anew.
static IMAGE_ID: AtomicU32 = AtomicU32::new(1);

// Transmit base64 PNG data in <=4096-byte chunks; `first` is the control prefix for
// chunk 0 (its `,m=...;` and the `\x1b_G` framing are added here).
fn transmit_chunks(out: &mut impl Write, b64: &str, first: &str) -> Result<()> {
    let chunks: Vec<&[u8]> = b64.as_bytes().chunks(4096).collect();
    for (i, chunk) in chunks.iter().enumerate() {
        let m = if i == chunks.len() - 1 { 0 } else { 1 };
        if i == 0 {
            write!(out, "{first},m={m};")?;
        } else {
            write!(out, "\x1b_Gm={m};")?;
        }
        out.write_all(chunk)?;
        out.write_all(b"\x1b\\")?;
    }
    Ok(())
}

impl TermInfo {
    pub fn query() -> Self {
        let (cols, rows) = terminal::size().unwrap_or((80, 24));
        let (mut cw, mut ch) = (8u16, 16u16);
        if let Ok(ws) = terminal::window_size() {
            if ws.width > 0 && ws.height > 0 && cols > 0 && rows > 0 {
                cw = (ws.width / cols).max(1);
                ch = (ws.height / rows).max(1);
            }
        }
        TermInfo {
            cols,
            rows,
            cell_w_px: cw,
            cell_h_px: ch,
        }
    }

    // Right after startup the reported size can churn (stale or zero) before the
    // terminal settles, which would rasterize math against a wrong cell. Poll until
    // two consecutive readings agree, then trust it; take what we have after ~500ms.
    pub fn acquire() -> Self {
        let mut prev = Self::query();
        for _ in 0..50 {
            std::thread::sleep(std::time::Duration::from_millis(10));
            let cur = Self::query();
            if (cur.cell_w_px, cur.cell_h_px) == (prev.cell_w_px, prev.cell_h_px) {
                return cur;
            }
            prev = cur;
        }
        prev
    }
}

pub fn emit(ops: &[RenderOp], out: &mut impl Write) -> Result<()> {
    for op in ops {
        match op {
            RenderOp::MoveTo(c, r) => queue!(out, cursor::MoveTo(*c, *r))?,
            RenderOp::LineBreak => queue!(out, Print("\r\n"))?,
            RenderOp::Text(t, style) => emit_text(out, t, *style)?,
            RenderOp::ClearImages => out.write_all(b"\x1b_Ga=d,d=A,q=2\x1b\\")?,
            RenderOp::Image {
                png_path,
                cols,
                rows,
            } => emit_image(out, png_path, *cols, *rows)?,
            RenderOp::InlineImage { png_path, cols } => emit_inline_image(out, png_path, *cols)?,
            RenderOp::Link { label, url, style } => emit_link(out, label, url, *style)?,
        }
    }
    out.flush()?;
    Ok(())
}

fn emit_text(out: &mut impl Write, text: &str, style: Style) -> Result<()> {
    if style.bold {
        queue!(out, SetAttribute(Attribute::Bold))?;
    }
    if style.italic {
        queue!(out, SetAttribute(Attribute::Italic))?;
    }
    if style.underline {
        queue!(out, SetAttribute(Attribute::Underlined))?;
    }
    if style.dim {
        queue!(out, SetAttribute(Attribute::Dim))?;
    }
    if style.code {
        queue!(out, SetBackgroundColor(Color::AnsiValue(236)))?;
    }
    queue!(out, Print(text))?;
    if style.bold || style.italic || style.underline || style.dim || style.code {
        queue!(out, SetAttribute(Attribute::Reset))?;
    }
    Ok(())
}

// OSC 8 hyperlink wrapping the underlined label, so the terminal makes it clickable.
fn emit_link(out: &mut impl Write, label: &str, url: &str, style: Style) -> Result<()> {
    write!(out, "\x1b]8;;{url}\x1b\\")?;
    emit_text(out, label, style)?;
    write!(out, "\x1b]8;;\x1b\\")?;
    Ok(())
}

// Kitty graphics protocol: transmit the PNG (f=100) and place it at the cursor
// (a=T) sized c=cols,r=rows with C=1 to suppress cursor move.
fn emit_image(out: &mut impl Write, png_path: &Path, cols: u16, rows: u16) -> Result<()> {
    let id = IMAGE_ID.fetch_add(1, Ordering::Relaxed);
    let b64 = base64::engine::general_purpose::STANDARD.encode(fs::read(png_path)?);
    transmit_chunks(
        out,
        &b64,
        &format!("\x1b_Gf=100,a=T,i={id},c={cols},r={rows},C=1,q=2"),
    )
}

// Inline image via kitty Unicode placeholders: transmit (a=t), create a virtual
// placement spanning one row, then emit U+10EEEE cells carrying the image id in the
// foreground color and the row/column index in combining diacritics.
fn emit_inline_image(out: &mut impl Write, png_path: &Path, cols: u16) -> Result<()> {
    let id = IMAGE_ID.fetch_add(1, Ordering::Relaxed);
    let b64 = base64::engine::general_purpose::STANDARD.encode(fs::read(png_path)?);
    transmit_chunks(out, &b64, &format!("\x1b_Gf=100,a=t,t=d,i={id},q=2"))?;
    write!(out, "\x1b_Ga=p,U=1,i={id},c={cols},r=1,q=2\x1b\\")?;
    emit_placeholder_row(out, id, cols)
}

fn emit_placeholder_row(out: &mut impl Write, id: u32, cols: u16) -> Result<()> {
    // The image id must be the cell's 24-bit foreground RGB (id N => 0,0,N). The
    // 256-color form sets a palette index, a different color, so the placeholder
    // would bind to the wrong (or no) image.
    write!(
        out,
        "\x1b[38;2;{};{};{}m",
        (id >> 16) & 0xff,
        (id >> 8) & 0xff,
        id & 0xff
    )?;
    let row = DIACRITICS[0];
    for c in 0..cols as usize {
        write!(out, "\u{10EEEE}{row}{}", DIACRITICS[c % DIACRITICS.len()])?;
    }
    write!(out, "\x1b[39m")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_row_encoding() {
        let mut buf = Vec::new();
        emit_placeholder_row(&mut buf, 42, 3).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.starts_with("\x1b[38;2;0;0;42m"));
        assert!(s.ends_with("\x1b[39m"));
        assert_eq!(s.matches('\u{10EEEE}').count(), 3);
        assert!(s.contains(DIACRITICS[0]));
        assert!(s.contains(DIACRITICS[1]));
        assert!(s.contains(DIACRITICS[2]));
    }
}
