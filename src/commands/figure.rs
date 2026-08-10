use crate::core::ir::Block;
use crate::core::parse::art_parts;

use super::{bracket_cmd, Frag};

// `◊figure[caption]{body}`: a language-less fence with a caption under it.
pub(crate) fn parse(after: &str) -> Option<(Frag, usize)> {
    let (caption, body, used) = bracket_cmd(after, "figure")?;
    let block = Block::Art {
        parts: art_parts(body.trim_matches('\n')),
        caption,
    };
    Some((Frag::Block(block), used))
}
