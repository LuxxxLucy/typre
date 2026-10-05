//! Parse content, prepare assets, compose rows, and emit terminal output.
//!
//! ```
//! use std::{collections::HashSet, path::Path};
//! use typre::{assets::Assets, core::parse::parse, layout::TermInfo, render, term::Emitter};
//!
//! let document = parse("## Example\n\nA paragraph.");
//! let assets = Assets::new(Path::new("."));
//! let terminal = TermInfo { cols: 80, rows: 24, cell_w_px: 8, cell_h_px: 16 };
//! let body = render::body(&document.slides[0], &terminal, &assets, &HashSet::new());
//! let view = render::frame::View { total: document.slides.len(), ..Default::default() };
//! let frame = render::frame::compose(&body, &terminal, &document.meta, &view);
//! let mut bytes = Vec::new();
//! Emitter::new().emit(&frame.ops, &mut bytes)?;
//! # Ok::<(), anyhow::Error>(())
//! ```

pub mod app;
pub mod assets;
pub mod core;
mod diacritics;
pub mod layout;
pub mod render;
pub mod term;
