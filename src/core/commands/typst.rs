use crate::core::ir::Width;

use super::{parse_braced_command, ParsedCommand};

pub(crate) fn parse(command_text: &str) -> Option<(ParsedCommand, usize)> {
    let (body, used) = parse_braced_command(command_text, "typst")?;
    Some((
        ParsedCommand::Inline {
            src: body,
            width: Width::Natural,
        },
        used,
    ))
}
