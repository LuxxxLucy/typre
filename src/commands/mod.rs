// The ◊ commands. Each command owns its body parsing and rendering in its own
// module; this file parses the shared `◊name[arg]{body}` envelope and dispatches.
use crate::core::ir::{Block, Width};

pub mod details;
pub mod figure;
pub mod grid;
pub mod tree;
pub mod typst;
pub mod width;

// A ◊ command pulled out before markdown parsing: `typst`/`width` restore inline
// (or as block math when alone), the rest restore as their structured block.
pub(crate) enum Frag {
    Inline { src: String, width: Width },
    Block(Block),
}

// Parse one ◊ command at the start of `after` (just past the ◊). Each command
// matches its own name and returns its fragment and the bytes consumed through
// the closing `}`.
pub(crate) fn parse_command(after: &str) -> Option<(Frag, usize)> {
    typst::parse(after)
        .or_else(|| tree::parse(after))
        .or_else(|| grid::parse(after))
        .or_else(|| width::parse(after))
        .or_else(|| figure::parse(after))
        .or_else(|| details::parse(after))
}

// `◊name{body}`: return the body and the bytes consumed through the closing `}`.
pub(crate) fn brace_cmd(after: &str, name: &str) -> Option<(String, usize)> {
    let rest = after.strip_prefix(name)?.strip_prefix('{')?;
    let (body, used) = balanced(rest, '{', '}')?;
    Some((body, name.len() + 1 + used))
}

// `◊name[arg]{body}`: return the arg, the body, and the bytes consumed through `}`.
pub(crate) fn bracket_cmd(after: &str, name: &str) -> Option<(String, String, usize)> {
    let rest = after.strip_prefix(name)?.strip_prefix('[')?;
    // Balanced, so a markdown link in the argument keeps its own brackets.
    let (arg, arg_used) = balanced(rest, '[', ']')?;
    let rest = rest[arg_used..].strip_prefix('{')?;
    let (body, used) = balanced(rest, '{', '}')?;
    Some((arg, body, name.len() + arg_used + 2 + used))
}

// `s` begins just after an opening `open`; return the body up to the matching `close` and the
// byte count through it, counting nested pairs.
fn balanced(s: &str, open: char, close: char) -> Option<(String, usize)> {
    let mut depth = 1usize;
    for (i, c) in s.char_indices() {
        if c == open {
            depth += 1;
        } else if c == close {
            depth -= 1;
            if depth == 0 {
                return Some((s[..i].to_string(), i + c.len_utf8()));
            }
        }
    }
    None
}
