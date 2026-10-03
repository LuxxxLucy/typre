use super::state::{Command, Nav};
use super::*;
use crate::core::ir::{Meta, Slide};
use crate::core::parse::parse as build_deck;
use crate::layout::HitAction;
use crate::layout::RenderOp;
use std::collections::HashSet;

#[allow(clippy::too_many_arguments)]
fn frame(
    slide: &Slide,
    term: &TermInfo,
    path: &Path,
    index: usize,
    total: usize,
    meta: &Meta,
    open: &HashSet<usize>,
    scroll: usize,
) -> Frame {
    let assets = Assets::new(path);
    let body = render::body(slide, term, &assets, open);
    compose(
        &body,
        term,
        meta,
        &View {
            index,
            total,
            scroll,
            help: false,
        },
    )
}
use crate::core::parse::parse;
use crate::render::blocks::{line_width, split_lines};

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
fn a_table_with_an_empty_header_draws_its_body_alone() {
    let deck = parse("| | |\n|:--|:--|\n| a | 1 |\n");
    let f = frame(
        &deck.slides[0],
        &term(),
        Path::new("."),
        0,
        1,
        &Meta::default(),
        &HashSet::new(),
        0,
    );
    let txt: Vec<String> = split_lines(f.ops)
        .iter()
        .map(|l| ops_text(l))
        .filter(|l| l.contains('│') || l.contains('┌') || l.contains('└'))
        .collect();
    assert!(
        !txt.iter().any(|l| l.contains('├')),
        "no header rule: {txt:?}"
    );
    assert!(txt[0].contains('┌'), "the top rule opens the body: {txt:?}");
    assert_eq!(txt.len(), 3, "a top rule, one row, a bottom rule: {txt:?}");
}

#[test]
fn a_link_inside_a_details_box_is_clickable() {
    let plain = parse("◊details[why]{\nsee [docs](https://x.test)\n}\n");
    let nested =
        parse("◊figure{\n├─ branch\n│  ◊details[why]{\n│  see [docs](https://x.test)\n│  }\n}\n");
    let mut open = HashSet::new();
    open.insert(0);
    for deck in [plain, nested] {
        let f = frame(
            &deck.slides[0],
            &term(),
            Path::new("."),
            0,
            1,
            &Meta::default(),
            &open,
            0,
        );
        assert!(
            f.hits
                .iter()
                .any(|h| matches!(&h.action, HitAction::OpenUrl(u) if u == "https://x.test")),
            "the link in the body is a click target"
        );
    }
}

#[test]
fn a_closed_details_box_glows_and_an_open_one_does_not() {
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
    let runs = glow_runs(&closed.ops);
    assert!(!runs.is_empty(), "a closed box glows");
    assert!(
        runs.iter()
            .all(|r| r.text.contains('│') || r.text.contains('┌') || r.text.contains('└')),
        "only the frame glows, not the text inside"
    );

    let mut open = HashSet::new();
    open.insert(0);
    let shown = frame(
        slide,
        &term(),
        Path::new("."),
        0,
        1,
        &Meta::default(),
        &open,
        0,
    );
    assert!(
        glow_runs(&shown.ops).is_empty(),
        "an open box is a plain frame"
    );
}

#[test]
fn a_link_inside_a_figure_is_clickable() {
    let deck = parse("◊figure{\n│  see [docs](https://x.test)\n}\n");
    let f = frame(
        &deck.slides[0],
        &term(),
        Path::new("."),
        0,
        1,
        &Meta::default(),
        &HashSet::new(),
        0,
    );
    assert!(
        f.hits
            .iter()
            .any(|h| matches!(&h.action, HitAction::OpenUrl(u) if u == "https://x.test")),
        "the link is a click target"
    );
}

#[test]
fn details_box_frame_stays_square_over_long_lines() {
    let long = "a word ".repeat(10);
    let deck = parse(&format!("◊details[{long}]{{\n{long}\n}}\n"));
    let mut open = HashSet::new();
    open.insert(0);
    let f = frame(
        &deck.slides[0],
        &term(),
        Path::new("."),
        0,
        1,
        &Meta::default(),
        &open,
        0,
    );
    let rows = split_lines(f.ops);
    let find = |c: char| rows.iter().position(|l| ops_text(l).contains(c));
    let (top, bottom) = (find('┌').unwrap(), find('└').unwrap());
    let widths: Vec<usize> = rows[top..=bottom].iter().map(|l| line_width(l)).collect();
    assert!(
        bottom > top + 1,
        "the body wrapped to several rows: {widths:?}"
    );
    assert!(
        widths[0] <= term().cols as usize,
        "the box fits the terminal"
    );
    for w in &widths {
        assert_eq!(*w, widths[0], "every row is the same width: {widths:?}");
    }
}

#[test]
fn details_inside_art_toggles_under_its_branch() {
    let deck = parse("◊figure{\n├─ branch\n│  ◊details[why]{\n│  because\n│  }\n}\n");
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
    assert_eq!(toggles, 1, "a nested box is a click target");
    let txt = ops_text(&closed.ops);
    assert!(!txt.contains("because"), "closed hides body");

    let mut open = HashSet::new();
    open.insert(0);
    let shown = frame(
        slide,
        &term(),
        Path::new("."),
        0,
        1,
        &Meta::default(),
        &open,
        0,
    );
    let txt = ops_text(&shown.ops);
    assert!(
        txt.contains("│  │ - why"),
        "the box keeps the branch guide: {txt:?}"
    );
    assert!(txt.contains("│  │   because"), "so does its body: {txt:?}");
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
    assert!(
        txt.contains("+ Summary") && !txt.contains("body line"),
        "closed hides body"
    );

    let mut open = HashSet::new();
    open.insert(0);
    let shown = frame(
        slide,
        &term(),
        Path::new("."),
        0,
        1,
        &Meta::default(),
        &open,
        0,
    );
    let txt = ops_text(&shown.ops);
    assert!(
        txt.contains("- Summary") && txt.contains("body line"),
        "open shows body"
    );
    assert!(
        txt.contains("  body line"),
        "body aligns under the summary text"
    );
}

#[test]
fn scroll_windows_the_body() {
    let md: String = (1..=40).map(|i| format!("row{i:03}\n\n")).collect();
    let deck = parse(&md);
    let slide = &deck.slides[0];
    let t = term();
    let top = frame(
        slide,
        &t,
        Path::new("."),
        0,
        1,
        &Meta::default(),
        &HashSet::new(),
        0,
    );
    assert!(top.height > viewport(&t), "content overflows the viewport");
    assert!(ops_text(&top.ops).contains("row001"));
    let down = frame(
        slide,
        &t,
        Path::new("."),
        0,
        1,
        &Meta::default(),
        &HashSet::new(),
        6,
    );
    assert!(
        !ops_text(&down.ops).contains("row001"),
        "scrolled past the first rows"
    );
}

#[test]
fn title_slide_lists_sections_as_jump_targets() {
    let deck = build_deck("# Talk\n\n## Alpha\n\na\n\n## Beta\n\nb\n");
    assert_eq!(deck.slides[0].toc.len(), 2, "two section slides");
    assert_eq!(deck.slides[0].toc[1].index, 2, "Beta is slide 2");
    assert_eq!(deck.slides[0].toc[1].title, "Beta");
    let f = frame(
        &deck.slides[0],
        &term(),
        Path::new("."),
        0,
        3,
        &Meta::default(),
        &HashSet::new(),
        0,
    );
    assert!(
        f.hits
            .iter()
            .any(|h| matches!(h.action, HitAction::Goto(2))),
        "a contents line jumps to its section",
    );
}

#[test]
fn reload_keeps_current_document_during_file_replacement() {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("slides.md");
    let mut deck = parse("# Kept");
    let mut state = State::new(deck.slides.len());
    assert!(!reload_document(&path, &mut deck, &mut state));
    assert!(format!("{deck:?}").contains("Kept"));
    std::fs::write(&path, "# Replaced").unwrap();
    assert!(reload_document(&path, &mut deck, &mut state));
    assert!(format!("{deck:?}").contains("Replaced"));
}
