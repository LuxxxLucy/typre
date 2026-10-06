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

#[derive(Debug, Default)]
pub struct Slide {
    pub blocks: Vec<Block>,
    pub toc: Vec<TocEntry>,
}

impl Slide {
    pub fn is_title(&self) -> bool {
        matches!(self.blocks.first(), Some(Block::Heading(1, _)))
    }
}

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
    Details {
        summary: Vec<Inline>,
        body: Vec<Vec<Inline>>,
    },
}

pub(crate) fn draws(c: char) -> bool {
    matches!(c, '\u{2190}'..='\u{21ff}' | '\u{2500}'..='\u{25ff}')
}

pub(crate) fn guides(c: char) -> bool {
    c.is_whitespace() || draws(c)
}

#[derive(Debug, Clone)]
pub enum ArtPart {
    Lines(Vec<ArtLine>),
    Nested { guide: String, block: Box<Block> },
}

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
    Link {
        label: String,
        url: String,
    },
    InlineTypst {
        src: String,
        width: Width,
        display: bool,
    },
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
    pub glow: bool,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub dim: bool,
    pub code: bool,
    pub quote: bool,
}
