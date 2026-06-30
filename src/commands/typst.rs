use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

use crate::core::ir::{RenderOp, Style, Width};
use crate::layout::{natural_ppi, TermInfo};
use crate::render::paint::place_image;

use super::{brace_cmd, Frag};

pub(crate) fn parse(after: &str) -> Option<(Frag, usize)> {
    let (body, used) = brace_cmd(after, "typst")?;
    Some((Frag::Inline { src: body, width: Width::Natural }, used))
}

// Hash the local files a fragment imports, recursing through .typ imports.
fn imported_files_digest(src: &str, deck_dir: &Path) -> String {
    let mut hasher = blake3::Hasher::new();
    let mut seen = std::collections::BTreeSet::new();
    collect_refs(src, deck_dir, &mut seen, &mut hasher);
    hasher.finalize().to_hex().to_string()
}

fn collect_refs(
    src: &str,
    base: &Path,
    seen: &mut std::collections::BTreeSet<PathBuf>,
    hasher: &mut blake3::Hasher,
) {
    for quoted in src.split('"').skip(1).step_by(2) {
        let path = base.join(quoted);
        if !path.is_file() || !seen.insert(path.clone()) {
            continue;
        }
        let Ok(bytes) = fs::read(&path) else { continue };
        hasher.update(quoted.as_bytes());
        hasher.update(&bytes);
        if path.extension().is_some_and(|e| e == "typ") {
            if let Ok(text) = std::str::from_utf8(&bytes) {
                let parent = path.parent().unwrap_or(base).to_path_buf();
                collect_refs(text, &parent, seen, hasher);
            }
        }
    }
}

pub(crate) fn render_fragment(
    src: &str,
    deck_dir: &Path,
    ppi: u32,
    display: bool,
) -> Result<PathBuf> {
    let mode = if display { 'b' } else { 'i' };
    let deps = imported_files_digest(src, deck_dir);
    let hash = blake3::hash(format!("{mode}{ppi}{src}{deps}").as_bytes())
        .to_hex()
        .to_string();
    let cache_dir = deck_dir.join(".typre-cache");
    let png = cache_dir.join(format!("{hash}.png"));
    if png.exists() {
        return Ok(png);
    }
    fs::create_dir_all(&cache_dir).context("create cache dir")?;

    // Pad inline math vertically so the glyph fills ~85% of the one-cell line height.
    let body = if display {
        format!("$ {src} $")
    } else {
        format!("#context {{ let e = [${src}$]; box(inset: (y: measure(e).height * 0.088), e) }}")
    };
    let wrapped = format!(
        "#set page(width: auto, height: auto, margin: 0pt, fill: none)\n#set text(fill: white)\n{body}"
    );
    let tmp = deck_dir.join(format!(".typre-frag-{hash}.typ"));
    fs::write(&tmp, wrapped).context("write temp typst")?;

    let result = Command::new("typst")
        .arg("compile")
        .arg("--root")
        .arg(deck_dir)
        .arg("--ppi")
        .arg(ppi.to_string())
        .arg(&tmp)
        .arg(&png)
        .output();

    let _ = fs::remove_file(&tmp);

    let out = result.context("spawn typst")?;
    if !out.status.success() {
        bail!(
            "typst compile failed:\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(png)
}

pub(crate) fn render_block(
    src: &str,
    width: Width,
    term: &TermInfo,
    deck_dir: &Path,
    indent: usize,
    ops: &mut Vec<RenderOp>,
) {
    match render_fragment(src, deck_dir, natural_ppi(term), true) {
        Ok(png_path) => {
            place_image(ops, png_path, term, indent, width);
        }
        Err(e) => {
            ops.push(RenderOp::Text(
                format!("{}[typst error: {e}]", " ".repeat(indent)),
                Style::default(),
            ));
            ops.push(RenderOp::LineBreak);
        }
    }
}
