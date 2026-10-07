use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

pub(crate) fn dir() -> PathBuf {
    std::env::temp_dir().join("typre-cache")
}

pub(crate) fn is_remote(src: &str) -> bool {
    src.starts_with("http://") || src.starts_with("https://")
}

pub(crate) fn remote_path(url: &str) -> PathBuf {
    dir().join(format!(
        "remote-{}.png",
        blake3::hash(url.as_bytes()).to_hex()
    ))
}

pub(crate) fn publish_png(path: &Path, write: impl FnOnce(&Path) -> Result<()>) -> Result<()> {
    let parent = path.parent().context("cache path parent")?;
    fs::create_dir_all(parent).context("create cache dir")?;
    let temporary = tempfile::Builder::new()
        .suffix(".png")
        .tempfile_in(parent)?;
    write(temporary.path())?;
    temporary.persist(path).context("publish cached png")?;
    Ok(())
}

fn save_image(bytes: &[u8], path: &Path) -> Result<()> {
    let image = image::load_from_memory(bytes).context("decode image")?;
    publish_png(path, |temporary| {
        image
            .save_with_format(temporary, image::ImageFormat::Png)
            .context("write png")
    })
}

pub(crate) fn snapshot(bytes: &[u8]) -> Result<PathBuf> {
    let png = dir().join(format!("image-{}.png", blake3::hash(bytes).to_hex()));
    if !png.is_file() {
        save_image(bytes, &png)?;
    }
    Ok(png)
}

pub(crate) fn download(url: &str, png: &Path) -> Result<()> {
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
    save_image(&out.stdout, png)
}
