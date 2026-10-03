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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ir::{ArtLine, Width};

    fn math(src: &str) -> Inline {
        Inline::InlineTypst {
            src: src.into(),
            width: Width::Natural,
            display: false,
        }
    }

    #[test]
    fn visits_figures_details_and_table_render_modes() {
        let slide = Slide {
            blocks: vec![
                Block::Art {
                    parts: vec![
                        ArtPart::Lines(vec![ArtLine {
                            guide: String::new(),
                            inls: vec![math("line")],
                        }]),
                        ArtPart::Nested {
                            guide: String::new(),
                            block: Box::new(Block::BlockTypst {
                                src: "nested".into(),
                                width: Width::Natural,
                            }),
                        },
                    ],
                    caption: vec![math("caption")],
                },
                Block::Details {
                    summary: vec![math("summary")],
                    body: vec![vec![math("body")]],
                },
                Block::Table {
                    aligns: vec![],
                    head: vec![vec![math("head")]],
                    rows: vec![vec![vec![math("row")]]],
                },
            ],
            toc: vec![],
        };
        let mut found = Vec::new();
        visit_slide(&slide, &mut |asset| {
            if let Asset::Fragment(src, display) = asset {
                found.push((src.to_owned(), display));
            }
        });
        assert_eq!(
            found,
            vec![
                ("line".into(), false),
                ("nested".into(), true),
                ("caption".into(), false),
                ("summary".into(), false),
                ("body".into(), false),
                ("head".into(), true),
                ("row".into(), true),
            ]
        );
    }
}

#[cfg(test)]
mod image_tests {
    use super::*;

    #[test]
    fn prepares_local_images_before_rendering() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("image.png");
        image::RgbImage::new(2, 3).save(&source).unwrap();
        let slide = Slide {
            blocks: vec![Block::Quote(vec![Block::Image {
                src: "image.png".into(),
                alt: String::new(),
            }])],
            toc: vec![],
        };
        let assets = Assets::new(dir.path());
        assert!(assets.prepare(&slide, 96).is_empty());
        assert_eq!(assets.dependencies(), vec![source.canonicalize().unwrap()]);
        std::fs::remove_file(source).unwrap();
        let snapshot = assets.image("image.png").unwrap();
        assert_eq!(image::image_dimensions(snapshot).unwrap(), (2, 3));
    }
}
