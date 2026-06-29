use crate::core::ir::Width;

use super::{bracket_cmd, Frag};

pub(crate) fn parse(after: &str) -> Option<(Frag, usize)> {
    let (arg, body, used) = bracket_cmd(after, "width")?;
    Some((Frag::Inline { src: body, width: value(&arg) }, used))
}

fn value(spec: &str) -> Width {
    let s = spec.trim();
    if s.eq_ignore_ascii_case("full") {
        return Width::Percent(100);
    }
    match s.strip_suffix('%') {
        Some(p) => Width::Percent(p.trim().parse().unwrap_or(100)),
        None => s.parse().map(Width::Cols).unwrap_or(Width::Natural),
    }
}
