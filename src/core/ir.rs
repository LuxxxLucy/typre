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

impl Slide {
    // A title slide leads with an H1; a normal slide leads with an H2.
    pub fn is_title(&self) -> bool {
        matches!(self.blocks.first(), Some(Block::Heading(1, _)))
    }
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
    // A fenced code block. `lang` is the fence's info string, absent when it has none, and
    // the label is drawn only when it is there. Nothing inside is evaluated.
    Code {
        src: String,
        lang: Option<String>,
    },
    // The lines of a ◊figure, each holding its own place, and the ◊ commands nested between
    // them. Inline markup inside a line is evaluated; only the reflow is suppressed.
    Art {
        parts: Vec<ArtPart>,
        caption: Vec<Inline>,
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
    // A collapsible box: the summary, and one entry per body line. Markup inside either is
    // evaluated; a body line holds its own place, as a figure line does.
    Details {
        summary: Vec<Inline>,
        body: Vec<Vec<Inline>>,
    },
}

// Part of a drawing: arrows, box drawing, block elements, geometric shapes.
pub(crate) fn draws(c: char) -> bool {
    matches!(c, '\u{2190}'..='\u{21ff}' | '\u{2500}'..='\u{25ff}')
}

// May lead an art line before its content: indentation, or the guides of a hand-drawn tree.
pub(crate) fn guides(c: char) -> bool {
    c.is_whitespace() || draws(c)
}


// One stretch of an art block. A nested command keeps the leading whitespace and vertical
// guides of the line it sat on, and every line it renders carries them.
#[derive(Debug, Clone)]
pub enum ArtPart {
    Lines(Vec<ArtLine>),
    Nested { guide: String, block: Box<Block> },
}

// One art line: the indentation and guides it opens with, kept as written, and the inline
// content after them.
#[derive(Debug, Clone)]
pub struct ArtLine {
    pub guide: String,
    pub inls: Vec<Inline>,
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
    // `display` lifts a lone fragment to its own block: set for `◊typst{}` and
    // `$$…$$`, cleared for inline `$…$`.
    InlineTypst { src: String, width: Width, display: bool },
    // A structured ◊ command (tree/grid/figure/details) restored mid-stream; the
    // paragraph fold lifts a lone one to its block. Never reaches inline rendering.
    BlockFragment(Box<Block>),
    SoftBreak,
    HardBreak,
}

pub fn plain_text(inlines: &[Inline]) -> String {
    inlines
        .iter()
        .filter_map(|inline| match inline {
            Inline::Text(text, _) | Inline::Code(text) => Some(text.as_str()),
            Inline::Link { label, .. } => Some(label.as_str()),
            _ => None,
        })
        .collect()
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Style {
    // Text that cycles through the spectrum frame by frame, to say a box opens on click.
    pub glow: bool,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub dim: bool,
    pub code: bool,
    pub quote: bool,
}

#[derive(Debug, Clone)]
pub enum RenderOp {
    MoveTo(u16, u16),
    Text(String, Style),
    LineBreak,
    Image {
        png_path: PathBuf,
        cols: u16,
        rows: u16,
    },
    // One placeholder line of a virtual placement `rows` tall: `row` selects the
    // slice. A table cell emits one op per line so borders stay in the text grid.
    InlineImage {
        png_path: PathBuf,
        cols: u16,
        rows: u16,
        row: u16,
    },
    Link {
        label: String,
        url: String,
        style: Style,
    },
    // Zero-width: the row it lands on toggles details box `id` when clicked.
    ToggleTarget(usize),
    ClearImages,
}
