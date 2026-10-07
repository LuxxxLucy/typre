use crate::layout::HitAction;
use crossterm::event::KeyCode;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Command {
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

pub(super) fn command_for(code: KeyCode, ctrl: bool) -> Command {
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
pub(super) struct Nav {
    page: usize,
    len: usize,
    pending: Option<usize>,
}

impl Nav {
    pub(super) fn new(len: usize) -> Self {
        Nav {
            page: 0,
            len: len.max(1),
            pending: None,
        }
    }

    pub(super) fn page(&self) -> usize {
        self.page
    }

    pub(super) fn last(&self) -> usize {
        self.len - 1
    }

    pub(super) fn apply(&mut self, cmd: Command) -> bool {
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
                self.pending = Some(
                    self.pending
                        .unwrap_or(0)
                        .saturating_mul(10)
                        .saturating_add(d),
                );
                return false;
            }
            Command::ToggleHelp | Command::Cancel | Command::Quit | Command::Ignore => {
                return false
            }
        }
        self.pending = None;
        self.page != prev
    }

    pub(super) fn goto(&mut self, page: usize) -> bool {
        let prev = self.page;
        self.page = page.min(self.last());
        self.pending = None;
        self.page != prev
    }

    pub(super) fn set_len(&mut self, len: usize) {
        self.len = len.max(1);
        self.page = self.page.min(self.last());
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Update {
    None,
    Frame,
    Layout,
    Quit,
    OpenUrl(String),
}

pub(super) struct State {
    pub nav: Nav,
    pub help: bool,
    pub scroll: usize,
    open: Vec<HashSet<usize>>,
}

impl State {
    pub fn new(len: usize) -> Self {
        Self {
            nav: Nav::new(len),
            help: false,
            scroll: 0,
            open: vec![HashSet::new(); len.max(1)],
        }
    }

    pub fn open(&self) -> &HashSet<usize> {
        &self.open[self.nav.page()]
    }

    pub fn reload(&mut self, len: usize) {
        self.nav.set_len(len);
        self.open = vec![HashSet::new(); len.max(1)];
        self.scroll = 0;
    }

    pub fn command(&mut self, cmd: Command) -> Update {
        match cmd {
            Command::Quit => Update::Quit,
            Command::Cancel if !self.help => Update::Quit,
            Command::Cancel | Command::ToggleHelp => {
                self.help = !self.help;
                Update::Frame
            }
            cmd => {
                let closing = std::mem::take(&mut self.help);
                if self.nav.apply(cmd) {
                    self.scroll = 0;
                    Update::Layout
                } else if closing {
                    Update::Frame
                } else {
                    Update::None
                }
            }
        }
    }

    pub fn click(&mut self, action: &HitAction) -> Update {
        if self.help {
            return Update::None;
        }
        match action {
            HitAction::ToggleDetails(id) => {
                let set = &mut self.open[self.nav.page()];
                if !set.remove(id) {
                    set.insert(*id);
                }
                Update::Layout
            }
            HitAction::Goto(page) if self.nav.goto(*page) => {
                self.scroll = 0;
                Update::Layout
            }
            HitAction::OpenUrl(url) => Update::OpenUrl(url.clone()),
            _ => Update::None,
        }
    }

    pub fn scroll_by(&mut self, down: bool, max: usize) -> Update {
        if self.help {
            return Update::None;
        }
        let old = self.scroll;
        self.scroll = if down {
            self.scroll.saturating_add(1).min(max)
        } else {
            self.scroll.saturating_sub(1)
        };
        if old == self.scroll {
            Update::None
        } else {
            Update::Frame
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrolling_and_help_reuse_layout() {
        let mut state = State::new(3);
        assert_eq!(state.scroll_by(true, 10), Update::Frame);
        assert_eq!(state.command(Command::ToggleHelp), Update::Frame);
        assert_eq!(state.click(&HitAction::Goto(2)), Update::None);
        assert_eq!(state.nav.page(), 0);
        assert_eq!(state.command(Command::Next), Update::Layout);
        assert_eq!(state.scroll, 0);
    }

    #[test]
    fn reload_clears_structural_details_ids() {
        let mut state = State::new(3);
        state.click(&HitAction::ToggleDetails(0));
        state.command(Command::Last);
        state.reload(1);
        assert_eq!(state.nav.page(), 0);
        assert!(state.open().is_empty());
    }

    #[test]
    fn long_prefix_saturates() {
        let mut nav = Nav::new(3);
        for _ in 0..100 {
            nav.apply(Command::Digit(9));
        }
        nav.apply(Command::Last);
        assert_eq!(nav.page(), 2);
    }
}
