use std::collections::HashMap;
use std::fs;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Instant;

use anyhow::{ensure, Context, Result};
use base64::Engine;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::style::{
    Attribute, Color, Print, SetAttribute, SetBackgroundColor, SetForegroundColor,
};
use crossterm::{cursor, execute, queue, terminal};

use crate::core::ir::Style;
use crate::diacritics::DIACRITICS;
use crate::layout::{GlowRun, RenderOp, TermInfo};

pub struct TerminalSession {
    raw: bool,
    alternate: bool,
}

impl TerminalSession {
    pub fn new() -> Result<Self> {
        let mut session = Self {
            raw: false,
            alternate: false,
        };
        terminal::enable_raw_mode()?;
        session.raw = true;
        session.alternate = true;
        execute!(
            std::io::stdout(),
            terminal::EnterAlternateScreen,
            EnableMouseCapture,
            cursor::Hide
        )?;
        Ok(session)
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        if self.alternate {
            let mut out = std::io::stdout();
            let _ = out.write_all(b"\x1b]9;4;0;\x1b\\");
            let _ = execute!(out, cursor::Show);
            let _ = execute!(out, DisableMouseCapture);
            let _ = execute!(out, terminal::LeaveAlternateScreen);
        }
        if self.raw {
            let _ = terminal::disable_raw_mode();
        }
    }
}

struct EncodedImage {
    data: Rc<str>,
    width: u32,
    height: u32,
}

type PlacementKey = (PathBuf, u16, u16, u16);

pub struct Emitter {
    images: HashMap<(PathBuf, u16), Rc<EncodedImage>>,
    image_ids: Vec<u32>,
    next_image_id: u32,
    start: Instant,
}

impl Default for Emitter {
    fn default() -> Self {
        Self::new()
    }
}

impl Emitter {
    pub fn new() -> Self {
        Self {
            images: HashMap::new(),
            image_ids: Vec::new(),
            next_image_id: 1,
            start: Instant::now(),
        }
    }

    pub fn clear_images(&mut self) {
        self.images.clear();
    }

    fn image_id(&mut self) -> u32 {
        let id = self.next_image_id;
        self.next_image_id = id.wrapping_add(1).max(1);
        self.image_ids.push(id);
        id
    }

    fn phase(&self) -> u32 {
        (self.start.elapsed().as_millis() / 100) as u32
    }
}

// Transmit base64 PNG data in <=4096-byte chunks; `first` is the control prefix for
// chunk 0 (its `,m=...;` and the `\x1b_G` framing are added here).
fn transmit_chunks(out: &mut impl Write, b64: &str, first: &str) -> Result<()> {
    let mut chunks = b64.as_bytes().chunks(4096).enumerate().peekable();
    while let Some((i, chunk)) = chunks.next() {
        let m = usize::from(chunks.peek().is_some());
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

impl Emitter {
    pub fn emit(&mut self, ops: &[RenderOp], out: &mut impl Write) -> Result<()> {
        // Placements live for one frame: the id of an image already transmitted in this
        // frame, keyed by its path and cell size, so the lines of one image share it.
        let mut placed: HashMap<PlacementKey, u32> = HashMap::new();
        let phase = self.phase();
        for op in ops {
            match op {
                RenderOp::MoveTo(c, r) => queue!(out, cursor::MoveTo(*c, *r))?,
                RenderOp::LineBreak => queue!(out, Print("\r\n"))?,
                RenderOp::Text(t, style) => emit_text(out, t, *style, phase)?,
                RenderOp::ClearImages => {
                    placed.clear();
                    for id in &self.image_ids {
                        write!(out, "\x1b_Ga=d,d=I,i={id},q=2\x1b\\")?;
                    }
                    self.image_ids.clear();
                }
                RenderOp::ImageRow {
                    png_path,
                    cols,
                    rows,
                    row,
                } => self.emit_inline_image(out, png_path, *cols, *rows, *row, &mut placed)?,
                RenderOp::Link { label, url, style } => emit_link(out, label, url, *style, phase)?,
                // A click target only; it draws nothing.
                RenderOp::ToggleTarget(_) => {}
            }
        }
        out.flush()?;
        Ok(())
    }

    // Repaint the glowing runs where they stand. Nothing is laid out again and no image is
    // transmitted, so this is cheap enough to do every tick.
    pub fn paint_glow(&mut self, out: &mut impl Write, runs: &[GlowRun]) -> Result<()> {
        let phase = self.phase();
        for run in runs {
            queue!(out, cursor::MoveTo(run.col, run.row))?;
            glow_text(out, &run.text, phase)?;
        }
        out.flush()?;
        Ok(())
    }
}

// One glowing run, in the hue the clock is at. Every run reads the same clock, so all the
// boxes on a slide hold one colour together, and a redraw picks the shimmer up where it
// stands instead of restarting it.
fn glow_text(out: &mut impl Write, text: &str, phase: u32) -> Result<()> {
    let (r, g, b) = hue(phase);
    queue!(
        out,
        SetForegroundColor(Color::Rgb { r, g, b }),
        Print(text),
        SetForegroundColor(Color::Reset)
    )?;
    Ok(())
}

// One full turn of hue every 256 steps, at full saturation.
fn hue(t: u32) -> (u8, u8, u8) {
    let h = t % 256;
    let seg = h / 43;
    let f = ((h % 43) * 255 / 43) as u8;
    match seg {
        0 => (255, f, 0),
        1 => (255 - f, 255, 0),
        2 => (0, 255, f),
        3 => (0, 255 - f, 255),
        4 => (f, 0, 255),
        _ => (255, 0, 255 - f),
    }
}

fn emit_text(out: &mut impl Write, text: &str, style: Style, phase: u32) -> Result<()> {
    if style.glow {
        return glow_text(out, text, phase);
    }
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
fn emit_link(out: &mut impl Write, label: &str, url: &str, style: Style, phase: u32) -> Result<()> {
    write!(out, "\x1b]8;;{url}\x1b\\")?;
    emit_text(out, label, style, phase)?;
    write!(out, "\x1b]8;;\x1b\\")?;
    Ok(())
}

impl Emitter {
    fn encoded_png(&mut self, path: &Path, rows: u16) -> Result<Rc<EncodedImage>> {
        let expanded_rows = if usize::from(rows) > DIACRITICS.len() {
            rows
        } else {
            0
        };
        let key = (path.to_owned(), expanded_rows);
        if let Some(image) = self.images.get(&key) {
            return Ok(Rc::clone(image));
        }
        let mut bytes = fs::read(path)?;
        let (mut width, mut height) = image::ImageReader::new(Cursor::new(&bytes))
            .with_guessed_format()?
            .into_dimensions()?;
        if height < u32::from(expanded_rows) {
            let target_height = u32::from(expanded_rows);
            width = ((u64::from(width) * u64::from(target_height) + u64::from(height) / 2)
                / u64::from(height))
            .max(1)
            .try_into()
            .context("expanded image width")?;
            let expanded = image::load_from_memory(&bytes)?.resize_exact(
                width,
                target_height,
                image::imageops::FilterType::Nearest,
            );
            let mut png = Cursor::new(Vec::new());
            expanded.write_to(&mut png, image::ImageFormat::Png)?;
            bytes = png.into_inner();
            height = target_height;
        }
        let image = Rc::new(EncodedImage {
            data: base64::engine::general_purpose::STANDARD
                .encode(bytes)
                .into(),
            width,
            height,
        });
        self.images.insert(key, Rc::clone(&image));
        Ok(image)
    }

    fn emit_inline_image(
        &mut self,
        out: &mut impl Write,
        path: &Path,
        cols: u16,
        rows: u16,
        row: u16,
        placed: &mut HashMap<PlacementKey, u32>,
    ) -> Result<()> {
        ensure!(row < rows, "image row exceeds placement height");
        let start = row / DIACRITICS.len() as u16 * DIACRITICS.len() as u16;
        let tile_rows = (rows - start).min(DIACRITICS.len() as u16);
        let key = (path.to_owned(), cols, rows, start);
        let id = match placed.get(&key) {
            Some(id) => *id,
            None => {
                let id = self.image_id();
                let image = self.encoded_png(path, rows)?;
                let y = u64::from(start) * u64::from(image.height) / u64::from(rows);
                let end = u64::from(start + tile_rows) * u64::from(image.height) / u64::from(rows);
                let height = end - y;
                transmit_chunks(out, &image.data, &format!("\x1b_Gf=100,a=t,t=d,i={id},q=2"))?;
                write!(out, "\x1b_Ga=p,U=1,i={id},c={cols},r={tile_rows},x=0,y={y},w={},h={height},q=2\x1b\\", image.width)?;
                placed.insert(key, id);
                id
            }
        };
        emit_placeholder_row(out, id, cols, row - start)
    }
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
    let row = DIACRITICS
        .get(row as usize)
        .context("image row exceeds placeholder range")?;
    for c in 0..cols as usize {
        if let Some(column) = DIACRITICS.get(c) {
            write!(
                out,
                "\u{10EEEE}{row}{column}{}",
                DIACRITICS[(id >> 24) as usize]
            )?;
        } else {
            write!(out, "\u{10EEEE}{row}")?;
        }
    }
    write!(out, "\x1b[39m")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clear_deletes_only_owned_image_ids() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("image.png");
        image::RgbImage::new(1, 1).save(&path).unwrap();
        let mut emitter = Emitter::new();
        let mut output = Vec::new();
        emitter
            .emit(
                &[RenderOp::ImageRow {
                    png_path: path,
                    cols: 1,
                    rows: 1,
                    row: 0,
                }],
                &mut output,
            )
            .unwrap();
        emitter.clear_images();
        output.clear();
        emitter.emit(&[RenderOp::ClearImages], &mut output).unwrap();
        assert_eq!(output, b"\x1b_Ga=d,d=I,i=1,q=2\x1b\\");
        output.clear();
        emitter.emit(&[RenderOp::ClearImages], &mut output).unwrap();
        assert!(output.is_empty());
    }

    #[test]
    fn tall_images_use_distinct_cropped_tiles() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tall.png");
        image::RgbImage::new(10, 600).save(&path).unwrap();
        let ops: Vec<_> = [296, 297, 298, 599]
            .into_iter()
            .map(|row| RenderOp::ImageRow {
                png_path: path.clone(),
                cols: 5,
                rows: 600,
                row,
            })
            .collect();
        let mut output = Vec::new();
        Emitter::new().emit(&ops, &mut output).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert_eq!(output.matches("a=t,t=d").count(), 3);
        assert!(output.contains("i=1,c=5,r=297,x=0,y=0,w=10,h=297,"));
        assert!(output.contains("i=2,c=5,r=297,x=0,y=297,w=10,h=297,"));
        assert!(output.contains("i=3,c=5,r=6,x=0,y=594,w=10,h=6,"));
        assert!(output.contains(&format!("\x1b[38;2;0;0;2m\u{10EEEE}{}", DIACRITICS[0])));
        assert!(output.contains(&format!("\x1b[38;2;0;0;3m\u{10EEEE}{}", DIACRITICS[5])));
    }

    #[test]
    fn enlarged_tiny_image_has_nonempty_source_tiles() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tiny.png");
        image::RgbImage::from_pixel(1, 1, image::Rgb([1, 2, 3]))
            .save(&path)
            .unwrap();
        let mut emitter = Emitter::new();
        let expanded = emitter.encoded_png(&path, 600).unwrap();
        assert_eq!((expanded.width, expanded.height), (600, 600));
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(expanded.data.as_bytes())
            .unwrap();
        let pixels = image::load_from_memory(&decoded).unwrap().to_rgb8();
        assert_eq!(pixels.get_pixel(599, 599), &image::Rgb([1, 2, 3]));
        let mut output = Vec::new();
        emitter
            .emit(
                &[RenderOp::ImageRow {
                    png_path: path,
                    cols: 1200,
                    rows: 600,
                    row: 599,
                }],
                &mut output,
            )
            .unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("c=1200,r=6,x=0,y=594,w=600,h=6,"));
        assert_eq!(output.matches('\u{10EEEE}').count(), 1200);
    }

    #[test]
    fn image_chunks_mark_only_the_last_chunk_complete() {
        let mut out = Vec::new();
        let first = "a".repeat(4096);
        transmit_chunks(&mut out, &format!("{first}tail"), "\x1b_Gf=100").unwrap();
        assert_eq!(
            out,
            format!("\x1b_Gf=100,m=1;{first}\x1b\\\x1b_Gm=0;tail\x1b\\").as_bytes()
        );
        out.clear();
        transmit_chunks(&mut out, &first, "\x1b_Gf=100").unwrap();
        assert_eq!(out, format!("\x1b_Gf=100,m=0;{first}\x1b\\").as_bytes());
    }

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
            assert!(
                cell.starts_with(DIACRITICS[3]),
                "row 3 diacritic leads each cell"
            );
        }
    }
}
