//! The page model: what the planner needs from a Markdown page, with 1-based lines.

/// Whether a leaf is a code span or a run of text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeafKind {
    Code,
    Text,
}

/// A code span or a one-line run of text, in order, inside one phrasing container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Leaf {
    pub kind: LeafKind,
    pub value: String,
    pub line: usize,
}

/// A list item: its first line and its value (first code span, else its text).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListItem {
    pub line: usize,
    pub value: String,
}

/// A table cell: its source text (with `\|` unescaped) and its value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableCell {
    pub raw: String,
    pub value: String,
}

/// A table body row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableRow {
    pub line: usize,
    pub cells: Vec<TableCell>,
}

/// A table. Tables are kept in [`Page::tables`] and referred to by index, so a directive in
/// a cell can be matched to its table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    pub header: Vec<String>,
    pub rows: Vec<TableRow>,
}

/// What a block-level node is. `text` is prose: comments removed, whitespace collapsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockKind {
    Heading {
        text: String,
    },
    Paragraph {
        text: String,
    },
    List {
        items: Vec<ListItem>,
    },
    /// Index into [`Page::tables`].
    Table(usize),
    Code {
        info: String,
        value: String,
    },
    Other,
}

/// A block-level node and its first and last lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub start: usize,
    pub end: usize,
    pub kind: BlockKind,
    /// Index into [`Page::flow`] of the block just before it in the same container (a list
    /// item, a quote, the page). The first block of a container has the block before the
    /// container, and a list's items count as one container, so the first block of every
    /// item has the block before the list.
    pub previous: Option<usize>,
}

impl Block {
    /// The block's mdast type name.
    pub fn type_name(&self) -> &'static str {
        match self.kind {
            BlockKind::Heading { .. } => "heading",
            BlockKind::Paragraph { .. } => "paragraph",
            BlockKind::List { .. } => "list",
            BlockKind::Table(_) => "table",
            BlockKind::Code { .. } => "code",
            BlockKind::Other => "other",
        }
    }
}

/// An HTML comment and what surrounds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comment {
    /// The text between `<!--` and `-->`.
    pub body: String,
    pub line: usize,
    pub end_line: usize,
    /// Nothing but whitespace follows it on its last line.
    pub ends_line: bool,
    /// Index into [`Page::flow`] of the block that holds it: its paragraph, heading, HTML
    /// block or table.
    pub block: usize,
    /// Prose from the start of its paragraph, heading, list item or cell up to the comment.
    pub lead: String,
    /// Leaves before it in the same phrasing container, since the previous comment.
    pub before: Vec<Leaf>,
    /// The table whose cell holds it.
    pub table: Option<usize>,
}

/// The directive keyword of an opening that never closes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keyword {
    Fact,
    Cases,
    Test,
}

impl Keyword {
    pub fn as_str(self) -> &'static str {
        match self {
            Keyword::Fact => "fact",
            Keyword::Cases => "cases",
            Keyword::Test => "test",
        }
    }
}

/// A `<!-- fact`, `<!-- cases` or `<!-- test:` opening that never closes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unclosed {
    pub keyword: Keyword,
    pub line: usize,
    /// Index into [`Page::flow`] of the block that holds it.
    pub block: usize,
    pub table: Option<usize>,
}

/// A fenced (or indented) code block at any depth.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeRef {
    /// Index into [`Page::flow`].
    pub block: usize,
}

/// A parsed page.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Page {
    /// Frontmatter that parses as YAML and says `docattest: false`.
    pub frozen: bool,
    /// Every block-level node in document order, nested ones included. Binding and claims
    /// look here.
    pub flow: Vec<Block>,
    /// Top-level blocks, in order, as indexes into `flow`.
    pub blocks: Vec<usize>,
    pub tables: Vec<Table>,
    pub code: Vec<CodeRef>,
    pub comments: Vec<Comment>,
    pub unclosed: Vec<Unclosed>,
}
