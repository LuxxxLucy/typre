use crate::core::ir::Block;
use crate::core::parse::{art_inlines, art_parts};

use super::{parse_braced_command, parse_command_with_argument, ParsedCommand};

pub(crate) fn parse(command_text: &str) -> Option<(ParsedCommand, usize)> {
    let (caption, body, used) = match parse_command_with_argument(command_text, "figure") {
        Some(parts) => parts,
        None => {
            let (body, used) = parse_braced_command(command_text, "figure")?;
            (String::new(), body, used)
        }
    };
    let block = Block::Art {
        parts: art_parts(body.trim_matches('\n')),
        caption: art_inlines(&caption),
    };
    Some((ParsedCommand::Block(block), used))
}
