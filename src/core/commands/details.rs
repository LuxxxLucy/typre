use crate::core::ir::Block;
use crate::core::parse::art_inlines;

use super::{parse_command_with_argument, ParsedCommand};

pub(crate) fn parse(command_text: &str) -> Option<(ParsedCommand, usize)> {
    let (summary, body, used) = parse_command_with_argument(command_text, "details")?;
    Some((ParsedCommand::Block(parse_details(&summary, &body)), used))
}

fn parse_details(summary: &str, body: &str) -> Block {
    Block::Details {
        summary: art_inlines(summary),
        body: body.trim_matches('\n').lines().map(art_inlines).collect(),
    }
}
