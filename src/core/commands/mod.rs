use crate::core::ir::{Block, Width};

pub mod details;
pub mod figure;
pub mod grid;
pub mod tree;
pub mod typst;
pub mod width;

pub(crate) enum ParsedCommand {
    Inline { src: String, width: Width },
    Block(Block),
}

pub(crate) fn parse_command(command_text: &str) -> Option<(ParsedCommand, usize)> {
    typst::parse(command_text)
        .or_else(|| tree::parse(command_text))
        .or_else(|| grid::parse(command_text))
        .or_else(|| width::parse(command_text))
        .or_else(|| figure::parse(command_text))
        .or_else(|| details::parse(command_text))
}

pub(crate) fn parse_braced_command(command_text: &str, name: &str) -> Option<(String, usize)> {
    let rest = command_text.strip_prefix(name)?.strip_prefix('{')?;
    let (body, used) = parse_balanced_body(rest, '{', '}')?;
    Some((body, name.len() + 1 + used))
}

pub(crate) fn parse_command_with_argument(
    command_text: &str,
    name: &str,
) -> Option<(String, String, usize)> {
    let rest = command_text.strip_prefix(name)?.strip_prefix('[')?;
    let (arg, arg_used) = parse_balanced_body(rest, '[', ']')?;
    let rest = rest[arg_used..].strip_prefix('{')?;
    let (body, used) = parse_balanced_body(rest, '{', '}')?;
    Some((arg, body, name.len() + arg_used + 2 + used))
}

fn parse_balanced_body(source: &str, open: char, close: char) -> Option<(String, usize)> {
    let mut depth = 1usize;
    for (offset, character) in source.char_indices() {
        if character == open {
            depth += 1;
        } else if character == close {
            depth -= 1;
            if depth == 0 {
                return Some((source[..offset].to_string(), offset + character.len_utf8()));
            }
        }
    }
    None
}
