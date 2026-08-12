use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};

use crate::core::ir::{RenderOp, Style, Width};
use crate::layout::{natural_ppi, TermInfo};
use crate::render::paint::place_image;

use super::{brace_cmd, Frag};

// Native-typst aliases for the helper names mitex emits.
const MATH_SHIM: &str = r#"#let mitexsqrt(..a) = { let p = a.pos(); if p.len() == 1 { math.sqrt(p.at(0)) } else { math.root(p.at(1), p.at(0)) } }
#let zws = math.zws
#let aligned(..a) = a.pos().join()
#let matrix(..a) = math.mat(delim: none, ..a)
#let pmatrix(..a) = math.mat(delim: "(", ..a)
#let bmatrix(..a) = math.mat(delim: "[", ..a)
#let vmatrix(..a) = math.mat(delim: "|", ..a)
#let Vmatrix(..a) = math.mat(delim: "‖", ..a)
"#;

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
    // The cache lives in the temporary directory, not beside the deck: a PNG is named by the
    // hash of what produced it, so nothing there is worth keeping and nothing is worth
    // writing into the author's folder.
    let cache_dir = std::env::temp_dir().join("typre-cache");
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
        "#set page(width: auto, height: auto, margin: 0pt, fill: none)\n#set text(fill: white)\n{MATH_SHIM}{body}"
    );
    // The source goes in on stdin, so no scratch file lands next to the deck. A relative
    // import in it resolves against `--root`, which is the deck's own directory.
    let mut child = Command::new("typst")
        .arg("compile")
        .arg("--root")
        .arg(deck_dir)
        .arg("--ppi")
        .arg(ppi.to_string())
        .arg("-")
        .arg(&png)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("spawn typst")?;
    child
        .stdin
        .take()
        .context("typst stdin")?
        .write_all(wrapped.as_bytes())
        .context("write typst source")?;
    let result = child.wait_with_output();

    let out = result.context("run typst")?;
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
