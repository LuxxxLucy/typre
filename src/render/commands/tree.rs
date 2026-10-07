use crate::core::ir::{Style, TreeNode};
use crate::layout::RenderOp;
use crate::render::paint::heading_style;

pub(crate) fn render(nodes: &[TreeNode], indent: usize, ops: &mut Vec<RenderOp>) {
    fn render_branches(nodes: &[TreeNode], prefix: &str, indent: usize, ops: &mut Vec<RenderOp>) {
        for (i, node) in nodes.iter().enumerate() {
            let is_last_child = i + 1 == nodes.len();
            let branch = if is_last_child {
                "└── "
            } else {
                "├── "
            };
            ops.push(RenderOp::Text(
                format!("{}{prefix}{branch}{}", " ".repeat(indent), node.label),
                Style::default(),
            ));
            ops.push(RenderOp::LineBreak);
            let child_prefix = format!("{prefix}{}", if is_last_child { "    " } else { "│   " });
            render_branches(&node.children, &child_prefix, indent, ops);
        }
    }
    for node in nodes {
        ops.push(RenderOp::Text(
            format!("{}{}", " ".repeat(indent), node.label),
            heading_style(),
        ));
        ops.push(RenderOp::LineBreak);
        render_branches(&node.children, "", indent, ops);
    }
}
