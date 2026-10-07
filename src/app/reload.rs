use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};

use anyhow::Result;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Change {
    Document,
    Assets,
}

pub(super) struct Reload {
    watcher: RecommendedWatcher,
    events: Receiver<notify::Result<Event>>,
    document: PathBuf,
    documents: HashSet<PathBuf>,
    dependencies: HashSet<PathBuf>,
    directories: HashSet<PathBuf>,
}

fn absolute(path: &Path) -> PathBuf {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    };
    path.components().collect()
}

fn aliases(path: &Path) -> HashSet<PathBuf> {
    let path = absolute(path);
    let mut paths = HashSet::from([path.clone()]);
    if let Ok(resolved) = path.canonicalize() {
        paths.insert(resolved);
    }
    if let (Some(parent), Some(name)) = (path.parent(), path.file_name()) {
        if let Ok(parent) = parent.canonicalize() {
            paths.insert(parent.join(name));
        }
    }
    paths
}

impl Reload {
    pub fn new(document: &Path) -> Result<Self> {
        let (tx, events) = mpsc::channel();
        let watcher = notify::recommended_watcher(move |event| {
            let _ = tx.send(event);
        })?;
        let mut reload = Self {
            watcher,
            events,
            document: absolute(document),
            documents: HashSet::new(),
            dependencies: HashSet::new(),
            directories: HashSet::new(),
        };
        reload.track(Vec::new())?;
        Ok(reload)
    }

    pub fn track(&mut self, dependencies: Vec<PathBuf>) -> Result<()> {
        self.documents = aliases(&self.document);
        self.dependencies = dependencies.iter().flat_map(|p| aliases(p)).collect();
        let mut directories: HashSet<PathBuf> = self
            .dependencies
            .iter()
            .chain(&self.documents)
            .filter_map(|path| path.parent())
            .map(|parent| {
                let mut parent = parent;
                while !parent.is_dir() {
                    let Some(next) = parent.parent() else {
                        break;
                    };
                    parent = next;
                }
                parent
                    .canonicalize()
                    .unwrap_or_else(|_| parent.to_path_buf())
            })
            .collect();
        let candidates = directories.clone();
        directories.retain(|dir| {
            !candidates
                .iter()
                .any(|parent| parent != dir && dir.starts_with(parent))
        });
        for dir in directories.difference(&self.directories) {
            self.watcher.watch(dir, RecursiveMode::Recursive)?;
        }
        for dir in self.directories.difference(&directories) {
            self.watcher.unwatch(dir)?;
        }
        self.directories = directories;
        Ok(())
    }

    pub fn poll(&self) -> Result<Option<Change>> {
        let mut change = None;
        for event in self.events.try_iter() {
            let event = event?;
            let next = classify(&event, &self.documents, &self.dependencies);
            if next == Some(Change::Document) || change.is_none() {
                change = next;
            }
        }
        Ok(change)
    }
}

fn classify(
    event: &Event,
    documents: &HashSet<PathBuf>,
    dependencies: &HashSet<PathBuf>,
) -> Option<Change> {
    if !matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    ) {
        return None;
    }
    let paths: HashSet<PathBuf> = event.paths.iter().flat_map(|p| aliases(p)).collect();
    if paths
        .iter()
        .any(|path| documents.iter().any(|document| document.starts_with(path)))
    {
        Some(Change::Document)
    } else if paths.iter().any(|path| {
        dependencies
            .iter()
            .any(|dep| dep == path || dep.starts_with(path))
    }) {
        Some(Change::Assets)
    } else {
        None
    }
}
