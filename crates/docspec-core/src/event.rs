//! Event types for the streaming document pipeline.
//!
//! Events represent the atomic units of document structure. Sources emit events
//! in document order; sinks consume them. This decouples all readers from all writers.

/// A streaming document event.
///
/// Events flow from [`crate::EventSource`] readers to [`crate::EventSink`] writers. The enum is
/// marked `#[non_exhaustive]` to allow adding new event types in future versions.
///
/// Events come in three categories:
/// - **Start/End pairs**: Container elements like headings, paragraphs, tables
/// - **Self-contained**: Standalone elements like text, images, line breaks
/// - **Block vs Inline**: Block events create new vertical sections; inline events flow within blocks
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// End a block quote.
    EndBlockQuote,

    /// End a table caption.
    EndCaption,

    /// End a definition detail.
    EndDefinitionDetail,

    /// End a definition list.
    EndDefinitionList,

    /// End a definition term.
    EndDefinitionTerm,

    /// End a document.
    EndDocument,

    /// End a footnote definition.
    EndFootnote,

    /// End a heading.
    EndHeading,

    /// End a hyperlink.
    EndLink,

    /// End a list item.
    EndListItem,

    /// End a paragraph.
    EndParagraph,

    /// End a preformatted block.
    EndPreformatted,

    /// End a table.
    EndTable,

    /// End a table data cell.
    EndTableCell,

    /// End a table header cell.
    EndTableHeader,

    /// End a table row.
    EndTableRow,

    /// A reference to a footnote.
    FootnoteRef {
        /// The footnote identifier being referenced.
        id: u32,
    },

    /// An image reference.
    Image {
        /// Alternative text for accessibility.
        alt: Option<String>,
        /// Whether the image is purely decorative (no alt text needed).
        decorative: bool,
        /// Optional block identifier for the image.
        id: Option<String>,
        /// Source of the image (embedded asset or external URI).
        source: crate::ImageSource,
        /// Optional tooltip text.
        title: Option<String>,
    },

    /// A hard line break within a paragraph.
    LineBreak,

    /// Begin a block quote.
    StartBlockQuote {
        /// Optional block identifier.
        id: Option<String>,
    },

    /// Begin a table caption.
    StartCaption {
        /// Optional block identifier.
        id: Option<String>,
    },

    /// Begin a definition detail (description).
    StartDefinitionDetail {
        /// Optional block identifier.
        id: Option<String>,
    },

    /// Begin a definition list.
    StartDefinitionList {
        /// Optional block identifier.
        id: Option<String>,
    },

    /// Begin a definition term.
    StartDefinitionTerm {
        /// Optional block identifier.
        id: Option<String>,
    },

    /// Begin a document with optional language and metadata.
    StartDocument {
        /// Optional block identifier.
        id: Option<String>,
        /// BCP 47 language tag (e.g., "en", "en-US", "zh-Hans").
        language: Option<String>,
        /// Document metadata including title, authors, and description.
        metadata: Option<crate::DocumentMeta>,
    },

    /// Begin a footnote definition.
    StartFootnote {
        /// Unique identifier for this footnote.
        id: u32,
    },

    /// Begin a heading of the given level.
    StartHeading {
        /// Optional block identifier for the heading.
        id: Option<String>,
        /// Heading level, 1–9 (1 is most prominent).
        level: u8,
    },

    /// Begin a hyperlink.
    StartLink {
        /// URL or URI target of the link.
        href: String,
        /// Optional block identifier.
        id: Option<String>,
        /// Optional tooltip text.
        title: Option<String>,
    },

    /// Begin a list item.
    StartListItem {
        /// Optional block identifier.
        id: Option<String>,
        /// Nesting level (1 = top-level).
        level: u8,
        /// Whether the list is ordered or unordered.
        list_type: crate::ListType,
        /// Starting number for ordered lists (None = continue from previous).
        start: Option<u32>,
        /// Visual style for the list marker.
        style_type: Option<crate::ListStyleType>,
    },

    /// Begin a paragraph with optional alignment.
    StartParagraph {
        /// Text alignment for the paragraph.
        alignment: Option<crate::TextAlignment>,
        /// Optional block identifier for the paragraph.
        id: Option<String>,
    },

    /// Begin a preformatted (code) block with optional syntax highlighting.
    StartPreformatted {
        /// Optional block identifier.
        id: Option<String>,
        /// Language identifier for syntax highlighting (e.g., "rust", "python").
        syntax: Option<String>,
    },

    /// Begin a table.
    StartTable {
        /// Optional block identifier.
        id: Option<String>,
    },

    /// Begin a table data cell.
    StartTableCell {
        /// Number of columns this cell spans.
        colspan: Option<u32>,
        /// Optional block identifier.
        id: Option<String>,
        /// Number of rows this cell spans.
        rowspan: Option<u32>,
    },

    /// Begin a table header cell.
    StartTableHeader {
        /// Abbreviated content for accessibility.
        abbr: Option<String>,
        /// Number of columns this cell spans.
        colspan: Option<u32>,
        /// Optional block identifier.
        id: Option<String>,
        /// Number of rows this cell spans.
        rowspan: Option<u32>,
        /// Whether this header applies to a column or row.
        scope: Option<crate::TableHeaderScope>,
    },

    /// Begin a table row.
    StartTableRow {
        /// Optional block identifier.
        id: Option<String>,
    },

    /// A text run with formatting attributes.
    Text {
        /// Bold formatting.
        bold: bool,
        /// Monospace/code formatting.
        code: bool,
        /// The text content.
        content: String,
        /// Italic formatting.
        italic: bool,
        /// Highlight/mark color.
        mark: Option<crate::Color>,
        /// Strikethrough formatting.
        strikethrough: bool,
        /// Subscript formatting.
        subscript: bool,
        /// Superscript formatting.
        superscript: bool,
        /// Underline formatting.
        underline: bool,
    },

    /// A horizontal rule / thematic break.
    ThematicBreak {
        /// Optional block identifier.
        id: Option<String>,
    },
}
