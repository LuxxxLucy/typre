use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

use anyhow::{anyhow, Result};

mod cache;
mod prepare;
mod typst;

use crate::core::ir::Slide;

#[derive(Hash, PartialEq, Eq)]
struct Fragment {
    src: String,
    ppi: u32,
    display: bool,
}

#[derive(Default)]
struct Generation {
    fragments: HashMap<Fragment, Result<PathBuf, String>>,
    images: HashMap<String, Option<PathBuf>>,
    dependencies: BTreeSet<PathBuf>,
    dimensions: HashMap<PathBuf, (u32, u32)>,
}

pub struct Assets {
    deck_dir: PathBuf,
    generation: RefCell<Generation>,
    redraw: RefCell<Option<Sender<()>>>,
    started: RefCell<HashSet<String>>,
}

impl Assets {
    pub fn new(deck_dir: &Path) -> Self {
        Self {
            deck_dir: deck_dir
                .canonicalize()
                .unwrap_or_else(|_| deck_dir.to_path_buf()),
            generation: RefCell::new(Generation::default()),
            redraw: RefCell::new(None),
            started: RefCell::new(HashSet::new()),
        }
    }

    pub fn fragment(&self, src: &str, ppi: u32, display: bool) -> Result<PathBuf> {
        let key = Fragment {
            src: src.to_owned(),
            ppi,
            display,
        };
        let mut generation = self.generation.borrow_mut();
        if let Some(result) = generation.fragments.get(&key) {
            return result.clone().map_err(|error| anyhow!(error));
        }
        let result = typst::compile_fragment(
            src,
            &self.deck_dir,
            ppi,
            display,
            &mut generation.dependencies,
        )
        .map_err(|error| format!("{error:#}"));
        generation.fragments.insert(key, result.clone());
        result.map_err(|error| anyhow!(error))
    }

    pub fn prepare(&self, slide: &Slide, ppi: u32) -> Vec<String> {
        prepare::prepare(slide, ppi, self)
    }

    pub fn image(&self, src: &str) -> Option<PathBuf> {
        if cache::is_remote(src) {
            return self.remote_image(src);
        }
        let mut generation = self.generation.borrow_mut();
        if let Some(result) = generation.images.get(src) {
            return result.clone();
        }
        let path = self.deck_dir.join(src);
        generation.dependencies.insert(path.clone());
        let result = std::fs::read(path)
            .ok()
            .and_then(|bytes| cache::snapshot(&bytes).ok());
        generation.images.insert(src.to_owned(), result.clone());
        result
    }

    fn remote_image(&self, url: &str) -> Option<PathBuf> {
        let png = cache::remote_path(url);
        if png.is_file() {
            return Some(png);
        }
        if !self.started.borrow_mut().insert(url.to_owned()) {
            return None;
        }
        let Some(tx) = self.redraw.borrow().clone() else {
            return cache::download(url, &png).ok().map(|()| png);
        };
        let url = url.to_owned();
        std::thread::spawn(move || {
            let _ = cache::download(&url, &png);
            let _ = tx.send(());
        });
        None
    }

    pub fn dimensions(&self, png: &Path) -> Option<(u32, u32)> {
        let mut generation = self.generation.borrow_mut();
        if let Some(size) = generation.dimensions.get(png) {
            return Some(*size);
        }
        let size = image::image_dimensions(png).ok()?;
        generation.dimensions.insert(png.to_path_buf(), size);
        Some(size)
    }

    pub fn invalidate(&self) {
        *self.generation.borrow_mut() = Generation::default();
    }

    pub fn set_redraw(&self, tx: Sender<()>) {
        *self.redraw.borrow_mut() = Some(tx);
    }

    pub fn dependencies(&self) -> Vec<PathBuf> {
        self.generation
            .borrow()
            .dependencies
            .iter()
            .cloned()
            .collect()
    }
}
