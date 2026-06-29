use crate::core::ir::{Align, Block, RenderOp, Style};
use crate::layout::TermInfo;
use crate::render::paint::{hrule, pad};

use super::{brace_cmd, Frag};

pub(crate) fn parse(after: &str) -> Option<(Frag, usize)> {
    let (body, used) = brace_cmd(after, "grid")?;
    Some((Frag::Block(Block::Grid(cells(&body))), used))
}

fn cells(src: &str) -> Vec<String> {
    src.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
}

pub(crate) fn render(cells: &[String], term: &TermInfo, indent: usize, ops: &mut Vec<RenderOp>) {
    let n = cells.len();
    if n == 0 {
        return;
    }
    let avail = (term.cols as usize).saturating_sub(indent);
    let gaps = n.saturating_sub(1);
    let inner = (avail.saturating_sub(gaps + n * 2) / n).max(1);
    let pre = " ".repeat(indent);
    let mut top = String::new();
    let mut mid = String::new();
    let mut bot = String::new();
    for (i, cell) in cells.iter().enumerate() {
        if i > 0 {
            top.push(' ');
            mid.push(' ');
            bot.push(' ');
        }
        top.push_str(&hrule('┌', inner + 2, '┐'));
        mid.push_str(&format!("│ {} │", pad(cell, inner, Align::Center)));
        bot.push_str(&hrule('└', inner + 2, '┘'));
    }
    for line in [top, mid, bot] {
        ops.push(RenderOp::Text(format!("{pre}{line}"), Style::default()));
        ops.push(RenderOp::LineBreak);
    }
}
