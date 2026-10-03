use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};

use super::cache;

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

// Hash the local files a fragment imports, recursing through .typ imports.
fn imported_files_digest(
    src: &str,
    deck_dir: &Path,
    dependencies: &mut BTreeSet<PathBuf>,
) -> String {
    let mut hasher = blake3::Hasher::new();
    let mut seen = BTreeSet::new();
    collect_refs(src, deck_dir, &mut seen, dependencies, &mut hasher);
    hasher.finalize().to_hex().to_string()
}

fn collect_refs(
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
                collect_refs(text, &parent, seen, dependencies, hasher);
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
    // Pad inline math vertically so the glyph fills ~85% of the one-cell line height.
    let body = if display {
        format!("$ {src} $")
    } else {
        format!("#context {{ let e = [${src}$]; box(inset: (y: measure(e).height * 0.088), e) }}")
    };
    let wrapped = format!(
        "#set page(width: auto, height: auto, margin: 0pt, fill: none)\n#set text(fill: white)\n{MATH_SHIM}{body}"
    );
    let deps = imported_files_digest(src, deck_dir, dependencies);
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
        // The source goes in on stdin, so no scratch file lands next to the deck. A relative
        // import in it resolves against `--root`, which is the deck's own directory.
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
#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn dependency_scan_keeps_link_paths_and_stops_link_cycles() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        fs::write(
            root.join("target.typ"),
            "#include \"alias.typ\"\n#read(\"missing.txt\")",
        )
        .unwrap();
        symlink("target.typ", root.join("alias.typ")).unwrap();
        let mut dependencies = BTreeSet::new();
        imported_files_digest("#include \"alias.typ\"", &root, &mut dependencies);
        assert_eq!(
            dependencies,
            BTreeSet::from([
                root.join("alias.typ"),
                root.join("target.typ"),
                root.join("missing.txt"),
            ])
        );
    }

    #[test]
    fn dependency_scan_handles_cycles_directories_and_missing_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        fs::write(root.join("a.typ"), "#include \"./b.typ\"").unwrap();
        fs::write(
            root.join("b.typ"),
            "#include \"./a.typ\"\n#read(\"missing.txt\")",
        )
        .unwrap();
        let mut dependencies = BTreeSet::new();
        let first =
            imported_files_digest("#include \"a.typ\"\n\".\"\n\"\"", &root, &mut dependencies);
        assert_eq!(
            dependencies,
            BTreeSet::from([
                root.join("a.typ"),
                root.join("b.typ"),
                root.join("missing.txt"),
            ])
        );
        fs::write(root.join("missing.txt"), "created").unwrap();
        let second = imported_files_digest(
            "#include \"a.typ\"\n\".\"\n\"\"",
            &root,
            &mut BTreeSet::new(),
        );
        assert_ne!(first, second);
    }
}
