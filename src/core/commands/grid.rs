use crate::core::ir::Block;

use super::{parse_braced_command, ParsedCommand};

pub(crate) fn parse(command_text: &str) -> Option<(ParsedCommand, usize)> {
    let (body, used) = parse_braced_command(command_text, "grid")?;
    Some((
        ParsedCommand::Block(Block::Grid(parse_entries(&body))),
        used,
    ))
}

fn parse_entries(src: &str) -> Vec<String> {
    src.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
}
