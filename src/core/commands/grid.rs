use crate::core::ir::Block;

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
