use crate::core::ir::Block;
use crate::core::parse::{art_inlines, art_parts};

use super::{brace_cmd, bracket_cmd, Frag};

// `◊figure{body}` keeps its lines as written, where markdown would reflow them, and hosts
// nested ◊ commands. `◊figure[caption]{body}` puts a caption under it.
pub(crate) fn parse(after: &str) -> Option<(Frag, usize)> {
    let (caption, body, used) = match bracket_cmd(after, "figure") {
        Some(parts) => parts,
        None => {
            let (body, used) = brace_cmd(after, "figure")?;
            (String::new(), body, used)
        }
    };
    let block = Block::Art {
        parts: art_parts(body.trim_matches('\n')),
        caption: art_inlines(&caption),
    };
    Some((Frag::Block(block), used))
}
