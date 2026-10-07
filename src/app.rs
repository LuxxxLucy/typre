mod reload;
mod state;
#[cfg(test)]
mod tests;

use std::fs;
use std::io::{stdout, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind};
use crossterm::{cursor, queue, terminal};

use crate::assets::Assets;
use crate::core::ir::Deck;
use crate::core::parse::parse;
use crate::layout::ops::glow_runs;
use crate::layout::Body;
use crate::layout::{natural_ppi, viewport, TermInfo};
use crate::render;
use crate::render::frame::{compose, error_prompt, loading_frame, Frame, View};
use crate::term::{Emitter, TerminalSession};
use reload::{Change, Reload};
use state::{command_for, State, Update};

#[derive(Parser)]
#[command(about = "Terminal typst slideshow")]
struct Cli {
    deck: PathBuf,
    #[arg(
        long,
        conflicts_with = "export",
        help = "Print parsed slides and drawing operations"
    )]
    dump_ops: bool,
    #[arg(long, help = "Write the terminal byte stream")]
    export: bool,
    #[arg(short, long, requires = "export", help = "Set the export destination")]
    output: Option<PathBuf>,
}

pub fn run_cli() -> Result<()> {
    let cli = Cli::parse();
    let deck = load(&cli.deck)?;
    let assets = Assets::new(&deck_dir_of(&cli.deck));
    if cli.dump_ops {
        println!("{deck:#?}");
        for (i, slide) in deck.slides.iter().enumerate() {
            println!("\n=== slide {i} render ops ===");
            println!("{:#?}", render::render(slide, &export_term(), &assets));
        }
        return Ok(());
    }
    if cli.export {
        return export(&deck, &assets, cli.output.as_deref());
    }
    let _session = TerminalSession::new()?;
    present(&cli.deck, deck, assets, &mut stdout())
}

fn export_term() -> TermInfo {
    TermInfo {
        cols: 80,
        rows: 24,
        cell_w_px: 9,
        cell_h_px: 18,
    }
}

fn export(deck: &Deck, assets: &Assets, output: Option<&Path>) -> Result<()> {
    let mut out: Box<dyn Write> = match output {
        Some(path) => {
            Box::new(fs::File::create(path).with_context(|| format!("write {}", path.display()))?)
        }
        None => Box::new(stdout()),
    };
    let term = export_term();
    let mut emitter = Emitter::default();
    let open = Default::default();
    for (index, slide) in deck.slides.iter().enumerate() {
        let body = render::body(slide, &term, assets, &open);
        let frame = compose(
            &body,
            &term,
            &deck.meta,
            &View {
                index,
                total: deck.slides.len(),
                ..View::default()
            },
        );
        emitter.emit(&frame.ops, &mut out)?;
    }
    Ok(())
}

fn deck_dir_of(path: &Path) -> PathBuf {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .to_path_buf()
}

fn load(path: &Path) -> Result<Deck> {
    let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    Ok(parse(&text))
}

fn prepare_and_confirm(
    emitter: &mut Emitter,
    deck: &Deck,
    assets: &Assets,
    term: &TermInfo,
    out: &mut impl Write,
) -> Result<bool> {
    let mut errors = Vec::new();
    for (index, slide) in deck.slides.iter().enumerate() {
        queue!(
            out,
            terminal::Clear(terminal::ClearType::All),
            cursor::MoveTo(0, 0)
        )?;
        emitter.emit(
            &loading_frame(
                term,
                &format!("Compiling slide {} / {}", index + 1, deck.slides.len()),
            ),
            out,
        )?;
        errors.extend(
            assets
                .prepare(slide, natural_ppi(term))
                .into_iter()
                .map(|error| (index + 1, error)),
        );
    }
    if errors.is_empty() {
        return Ok(true);
    }
    queue!(
        out,
        terminal::Clear(terminal::ClearType::All),
        cursor::MoveTo(0, 0)
    )?;
    emitter.emit(&error_prompt(term, &errors), out)?;
    loop {
        if let Event::Key(KeyEvent {
            code,
            kind: event::KeyEventKind::Press,
            modifiers,
            ..
        }) = event::read()?
        {
            match code {
                KeyCode::Char('y' | 'Y') | KeyCode::Enter => return Ok(true),
                KeyCode::Char('n' | 'N' | 'q') | KeyCode::Esc => return Ok(false),
                KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                    return Ok(false)
                }
                _ => {}
            }
        }
    }
}

fn build_body(deck: &Deck, state: &State, term: &TermInfo, assets: &Assets) -> Body {
    deck.slides
        .get(state.nav.page())
        .map(|slide| render::body(slide, term, assets, state.open()))
        .unwrap_or_default()
}

fn build_frame(deck: &Deck, state: &mut State, term: &TermInfo, body: &Body) -> Frame {
    let frame = compose(
        body,
        term,
        &deck.meta,
        &View {
            index: state.nav.page(),
            total: deck.slides.len(),
            scroll: state.scroll,
            help: state.help,
        },
    );
    state.scroll = frame.scroll;
    frame
}

fn draw(emitter: &mut Emitter, frame: &Frame, state: &State, out: &mut impl Write) -> Result<()> {
    queue!(
        out,
        terminal::Clear(terminal::ClearType::All),
        cursor::MoveTo(0, 0)
    )?;
    emitter.emit(&frame.ops, out)?;
    let pct = ((state.nav.page() + 1) * 100 / (state.nav.last() + 1)).min(100);
    write!(out, "\x1b]9;4;1;{pct}\x1b\\")?;
    out.flush()?;
    Ok(())
}

fn input(event: Event, state: &mut State, term: &mut TermInfo, frame: &Frame) -> Update {
    match event {
        Event::Resize(..) => {
            *term = TermInfo::query();
            Update::Layout
        }
        Event::Key(KeyEvent {
            code,
            modifiers,
            kind: event::KeyEventKind::Press,
            ..
        }) => state.command(command_for(code, modifiers.contains(KeyModifiers::CONTROL))),
        Event::Mouse(mouse) => match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => frame
                .hits
                .iter()
                .find(|hit| hit.row == mouse.row && hit.cols.contains(&mouse.column))
                .map(|hit| state.click(&hit.action))
                .unwrap_or(Update::None),
            MouseEventKind::ScrollDown | MouseEventKind::ScrollUp => state.scroll_by(
                mouse.kind == MouseEventKind::ScrollDown,
                frame.height.saturating_sub(viewport(term)),
            ),
            _ => Update::None,
        },
        _ => Update::None,
    }
}

fn present(path: &Path, mut deck: Deck, assets: Assets, out: &mut impl Write) -> Result<()> {
    let mut state = State::new(deck.slides.len());
    let mut term = TermInfo::acquire();
    let mut reload = Reload::new(path)?;
    let mut emitter = Emitter::default();
    if !prepare_and_confirm(&mut emitter, &deck, &assets, &term, out)? {
        return Ok(());
    }
    let (tx, downloads) = mpsc::channel();
    assets.set_redraw(tx);
    let mut body = build_body(&deck, &state, &term, &assets);
    let mut frame = build_frame(&deck, &mut state, &term, &body);
    let mut glow = glow_runs(&frame.ops);
    reload.track(assets.dependencies())?;
    draw(&mut emitter, &frame, &state, out)?;
    let mut repeat_frame_pending = true;
    loop {
        let mut update = Update::None;
        if event::poll(Duration::from_millis(if repeat_frame_pending {
            0
        } else {
            100
        }))? {
            update = input(event::read()?, &mut state, &mut term, &frame);
        }
        match update {
            Update::Quit => break,
            Update::OpenUrl(ref url) => open_url(url),
            _ => {}
        }
        if let Some(change) = reload.poll()? {
            let ready = change == Change::Assets || reload_document(path, &mut deck, &mut state);
            if ready {
                assets.invalidate();
                emitter.clear_images();
                for slide in &deck.slides {
                    assets.prepare(slide, natural_ppi(&term));
                }
                update = Update::Layout;
            }
        }
        if downloads.try_iter().count() > 0 {
            update = Update::Layout;
        }
        if update == Update::Layout {
            body = build_body(&deck, &state, &term, &assets);
            reload.track(assets.dependencies())?;
        }
        let dirty = matches!(update, Update::Frame | Update::Layout);
        if dirty {
            frame = build_frame(&deck, &mut state, &term, &body);
            glow = if state.help {
                Vec::new()
            } else {
                glow_runs(&frame.ops)
            };
        }
        if dirty || repeat_frame_pending {
            draw(&mut emitter, &frame, &state, out)?;
            repeat_frame_pending = dirty;
        } else if !glow.is_empty() {
            emitter.paint_glow(out, &glow)?;
        }
    }
    Ok(())
}

fn open_url(url: &str) {
    let cmd = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let _ = std::process::Command::new(cmd).arg(url).spawn();
}

fn reload_document(path: &Path, deck: &mut Deck, state: &mut State) -> bool {
    let Ok(next) = load(path) else {
        return false;
    };
    *deck = next;
    state.reload(deck.slides.len());
    true
}
