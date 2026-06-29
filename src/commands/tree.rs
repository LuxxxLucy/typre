use crate::core::ir::{Block, RenderOp, Style, TreeNode};
use crate::render::paint::heading_style;

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

pub(crate) fn render(nodes: &[TreeNode], indent: usize, ops: &mut Vec<RenderOp>) {
    fn walk(nodes: &[TreeNode], prefix: &str, indent: usize, ops: &mut Vec<RenderOp>) {
        for (i, node) in nodes.iter().enumerate() {
            let last = i + 1 == nodes.len();
            let branch = if last { "└── " } else { "├── " };
            ops.push(RenderOp::Text(
                format!("{}{prefix}{branch}{}", " ".repeat(indent), node.label),
                Style::default(),
            ));
            ops.push(RenderOp::LineBreak);
            let child_prefix = format!("{prefix}{}", if last { "    " } else { "│   " });
            walk(&node.children, &child_prefix, indent, ops);
        }
    }
    // top-level nodes are plain bold labels; their descendants carry connectors
    for node in nodes {
        ops.push(RenderOp::Text(
            format!("{}{}", " ".repeat(indent), node.label),
            heading_style(),
        ));
        ops.push(RenderOp::LineBreak);
        walk(&node.children, "", indent, ops);
    }
}
