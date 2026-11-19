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

/// A table cell: its source text (with `\|` unescaped) and the values a column fact compares
/// (every code span, else the cell's one value).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableCell {
    pub raw: String,
    pub values: Vec<String>,
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

impl Table {
    /// The values of the column at `index`, row by row; a row without that cell has one
    /// empty value.
    pub fn column_values(&self, index: usize) -> Vec<String> {
        self.rows
            .iter()
            .flat_map(|row| row.cells.get(index).map_or_else(|| vec![String::new()], |c| c.values.clone()))
            .collect()
    }
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
    /// Nothing but whitespace and other comments follows it on its last line
    /// ([`super::text::ends_line_past_comments`]).
    pub ends_line: bool,
    /// Index into [`Page::flow`] of the block that holds it: its paragraph, heading, HTML
    /// block or table.
    pub block: usize,
    /// Prose from the start of its paragraph, heading, list item or cell up to the comment.
    pub lead: String,
    /// Leaves before it in the same phrasing container, since the previous comment; for
    /// comments side by side, with only whitespace between, those before the first of them.
    pub before: Vec<Leaf>,
    /// The table whose cell holds it.
    pub table: Option<usize>,
    /// It sits on a line of its own: in an HTML block that holds nothing but comments.
    pub alone: bool,
}

/// Where the first sentence of a paragraph or heading ends, as a comment written there would
/// see it: a directive alone on the line above the block takes effect there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirstSentence {
    /// Index into [`Page::flow`] of the paragraph or heading.
    pub block: usize,
    /// The line the sentence ends on.
    pub line: usize,
    /// Nothing but whitespace and comments follows the sentence on that line.
    pub ends_line: bool,
    /// Prose from the start of the block to the sentence's end, as [`Comment::lead`].
    pub lead: String,
    /// Leaves of the sentence since the previous comment, as [`Comment::before`].
    pub before: Vec<Leaf>,
}

impl FirstSentence {
    /// The comment with `body`, as if it were written at the end of this sentence.
    pub fn as_comment(&self, body: &str) -> Comment {
        Comment {
            body: body.to_string(),
            line: self.line,
            end_line: self.line,
            ends_line: self.ends_line,
            block: self.block,
            lead: self.lead.clone(),
            before: self.before.clone(),
            table: None,
            alone: false,
        }
    }
}

/// A directive's keyword: the word a docattest directive starts with. The one list of them,
/// which finding a directive and finding one left unclosed both read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keyword {
    Fact,
    Cases,
    Test,
    /// A test file alias, `<!-- tests: <name> = <file> -->`.
    Tests,
}

impl Keyword {
    pub const ALL: [Keyword; 4] = [Keyword::Fact, Keyword::Cases, Keyword::Test, Keyword::Tests];

    pub fn as_str(self) -> &'static str {
        match self {
            Keyword::Fact => "fact",
            Keyword::Cases => "cases",
            Keyword::Test => "test",
            Keyword::Tests => "tests",
        }
    }

    /// The keyword a word is, if it is one.
    pub fn named(word: &str) -> Option<Keyword> {
        Keyword::ALL.into_iter().find(|k| k.as_str() == word)
    }

    /// Whether the keyword opens a directive only with a colon after it: `test` and `tests`
    /// are words of prose too (`<!-- testing -->`, `<!-- tests pending -->`).
    pub fn needs_colon(self) -> bool {
        matches!(self, Keyword::Test | Keyword::Tests)
    }
}

/// A `<!-- fact`, `<!-- cases`, `<!-- test:` or `<!-- tests:` opening that never closes.
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

/// What a page's frontmatter declares about it with its `docattest` key.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Declared {
    #[default]
    Nothing,
    /// `docattest: false`: the page is never checked.
    Frozen,
    /// `docattest: prose`: the page is checked, and has nothing to prove.
    Prose,
}

/// A parsed page.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Page {
    /// What its frontmatter declares, when it parses as YAML.
    pub declared: Declared,
    /// Every block-level node in document order, nested ones included. Binding and claims
    /// look here.
    pub flow: Vec<Block>,
    /// Top-level blocks, in order, as indexes into `flow`.
    pub blocks: Vec<usize>,
    pub tables: Vec<Table>,
    pub code: Vec<CodeRef>,
    pub comments: Vec<Comment>,
    pub unclosed: Vec<Unclosed>,
    /// The first sentence of every paragraph and heading, in block order.
    pub first_sentences: Vec<FirstSentence>,
}

impl Page {
    /// Whether its frontmatter freezes it.
    pub fn is_frozen(&self) -> bool {
        self.declared == Declared::Frozen
    }

    /// Whether the block at `index` is an HTML block of directives: one the page's comments
    /// sit in, which a reader never sees, and not prose.
    pub fn is_only_comments(&self, index: usize) -> bool {
        self.flow.get(index).is_some_and(|b| matches!(b.kind, BlockKind::Other))
            && self.comments.iter().any(|c| c.block == index)
    }

    /// Whether the block at `index` is lines of their own holding only comments.
    pub fn is_alone_comments(&self, index: usize) -> bool {
        self.comments.iter().any(|c| c.block == index && c.alone)
    }

    /// The first sentence of the paragraph or heading at `index`.
    pub fn first_sentence(&self, index: usize) -> Option<&FirstSentence> {
        self.first_sentences.iter().find(|s| s.block == index)
    }
}
