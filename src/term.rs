use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::rc::Rc;
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
    // Placements live for one frame: the id of an image already transmitted in this
    // frame, keyed by its path and cell size, so the lines of one image share it.
    let mut placed: HashMap<(PathBuf, u16, u16), u32> = HashMap::new();
    for op in ops {
        match op {
            RenderOp::MoveTo(c, r) => queue!(out, cursor::MoveTo(*c, *r))?,
            RenderOp::LineBreak => queue!(out, Print("\r\n"))?,
            RenderOp::Text(t, style) => emit_text(out, t, *style)?,
            RenderOp::ClearImages => {
                placed.clear();
                out.write_all(b"\x1b_Ga=d,d=A,q=2\x1b\\")?
            }
            RenderOp::Image {
                png_path,
                cols,
                rows,
            } => emit_image(out, png_path, *cols, *rows)?,
            RenderOp::InlineImage {
                png_path,
                cols,
                rows,
                row,
            } => emit_inline_image(out, png_path, *cols, *rows, *row, &mut placed)?,
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
    if style.quote {
        queue!(out, SetBackgroundColor(Color::AnsiValue(237)))?;
    }
    if style.code {
        queue!(out, SetBackgroundColor(Color::AnsiValue(236)))?;
    }
    queue!(out, Print(text))?;
    if style.bold || style.italic || style.underline || style.dim || style.code || style.quote {
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
// Base64-encoded PNG bytes, cached by path. Cache paths are content-hashed, so the
// encoding is stable for the process and the file is read and encoded only once.
fn encoded_png(png_path: &Path) -> Result<Rc<str>> {
    thread_local! {
        static CACHE: RefCell<HashMap<PathBuf, Rc<str>>> = RefCell::new(HashMap::new());
    }
    if let Some(b64) = CACHE.with(|c| c.borrow().get(png_path).cloned()) {
        return Ok(b64);
    }
    let b64: Rc<str> = base64::engine::general_purpose::STANDARD
        .encode(fs::read(png_path)?)
        .into();
    CACHE.with(|c| c.borrow_mut().insert(png_path.to_path_buf(), Rc::clone(&b64)));
    Ok(b64)
}

fn emit_image(out: &mut impl Write, png_path: &Path, cols: u16, rows: u16) -> Result<()> {
    let id = IMAGE_ID.fetch_add(1, Ordering::Relaxed);
    let b64 = encoded_png(png_path)?;
    transmit_chunks(
        out,
        &b64,
        &format!("\x1b_Gf=100,a=T,i={id},c={cols},r={rows},C=1,q=2"),
    )
}

// Inline image via kitty Unicode placeholders: transmit (a=t), create a virtual
// placement spanning `rows` rows, then emit U+10EEEE cells carrying the image id in
// the foreground color and the row/column index in combining diacritics. A taller
// placement is drawn one line per call, so the caller keeps the text grid intact
// around it; transmission happens on whichever line of the image comes first.
fn emit_inline_image(
    out: &mut impl Write,
    png_path: &Path,
    cols: u16,
    rows: u16,
    row: u16,
    placed: &mut HashMap<(PathBuf, u16, u16), u32>,
) -> Result<()> {
    let key = (png_path.to_path_buf(), cols, rows);
    let id = match placed.get(&key) {
        Some(id) => *id,
        None => {
            let id = IMAGE_ID.fetch_add(1, Ordering::Relaxed);
            let b64 = encoded_png(png_path)?;
            transmit_chunks(out, &b64, &format!("\x1b_Gf=100,a=t,t=d,i={id},q=2"))?;
            write!(out, "\x1b_Ga=p,U=1,i={id},c={cols},r={rows},q=2\x1b\\")?;
            placed.insert(key, id);
            id
        }
    };
    emit_placeholder_row(out, id, cols, row)
}

fn emit_placeholder_row(out: &mut impl Write, id: u32, cols: u16, row: u16) -> Result<()> {
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
    let row = DIACRITICS[row as usize % DIACRITICS.len()];
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
        emit_placeholder_row(&mut buf, 42, 3, 0).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.starts_with("\x1b[38;2;0;0;42m"));
        assert!(s.ends_with("\x1b[39m"));
        assert_eq!(s.matches('\u{10EEEE}').count(), 3);
        assert!(s.contains(DIACRITICS[0]));
        assert!(s.contains(DIACRITICS[1]));
        assert!(s.contains(DIACRITICS[2]));
    }

    #[test]
    fn placeholder_row_index_selects_slice() {
        let mut buf = Vec::new();
        emit_placeholder_row(&mut buf, 7, 2, 3).unwrap();
        let s = String::from_utf8(buf).unwrap();
        let cells: Vec<&str> = s.split('\u{10EEEE}').skip(1).collect();
        assert_eq!(cells.len(), 2);
        for cell in cells {
            assert!(cell.starts_with(DIACRITICS[3]), "row 3 diacritic leads each cell");
        }
    }
}
