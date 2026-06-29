use std::path::PathBuf;

#[derive(Debug)]
pub struct Deck {
    pub meta: Meta,
    pub slides: Vec<Slide>,
}

#[derive(Debug, Default)]
pub struct Meta {
    pub title: Option<String>,
    pub author: Option<String>,
}

// `toc` is non-empty only on a title slide that opens the deck: its section list.
#[derive(Debug, Default)]
pub struct Slide {
    pub blocks: Vec<Block>,
    pub toc: Vec<TocEntry>,
}

// One table-of-contents line: the section title and the slide it jumps to.
#[derive(Debug, Clone)]
pub struct TocEntry {
    pub index: usize,
    pub title: String,
}

#[derive(Debug, Clone)]
pub enum Block {
    Heading(u8, Vec<Inline>),
    Paragraph(Vec<Inline>),
    List {
        ordered: bool,
        items: Vec<Vec<Block>>,
    },
    Code {
        src: String,
        lang: Option<String>,
    },
    BlockTypst {
        src: String,
        width: Width,
    },
    Image {
        src: String,
        alt: String,
    },
    Rule,
    Quote(Vec<Block>),
    Table {
        aligns: Vec<Align>,
        head: Vec<Vec<Inline>>,
        rows: Vec<Vec<Vec<Inline>>>,
    },
    Tree(Vec<TreeNode>),
    Grid(Vec<String>),
    Figure {
        body: String,
        caption: String,
    },
    Details {
        summary: String,
        body: Vec<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

// Display width of a block visual: natural (shrink-only), a percent of the
// content column, or an absolute column count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Width {
    #[default]
    Natural,
    Percent(u8),
    Cols(u16),
}

#[derive(Debug, Clone)]
pub struct TreeNode {
    pub label: String,
    pub children: Vec<TreeNode>,
}

#[derive(Debug, Clone)]
pub enum Inline {
    Text(String, Style),
    Code(String),
    Link { label: String, url: String },
    InlineTypst { src: String, width: Width },
    // A structured ◊ command (tree/grid/figure/details) restored mid-stream; the
    // paragraph fold lifts a lone one to its block. Never reaches inline rendering.
    BlockFragment(Box<Block>),
    SoftBreak,
    HardBreak,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub dim: bool,
    pub code: bool,
}

#[derive(Debug)]
pub enum RenderOp {
    MoveTo(u16, u16),
    Text(String, Style),
    LineBreak,
    Image {
        png_path: PathBuf,
        cols: u16,
        rows: u16,
    },
    InlineImage {
        png_path: PathBuf,
        cols: u16,
    },
    Link {
        label: String,
        url: String,
        style: Style,
    },
    ClearImages,
}
