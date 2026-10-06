use crate::core::ir::Width;

use super::{parse_command_with_argument, ParsedCommand};

pub(crate) fn parse(command_text: &str) -> Option<(ParsedCommand, usize)> {
    let (arg, body, used) = parse_command_with_argument(command_text, "width")?;
    Some((
        ParsedCommand::Inline {
            src: body,
            width: parse_width(&arg),
        },
        used,
    ))
}

fn parse_width(spec: &str) -> Width {
    let s = spec.trim();
    if s.eq_ignore_ascii_case("full") {
        return Width::Percent(100);
    }
    match s.strip_suffix('%') {
        Some(p) => Width::Percent(p.trim().parse().unwrap_or(100)),
        None => s.parse().map(Width::Cols).unwrap_or(Width::Natural),
    }
}
