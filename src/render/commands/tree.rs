use crate::core::ir::{Style, TreeNode};
use crate::layout::RenderOp;
use crate::render::paint::heading_style;

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
