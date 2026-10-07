use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};

use super::cache;

const INLINE_MATH_VERTICAL_PADDING_RATIO: f64 = 0.088;

const MITEX_MATH_ALIASES: &str = r#"#let mitexsqrt(..a) = { let p = a.pos(); if p.len() == 1 { math.sqrt(p.at(0)) } else { math.root(p.at(1), p.at(0)) } }
#let zws = math.zws
#let aligned(..a) = a.pos().join()
#let matrix(..a) = math.mat(delim: none, ..a)
#let pmatrix(..a) = math.mat(delim: "(", ..a)
#let bmatrix(..a) = math.mat(delim: "[", ..a)
#let vmatrix(..a) = math.mat(delim: "|", ..a)
#let Vmatrix(..a) = math.mat(delim: "‖", ..a)
"#;

fn dependency_digest(src: &str, deck_dir: &Path, dependencies: &mut BTreeSet<PathBuf>) -> String {
    let mut hasher = blake3::Hasher::new();
    let mut seen = BTreeSet::new();
    hash_referenced_files(src, deck_dir, &mut seen, dependencies, &mut hasher);
    hasher.finalize().to_hex().to_string()
}

fn hash_referenced_files(
    src: &str,
    base: &Path,
    seen: &mut BTreeSet<PathBuf>,
    dependencies: &mut BTreeSet<PathBuf>,
    hasher: &mut blake3::Hasher,
) {
    for quoted in src.split('"').skip(1).step_by(2) {
        if quoted.is_empty() || quoted.starts_with('@') {
            continue;
        }
        let requested: PathBuf = base.join(quoted).components().collect();
        let path = requested
            .canonicalize()
            .unwrap_or_else(|_| requested.clone());
        if path.is_dir() {
            continue;
        }
        dependencies.insert(requested);
        dependencies.insert(path.clone());
        if !seen.insert(path.clone()) {
            continue;
        }
        let Ok(bytes) = fs::read(&path) else { continue };
        hasher.update(quoted.as_bytes());
        hasher.update(&bytes);
        if path.extension().is_some_and(|e| e == "typ") {
            if let Ok(text) = std::str::from_utf8(&bytes) {
                let parent = path.parent().unwrap_or(base).to_path_buf();
                hash_referenced_files(text, &parent, seen, dependencies, hasher);
            }
        }
    }
}

pub(crate) fn compile_fragment(
    src: &str,
    deck_dir: &Path,
    ppi: u32,
    display: bool,
    dependencies: &mut BTreeSet<PathBuf>,
) -> Result<PathBuf> {
    let body = if display {
        format!("$ {src} $")
    } else {
        format!("#context {{ let e = [${src}$]; box(inset: (y: measure(e).height * {INLINE_MATH_VERTICAL_PADDING_RATIO}), e) }}")
    };
    let wrapped = format!(
        "#set page(width: auto, height: auto, margin: 0pt, fill: none)\n#set text(fill: white)\n{MITEX_MATH_ALIASES}{body}"
    );
    let deps = dependency_digest(src, deck_dir, dependencies);
    let mut hash = blake3::Hasher::new();
    hash.update(&ppi.to_le_bytes());
    let root = deck_dir.as_os_str().as_encoded_bytes();
    hash.update(&(root.len() as u64).to_le_bytes());
    hash.update(root);
    hash.update(deps.as_bytes());
    hash.update(wrapped.as_bytes());
    let png = cache::dir().join(format!("typst-{}.png", hash.finalize().to_hex()));
    if png.is_file() {
        return Ok(png);
    }
    cache::publish_png(&png, |temporary| {
        let mut child = Command::new("typst")
            .arg("compile")
            .arg("--root")
            .arg(deck_dir)
            .arg("--ppi")
            .arg(ppi.to_string())
            .arg("-")
            .arg(temporary)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("spawn typst")?;
        let write_result = child
            .stdin
            .take()
            .context("typst stdin")?
            .write_all(wrapped.as_bytes())
            .context("write typst source");
        if write_result.is_err() {
            let _ = child.kill();
            let _ = child.wait();
            write_result?;
        }
        let result = child.wait_with_output();

        let out = result.context("run typst")?;
        if !out.status.success() {
            bail!(
                "typst compile failed:\n{}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
        Ok(())
    })?;
    Ok(png)
}
