use std::path::Path;

use crate::core::ir::{Block, Inline, Slide};
use crate::layout::{natural_ppi, TermInfo};
use crate::commands;

// Compile every typst fragment in a slide (warming the cache); return any errors.
pub fn typst_precompile_errors(slide: &Slide, term: &TermInfo, deck_dir: &Path) -> Vec<String> {
    let mut errs = Vec::new();
    for block in &slide.blocks {
        collect_block_typst(block, term, deck_dir, &mut errs);
    }
    errs
}

fn collect_block_typst(b: &Block, term: &TermInfo, deck_dir: &Path, errs: &mut Vec<String>) {
    match b {
        Block::BlockTypst { src, .. } => {
            if let Err(e) = commands::typst::render_fragment(src, deck_dir, natural_ppi(term), true) {
                errs.push(e.to_string());
            }
        }
        Block::Heading(_, inls) | Block::Paragraph(inls) => {
            collect_inline_typst(inls, term, deck_dir, errs)
        }
        Block::List { items, .. } => items
            .iter()
            .flatten()
            .for_each(|bb| collect_block_typst(bb, term, deck_dir, errs)),
        Block::Quote(inner) => inner
            .iter()
            .for_each(|bb| collect_block_typst(bb, term, deck_dir, errs)),
        Block::Table { head, rows, .. } => {
            for cell in head.iter().chain(rows.iter().flatten()) {
                collect_inline_typst(cell, term, deck_dir, errs);
            }
        }
        _ => {}
    }
}

fn collect_inline_typst(inls: &[Inline], term: &TermInfo, deck_dir: &Path, errs: &mut Vec<String>) {
    for i in inls {
        if let Inline::InlineTypst { src, .. } = i {
            if let Err(e) = commands::typst::render_fragment(src, deck_dir, natural_ppi(term), false) {
                errs.push(e.to_string());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::parse::parse;

    fn term() -> TermInfo {
        TermInfo {
            cols: 40,
            rows: 20,
            cell_w_px: 8,
            cell_h_px: 16,
        }
    }

    #[test]
    fn typst_errors_are_collected() {
        if std::process::Command::new("typst")
            .arg("--version")
            .output()
            .is_err()
        {
            return; // no typst on PATH; nothing to compile
        }
        let deck = parse("◊typst{#this_symbol_does_not_exist}\n");
        let errs = typst_precompile_errors(&deck.slides[0], &term(), &std::env::temp_dir());
        assert!(!errs.is_empty(), "a bad fragment must surface an error");
    }
}
