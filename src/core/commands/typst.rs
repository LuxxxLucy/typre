use crate::core::ir::Width;

use super::{brace_cmd, Frag};

pub(crate) fn parse(after: &str) -> Option<(Frag, usize)> {
    let (body, used) = brace_cmd(after, "typst")?;
    Some((
        Frag::Inline {
            src: body,
            width: Width::Natural,
        },
        used,
    ))
}
