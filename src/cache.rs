// Rasterized PNGs, named by the hash of what produced them. The directory sits in the
// temporary directory, not beside the deck: nothing here is worth keeping, and nothing is
// worth writing into the author's folder.
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::Sender;
use std::sync::{Mutex, OnceLock};

use anyhow::{bail, Context, Result};

pub(crate) fn dir() -> PathBuf {
    std::env::temp_dir().join("typre-cache")
}

pub(crate) fn is_remote(src: &str) -> bool {
    src.starts_with("http://") || src.starts_with("https://")
}

fn redraw() -> &'static Mutex<Option<Sender<()>>> {
    static TX: OnceLock<Mutex<Option<Sender<()>>>> = OnceLock::new();
    TX.get_or_init(|| Mutex::new(None))
}

// The presenter hands over a channel it wakes on, so a download that lands replaces the alt
// text on screen without a keypress.
pub(crate) fn notify_redraws(tx: Sender<()>) {
    *redraw().lock().unwrap() = Some(tx);
}

// URLs a download has started for, kept whether it succeeded or not: one attempt per run, so
// a redraw does not queue the same file again and a dead link does not retry forever.
fn started() -> &'static Mutex<HashSet<String>> {
    static SEEN: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    SEEN.get_or_init(|| Mutex::new(HashSet::new()))
}

// The cached copy of a remote image. A miss starts the download in the background and gives
// nothing back, so the slide draws at once and the picture arrives on a later frame. Without
// a redraw channel there is no later frame, and the download is waited for.
pub(crate) fn remote_image(url: &str) -> Option<PathBuf> {
    let hash = blake3::hash(url.as_bytes()).to_hex().to_string();
    let png = dir().join(format!("{hash}.png"));
    if png.exists() {
        return Some(png);
    }
    let Some(tx) = redraw().lock().unwrap().clone() else {
        return download(url, &png).ok().map(|()| png);
    };
    if started().lock().unwrap().insert(url.to_string()) {
        let url = url.to_string();
        std::thread::spawn(move || {
            let _ = download(&url, &png);
            let _ = tx.send(());
        });
    }
    None
}

// The terminal is sent PNG bytes, so whatever comes over the network is decoded and written
// back out as PNG.
fn download(url: &str, png: &Path) -> Result<()> {
    fs::create_dir_all(dir()).context("create cache dir")?;
    let out = Command::new("curl")
        .args(["-sSLf", "--connect-timeout", "10", "--max-time", "120", url])
        .output()
        .context("spawn curl")?;
    if !out.status.success() {
        bail!(
            "download failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    image::load_from_memory(&out.stdout)
        .context("decode image")?
        .save(png)
        .context("write png")?;
    Ok(())
}
