use crate::core::ir::{Block, TreeNode};

use super::{brace_cmd, Frag};

pub(crate) fn parse(after: &str) -> Option<(Frag, usize)> {
    let (body, used) = brace_cmd(after, "tree")?;
    Some((Frag::Block(Block::Tree(nodes(&body))), used))
}

fn nodes(src: &str) -> Vec<TreeNode> {
    let mut entries: Vec<(usize, String)> = Vec::new();
    for line in src.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        let mut label = line.trim_start().to_string();
        for p in ["- ", "* ", "+ "] {
            if let Some(rest) = label.strip_prefix(p) {
                label = rest.trim().to_string();
                break;
            }
        }
        entries.push((indent, label));
    }
    let mut pos = 0;
    build(&entries, &mut pos, 0)
}

fn build(entries: &[(usize, String)], pos: &mut usize, min_indent: usize) -> Vec<TreeNode> {
    let mut nodes = Vec::new();
    while *pos < entries.len() {
        let (indent, label) = &entries[*pos];
        if *indent < min_indent {
            break;
        }
        let cur_indent = *indent;
        let label = label.clone();
        *pos += 1;
        let children = build(entries, pos, cur_indent + 1);
        nodes.push(TreeNode { label, children });
    }
    nodes
}
