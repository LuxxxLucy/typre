mod commands;
mod core;
mod diacritics;
mod layout;
mod precompile;
mod render;
mod term;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{stdout, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Parser;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
    MouseButton, MouseEventKind,
};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::{cursor, execute, queue, terminal};
use notify::{RecursiveMode, Watcher};

use crate::core::ir::{Align, Block, Deck, Meta, RenderOp, Style, TocEntry};
use crate::core::{ir, parse};
use crate::layout::{layout, viewport, TermInfo};
use crate::precompile::typst_precompile_errors;
use crate::render::paint::{dim_style, heading_style, hrule, pad, Hit, HitAction};
use crate::term::emit;

#[derive(Parser)]
#[command(about = "Terminal typst slideshow")]
struct Cli {
    deck: PathBuf,
    /// Print the parsed Deck and per-slide RenderOps to stdout.
    #[arg(long)]
    dump_ops: bool,
    /// Run the full pipeline (typst + byte lowering) to a file or stdout.
    #[arg(long)]
    export: bool,
    /// Output path for --export.
    #[arg(short, long)]
    output: Option<PathBuf>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let deck_dir = deck_dir_of(&cli.deck);

    if cli.dump_ops || cli.export {
        let md = fs::read_to_string(&cli.deck)
            .with_context(|| format!("read {}", cli.deck.display()))?;
        let deck = build_deck(&md);
        if cli.dump_ops {
            return dump_ops(&deck, &deck_dir);
        }
        return export(&deck, &deck_dir, cli.output.as_deref());
    }
    run(&cli.deck)
}

fn export_term() -> TermInfo {
    TermInfo {
        cols: 80,
        rows: 24,
        cell_w_px: 9,
        cell_h_px: 18,
    }
}

fn dump_ops(deck: &ir::Deck, deck_dir: &Path) -> Result<()> {
    println!("{deck:#?}");
    let term = export_term();
    for (i, slide) in deck.slides.iter().enumerate() {
        println!("\n=== slide {i} render ops ===");
        let ops = render::render(slide, &term, deck_dir);
        println!("{ops:#?}");
    }
    Ok(())
}

fn export(deck: &ir::Deck, deck_dir: &Path, output: Option<&Path>) -> Result<()> {
    let term = export_term();
    let mut buf: Vec<u8> = Vec::new();
    let total = deck.slides.len();
    let open = HashSet::new();
    for (i, slide) in deck.slides.iter().enumerate() {
        let f = frame(slide, &term, deck_dir, i, total, &deck.meta, &open, 0);
        emit(&f.ops, &mut buf)?;
    }
    match output {
        Some(path) => fs::write(path, &buf).with_context(|| format!("write {}", path.display()))?,
        None => stdout().write_all(&buf)?,
    }
    Ok(())
}

// Navigation: the key map and the saturating page state. All clamping lives here.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
    Next,
    Prev,
    First,
    Last,
    Digit(usize),
    ToggleHelp,
    Cancel,
    Quit,
    Ignore,
}

fn command_for(code: KeyCode, ctrl: bool) -> Command {
    use KeyCode::*;
    match code {
        Char('c') if ctrl => Command::Quit,
        Char('q') => Command::Quit,
        Esc => Command::Cancel,
        Char('?') => Command::ToggleHelp,
        Right | Down | Char(' ') | Char('j') | Char('l') => Command::Next,
        Left | Up | Char('k') | Char('h') => Command::Prev,
        Char('g') => Command::First,
        Char('G') => Command::Last,
        Char(d) if d.is_ascii_digit() => Command::Digit(d.to_digit(10).unwrap() as usize),
        _ => Command::Ignore,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Nav {
    page: usize,
    len: usize,
    pending: Option<usize>,
}

impl Nav {
    fn new(len: usize) -> Self {
        Nav {
            page: 0,
            len: len.max(1),
            pending: None,
        }
    }

    fn page(&self) -> usize {
        self.page
    }

    fn last(&self) -> usize {
        self.len - 1
    }

    // Apply a movement command; returns whether the page changed (so the caller redraws).
    fn apply(&mut self, cmd: Command) -> bool {
        let prev = self.page;
        match cmd {
            Command::Next => self.page = (self.page + 1).min(self.last()),
            Command::Prev => self.page = self.page.saturating_sub(1),
            Command::First => self.page = 0,
            Command::Last => {
                self.page = match self.pending.take() {
                    Some(n) => n.saturating_sub(1).min(self.last()),
                    None => self.last(),
                };
            }
            Command::Digit(d) => {
                self.pending = Some(self.pending.unwrap_or(0) * 10 + d);
                return false;
            }
            Command::ToggleHelp | Command::Cancel | Command::Quit | Command::Ignore => return false,
        }
        self.pending = None;
        self.page != prev
    }

    // Jump to a page (clamped); returns whether the page changed.
    fn goto(&mut self, page: usize) -> bool {
        let prev = self.page;
        self.page = page.min(self.last());
        self.pending = None;
        self.page != prev
    }

    // Re-clamp after a live reload changes the slide count.
    fn set_len(&mut self, len: usize) {
        self.len = len.max(1);
        self.page = self.page.min(self.last());
    }
}

// Status bar and overlays: the non-content screen furniture.

// Bottom status bar: separator, a "Contents" button, metadata, "? help", and the
// slide counter. The returned hit makes the button jump to the title slide.
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
    let used = button.chars().count()
        + meta_text.chars().count()
        + hint.chars().count()
        + right.chars().count()
        + 4;
    let gap = content_w.saturating_sub(used).max(1);
    ops.push(RenderOp::Text(button.to_string(), bold));
    ops.push(RenderOp::Text(meta_text, dim));
    ops.push(RenderOp::Text(" ".repeat(gap), Style::default()));
    ops.push(RenderOp::Text(format!("{hint}    "), dim));
    ops.push(RenderOp::Text(right, bold));
    let hit = Hit {
        row,
        cols: mx..mx + button.chars().count() as u16,
        action: HitAction::Goto(0),
    };
    (ops, hit)
}

// A centered, bordered box; each line carries its own style.
fn centered_box(term: &TermInfo, lines: &[(String, Style)]) -> Vec<RenderOp> {
    let w = lines.iter().map(|(l, _)| l.chars().count()).max().unwrap_or(0);
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
        "click ▸       expand a details box",
        "click link    open in browser",
        "Shift+drag    select text",
        "?             toggle this help",
        "Esc           close help",
        "q  Ctrl-C     quit",
    ];
    let styled: Vec<(String, Style)> = lines
        .iter()
        .enumerate()
        .map(|(i, l)| (l.to_string(), if i == 0 { heading_style() } else { Style::default() }))
        .collect();
    centered_box(term, &styled)
}

// A centered status box shown while the deck's typst fragments compile.
fn loading_frame(term: &TermInfo, label: &str) -> Vec<RenderOp> {
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
fn error_gate(term: &TermInfo, errors: &[(usize, String)]) -> Vec<RenderOp> {
    let max_w = (term.cols as usize).saturating_sub(8).max(20);
    let rows = term.rows as usize;
    let mut lines: Vec<(String, Style)> = vec![
        ("Typst compile errors".to_string(), heading_style()),
        (String::new(), Style::default()),
    ];
    let budget = rows.saturating_sub(8).max(2);
    let mut shown = 0;
    for (n, msg) in errors {
        if shown >= budget {
            lines.push((format!("… and {} more", errors.len() - shown), dim_style()));
            break;
        }
        let first = msg.lines().next().unwrap_or("");
        lines.push((truncate(&format!("slide {n}: {first}"), max_w), Style::default()));
        shown += 1;
    }
    lines.push((String::new(), Style::default()));
    lines.push(("Continue opening?   [y]es    [n]o / quit".to_string(), heading_style()));
    centered_box(term, &lines)
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

// Frame composition: window the rendered body to the scroll offset and add the
// status bar and scroll markers. Everything the body needs is already determined.

// A composed slide screen plus its click targets and total body height.
#[derive(Debug)]
struct Frame {
    ops: Vec<RenderOp>,
    hits: Vec<Hit>,
    height: usize,
}

// Parse and run the deck-level prepare pass that the IR needs before rendering.
fn build_deck(md: &str) -> Deck {
    let mut deck = parse::parse(md);
    attach_toc(&mut deck);
    deck
}

// Populate the opening title slide's table of contents with every section slide
// (those leading with an H2) and the slide index each jumps to.
fn attach_toc(deck: &mut Deck) {
    let entries: Vec<TocEntry> = deck
        .slides
        .iter()
        .enumerate()
        .filter_map(|(i, s)| match s.blocks.first() {
            Some(Block::Heading(2, inls)) => Some(TocEntry {
                index: i,
                title: crate::render::inline::flat_text(inls),
            }),
            _ => None,
        })
        .collect();
    if let Some(first) = deck.slides.first_mut() {
        if first.is_title() {
            first.toc = entries;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn frame(
    slide: &ir::Slide,
    term: &TermInfo,
    deck_dir: &Path,
    idx: usize,
    total: usize,
    meta: &Meta,
    open: &HashSet<usize>,
    scroll: usize,
) -> Frame {
    use std::panic::{catch_unwind, AssertUnwindSafe};
    let (body, hits, height) =
        catch_unwind(AssertUnwindSafe(|| render::body(slide, term, deck_dir, open))).unwrap_or_else(
            |_| {
                (
                    vec![
                        RenderOp::Text(
                            format!("[slide {} failed to render]", idx + 1),
                            Style::default(),
                        ),
                        RenderOp::LineBreak,
                    ],
                    Vec::new(),
                    1,
                )
            },
        );
    let vp = viewport(term);
    let scroll = scroll.min(height.saturating_sub(vp));
    let (wbody, mut hits) = window(body, hits, scroll, vp);
    let mut ops = vec![RenderOp::ClearImages, RenderOp::MoveTo(0, 0)];
    ops.extend(wbody);
    let (bar, home) = status_bar(term, idx, total, meta);
    ops.extend(bar);
    hits.push(home);
    ops.extend(scrollbar(term, scroll, vp, height));
    Frame { ops, hits, height }
}

// A vertical scrollbar on the right column: a track with a thumb sized to the
// visible fraction and positioned by the scroll offset. Empty when all fits.
fn scrollbar(term: &TermInfo, scroll: usize, vp: usize, height: usize) -> Vec<RenderOp> {
    if height <= vp || vp == 0 {
        return Vec::new();
    }
    let (margin, content_w) = layout(term);
    let col = (margin + content_w).min(term.cols.saturating_sub(1) as usize) as u16;
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

// Keep only body rows [scroll, scroll+vp); shift hit rows to match.
fn window(ops: Vec<RenderOp>, hits: Vec<Hit>, scroll: usize, vp: usize) -> (Vec<RenderOp>, Vec<Hit>) {
    let mut rows: Vec<Vec<RenderOp>> = vec![Vec::new()];
    for op in ops {
        match op {
            RenderOp::LineBreak => rows.push(Vec::new()),
            other => rows.last_mut().unwrap().push(other),
        }
    }
    let end = (scroll + vp).min(rows.len());
    let mut out = Vec::new();
    for row in rows.into_iter().take(end).skip(scroll) {
        out.extend(row);
        out.push(RenderOp::LineBreak);
    }
    let hits = hits
        .into_iter()
        .filter_map(|h| {
            let r = h.row as usize;
            (r >= scroll && r < scroll + vp).then(|| Hit {
                row: (r - scroll) as u16,
                cols: h.cols,
                action: h.action,
            })
        })
        .collect();
    (out, hits)
}

// The interactive show: prepare (parse + precompile) then present (event loop).

// Open a hyperlink in the OS default handler, detached; ignore failure.
fn open_url(url: &str) {
    let cmd = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let _ = std::process::Command::new(cmd).arg(url).spawn();
}

// The deck's directory, used as the base for relative image/typst paths and the
// file watcher. A bare filename has an empty parent; treat that as ".".
fn deck_dir_of(deck_path: &Path) -> PathBuf {
    match deck_path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    }
}

fn run(deck_path: &Path) -> Result<()> {
    let deck_dir = deck_dir_of(deck_path);
    let mut out = stdout();
    enable_raw_mode()?;
    execute!(out, EnterAlternateScreen, EnableMouseCapture, cursor::Hide)?;

    let result = present(deck_path, &deck_dir, &mut out);

    let _ = out.write_all(b"\x1b]9;4;0;\x1b\\"); // clear the terminal progress bar
    execute!(out, cursor::Show, DisableMouseCapture, LeaveAlternateScreen)?;
    disable_raw_mode()?;
    result
}

fn load(deck_path: &Path) -> Deck {
    build_deck(&fs::read_to_string(deck_path).unwrap_or_default())
}

// Compile every slide's typst up front behind a loading frame. If any fragment
// fails, gate on a yes/no prompt; return whether to open the show.
fn precompile_gate(
    deck: &Deck,
    deck_dir: &Path,
    term: &TermInfo,
    out: &mut impl Write,
) -> Result<bool> {
    let total = deck.slides.len();
    let mut errors: Vec<(usize, String)> = Vec::new();
    for (i, slide) in deck.slides.iter().enumerate() {
        queue!(
            out,
            terminal::Clear(terminal::ClearType::All),
            cursor::MoveTo(0, 0)
        )?;
        emit(
            &loading_frame(term, &format!("Compiling slide {} / {}", i + 1, total)),
            out,
        )?;
        for e in typst_precompile_errors(slide, term, deck_dir) {
            errors.push((i + 1, e));
        }
    }
    if errors.is_empty() {
        return Ok(true);
    }
    queue!(
        out,
        terminal::Clear(terminal::ClearType::All),
        cursor::MoveTo(0, 0)
    )?;
    emit(&error_gate(term, &errors), out)?;
    loop {
        if let Event::Key(KeyEvent { code, kind, .. }) = event::read()? {
            if kind != event::KeyEventKind::Press {
                continue;
            }
            use crossterm::event::KeyCode::*;
            match code {
                Char('y') | Char('Y') | Enter => return Ok(true),
                Char('n') | Char('N') | Char('q') | Esc => return Ok(false),
                _ => {}
            }
        }
    }
}

fn present(deck_path: &Path, deck_dir: &Path, out: &mut impl Write) -> Result<()> {
    let mut deck = load(deck_path);
    let mut nav = Nav::new(deck.slides.len());
    let mut help = false;
    // Acquire a reliable cell pixel size before precompiling and reuse it; precompile
    // bakes the typst resolution from it, so a stale early reading would mis-size every
    // raster until the slide is re-rendered.
    let mut term = TermInfo::acquire();

    if !precompile_gate(&deck, deck_dir, &term, out)? {
        return Ok(());
    }

    // Reload on deck or imported-asset changes; ignore our own cache/temp writes.
    let (tx, rx) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(ev) = res {
            if ev
                .paths
                .iter()
                .any(|p| !p.to_string_lossy().contains(".typre-"))
            {
                let _ = tx.send(());
            }
        }
    })?;
    watcher.watch(deck_dir, RecursiveMode::NonRecursive)?;

    // Per-slide open details ids and the current slide's scroll offset.
    let mut open: HashMap<usize, HashSet<usize>> = HashMap::new();
    let mut scroll = 0usize;
    let empty = HashSet::new();
    let (mut hits, mut height) =
        draw(&deck, deck_dir, &term, nav.page(), help, scroll, &empty, out)?;
    // Ghostty truncates a freshly-transmitted image on its cold first decode; a second
    // paint of the same content shows it in full. After any content change we repaint
    // once to warm it. `settle` holds that pending one-shot repaint.
    let mut settle = true;

    loop {
        let mut dirty = false;
        let timeout = if settle { 0 } else { 100 };
        if event::poll(Duration::from_millis(timeout))? {
            match event::read()? {
                Event::Resize(..) => {
                    term = TermInfo::query();
                    dirty = true;
                }
                Event::Key(KeyEvent {
                    code,
                    modifiers,
                    kind,
                    ..
                }) if kind == event::KeyEventKind::Press => {
                    let ctrl = modifiers.contains(KeyModifiers::CONTROL);
                    match command_for(code, ctrl) {
                        Command::Quit => break,
                        // Esc closes the help overlay if open, otherwise quits.
                        Command::Cancel => {
                            if help {
                                help = false;
                                dirty = true;
                            } else {
                                break;
                            }
                        }
                        Command::ToggleHelp => {
                            help = !help;
                            dirty = true;
                        }
                        // Any movement key closes help and still performs the move.
                        cmd => {
                            let closing = help;
                            help = false;
                            if nav.apply(cmd) {
                                scroll = 0;
                                dirty = true;
                            } else if closing {
                                dirty = true;
                            }
                        }
                    }
                }
                // Left-click a details summary to toggle it or a link to open it;
                // the wheel scrolls an overflowing slide.
                Event::Mouse(me) => match me.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        let action = hits
                            .iter()
                            .find(|h| h.row == me.row && h.cols.contains(&me.column))
                            .map(|h| h.action.clone());
                        match action {
                            Some(HitAction::ToggleDetails(id)) => {
                                let set = open.entry(nav.page()).or_default();
                                if !set.remove(&id) {
                                    set.insert(id);
                                }
                                dirty = true;
                            }
                            Some(HitAction::OpenUrl(url)) => open_url(&url),
                            Some(HitAction::Goto(idx)) => {
                                if nav.goto(idx) {
                                    scroll = 0;
                                    dirty = true;
                                }
                            }
                            None => {}
                        }
                    }
                    MouseEventKind::ScrollDown => {
                        let max = height.saturating_sub(viewport(&term));
                        if scroll < max {
                            scroll = (scroll + 1).min(max);
                            dirty = true;
                        }
                    }
                    MouseEventKind::ScrollUp => {
                        if scroll > 0 {
                            scroll -= 1;
                            dirty = true;
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }

        if rx.try_iter().count() > 0 {
            deck = load(deck_path);
            nav.set_len(deck.slides.len());
            scroll = 0;
            dirty = true;
        }

        if dirty || settle {
            let cur = open.get(&nav.page()).unwrap_or(&empty);
            let (h, ht) = draw(&deck, deck_dir, &term, nav.page(), help, scroll, cur, out)?;
            hits = h;
            height = ht;
            // Arm the one-shot warm-up repaint only when this draw showed new content.
            settle = dirty;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn draw(
    deck: &Deck,
    deck_dir: &Path,
    term: &TermInfo,
    idx: usize,
    help: bool,
    scroll: usize,
    open: &HashSet<usize>,
    out: &mut impl Write,
) -> Result<(Vec<Hit>, usize)> {
    let total = deck.slides.len().max(1);
    queue!(
        out,
        terminal::Clear(terminal::ClearType::All),
        cursor::MoveTo(0, 0)
    )?;
    let mut hits = Vec::new();
    let mut height = 0;
    if let Some(slide) = deck.slides.get(idx) {
        let f = frame(slide, term, deck_dir, idx, total, &deck.meta, open, scroll);
        emit(&f.ops, out)?;
        hits = f.hits;
        height = f.height;
    }
    if help {
        emit(&help_overlay(term), out)?;
    }
    let pct = ((idx + 1) * 100 / total).min(100);
    write!(out, "\x1b]9;4;1;{pct}\x1b\\")?; // terminal progress bar for slide position
    out.flush()?;
    Ok((hits, height))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::parse::parse;

    fn term() -> TermInfo {
        TermInfo {
            cols: 40,
            rows: 20,
            cell_w_px: 8,
            cell_h_px: 16,
        }
    }

    fn ops_text(ops: &[RenderOp]) -> String {
        ops.iter()
            .filter_map(|o| match o {
                RenderOp::Text(t, _) => Some(t.as_str()),
                _ => None,
            })
            .collect()
    }

    fn drive(len: usize, keys: &str) -> usize {
        let mut nav = Nav::new(len);
        for ch in keys.chars() {
            nav.apply(command_for(KeyCode::Char(ch), false));
        }
        nav.page()
    }

    #[test]
    fn next_saturates_at_last() {
        assert_eq!(drive(11, "jjjjjjjjjj"), 10);
        assert_eq!(drive(11, "jjjjjjjjjjjjj"), 10);
    }

    #[test]
    fn prev_saturates_at_first() {
        assert_eq!(drive(11, "kkkk"), 0);
        assert_eq!(drive(11, "llkkkk"), 0);
    }

    #[test]
    fn forward_then_back() {
        assert_eq!(drive(11, "lll"), 3);
        assert_eq!(drive(11, "lllh"), 2);
    }

    #[test]
    fn first_and_last() {
        assert_eq!(drive(11, "llg"), 0);
        assert_eq!(drive(11, "G"), 10);
    }

    #[test]
    fn count_prefix_goto() {
        assert_eq!(drive(11, "3G"), 2);
        assert_eq!(drive(11, "11G"), 10);
    }

    #[test]
    fn goto_out_of_range_clamps() {
        assert_eq!(drive(11, "101G"), 10);
        assert_eq!(drive(11, "0G"), 0);
    }

    #[test]
    fn single_slide_stays() {
        assert_eq!(drive(1, "lllhhhG"), 0);
    }

    #[test]
    fn empty_deck_no_panic() {
        assert_eq!(drive(0, "lhG"), 0);
    }

    #[test]
    fn reload_clamps_page() {
        let mut nav = Nav::new(10);
        nav.apply(Command::Last);
        assert_eq!(nav.page(), 9);
        nav.set_len(3);
        assert_eq!(nav.page(), 2);
    }

    #[test]
    fn quit_help_and_cancel_are_distinct() {
        assert_eq!(command_for(KeyCode::Char('q'), false), Command::Quit);
        assert_eq!(command_for(KeyCode::Char('c'), true), Command::Quit);
        assert_eq!(command_for(KeyCode::Esc, false), Command::Cancel);
        assert_eq!(command_for(KeyCode::Char('?'), false), Command::ToggleHelp);
    }

    #[test]
    fn details_collapse_expand_and_hit() {
        let deck = parse("◊details[Summary]{\nbody line\n}\n");
        let slide = &deck.slides[0];
        let closed = frame(
            slide,
            &term(),
            Path::new("."),
            0,
            1,
            &Meta::default(),
            &HashSet::new(),
            0,
        );
        let toggles = closed
            .hits
            .iter()
            .filter(|h| matches!(h.action, HitAction::ToggleDetails(_)))
            .count();
        assert_eq!(toggles, 1, "summary is a click target");
        let txt = ops_text(&closed.ops);
        assert!(txt.contains('▸') && !txt.contains("body line"), "closed hides body");

        let mut open = HashSet::new();
        open.insert(0);
        let shown = frame(slide, &term(), Path::new("."), 0, 1, &Meta::default(), &open, 0);
        let txt = ops_text(&shown.ops);
        assert!(txt.contains('▾') && txt.contains("body line"), "open shows body");
        assert!(txt.contains("  body line"), "body aligns under the summary text");
    }

    #[test]
    fn scroll_windows_the_body() {
        let md: String = (1..=40).map(|i| format!("row{i:03}\n\n")).collect();
        let deck = parse(&md);
        let slide = &deck.slides[0];
        let t = term();
        let top = frame(slide, &t, Path::new("."), 0, 1, &Meta::default(), &HashSet::new(), 0);
        assert!(top.height > viewport(&t), "content overflows the viewport");
        assert!(ops_text(&top.ops).contains("row001"));
        let down = frame(slide, &t, Path::new("."), 0, 1, &Meta::default(), &HashSet::new(), 6);
        assert!(!ops_text(&down.ops).contains("row001"), "scrolled past the first rows");
    }

    #[test]
    fn title_slide_lists_sections_as_jump_targets() {
        let deck = build_deck("# Talk\n\n## Alpha\n\na\n\n## Beta\n\nb\n");
        assert_eq!(deck.slides[0].toc.len(), 2, "two section slides");
        assert_eq!(deck.slides[0].toc[1].index, 2, "Beta is slide 2");
        assert_eq!(deck.slides[0].toc[1].title, "Beta");
        let f = frame(&deck.slides[0], &term(), Path::new("."), 0, 3, &Meta::default(), &HashSet::new(), 0);
        assert!(
            f.hits.iter().any(|h| matches!(h.action, HitAction::Goto(2))),
            "a contents line jumps to its section",
        );
    }
}
