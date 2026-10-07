use super::{cache, Assets};
use crate::core::ir::{ArtPart, Block, Inline, Slide};

enum Asset<'a> {
    Fragment(&'a str, bool),
    Image(&'a str),
}

pub(super) fn prepare(slide: &Slide, ppi: u32, assets: &Assets) -> Vec<String> {
    let mut errors = Vec::new();
    visit_slide(slide, &mut |asset| match asset {
        Asset::Fragment(src, display) => {
            if let Err(error) = assets.fragment(src, ppi, display) {
                errors.push(error.to_string());
            }
        }
        Asset::Image(src) => {
            if !cache::is_remote(src) {
                assets.image(src);
            }
        }
    });
    errors
}

fn visit_slide(slide: &Slide, visit: &mut impl FnMut(Asset<'_>)) {
    for block in &slide.blocks {
        visit_block(block, visit);
    }
}

fn visit_block(block: &Block, visit: &mut impl FnMut(Asset<'_>)) {
    match block {
        Block::BlockTypst { src, .. } => visit(Asset::Fragment(src, true)),
        Block::Heading(_, inlines) | Block::Paragraph(inlines) => {
            visit_inlines(inlines, false, visit)
        }
        Block::List { items, .. } => {
            for block in items.iter().flatten() {
                visit_block(block, visit);
            }
        }
        Block::Quote(blocks) => {
            for block in blocks {
                visit_block(block, visit);
            }
        }
        Block::Table { head, rows, .. } => {
            for inlines in head.iter().chain(rows.iter().flatten()) {
                visit_inlines(inlines, true, visit);
            }
        }
        Block::Art { parts, caption } => {
            for part in parts {
                match part {
                    ArtPart::Lines(lines) => {
                        for line in lines {
                            visit_inlines(&line.inls, false, visit);
                        }
                    }
                    ArtPart::Nested { block, .. } => visit_block(block, visit),
                }
            }
            visit_inlines(caption, false, visit);
        }
        Block::Details { summary, body } => {
            visit_inlines(summary, false, visit);
            for line in body {
                visit_inlines(line, false, visit);
            }
        }
        Block::Image { src, .. } => visit(Asset::Image(src)),
        Block::Code { .. } | Block::Rule | Block::Tree(_) | Block::Grid(_) => {}
    }
}

fn visit_inlines(inlines: &[Inline], display: bool, visit: &mut impl FnMut(Asset<'_>)) {
    for inline in inlines {
        match inline {
            Inline::InlineTypst { src, .. } => visit(Asset::Fragment(src, display)),
            Inline::BlockFragment(block) => visit_block(block, visit),
            Inline::Text(..)
            | Inline::Code(_)
            | Inline::Link { .. }
            | Inline::SoftBreak
            | Inline::HardBreak => {}
        }
    }
}
