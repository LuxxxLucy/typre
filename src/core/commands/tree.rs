use crate::core::ir::{Block, TreeNode};

use super::{parse_braced_command, ParsedCommand};

pub(crate) fn parse(command_text: &str) -> Option<(ParsedCommand, usize)> {
    let (body, used) = parse_braced_command(command_text, "tree")?;
    Some((ParsedCommand::Block(Block::Tree(parse_nodes(&body))), used))
}

fn parse_nodes(src: &str) -> Vec<TreeNode> {
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
    let mut position = 0;
    build_subtree(&entries, &mut position, 0)
}

fn build_subtree(
    entries: &[(usize, String)],
    position: &mut usize,
    min_indent: usize,
) -> Vec<TreeNode> {
    let mut nodes = Vec::new();
    while *position < entries.len() {
        let (indent, label) = &entries[*position];
        if *indent < min_indent {
            break;
        }
        let current_indent = *indent;
        let label = label.clone();
        *position += 1;
        let children = build_subtree(entries, position, current_indent + 1);
        nodes.push(TreeNode { label, children });
    }
    nodes
}
