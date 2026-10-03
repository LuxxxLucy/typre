use crate::core::ir::Block;
use crate::core::parse::art_inlines;

use super::{bracket_cmd, Frag};

pub(crate) fn parse(after: &str) -> Option<(Frag, usize)> {
    let (summary, body, used) = bracket_cmd(after, "details")?;
    Some((Frag::Block(build(&summary, &body)), used))
}

fn build(summary: &str, body: &str) -> Block {
    Block::Details {
        summary: art_inlines(summary),
        body: body.trim_matches('\n').lines().map(art_inlines).collect(),
    }
}
