//! Event types for the streaming document pipeline.
//!
//! Events represent the atomic units of document structure. Sources emit events
//! in document order; sinks consume them. This decouples all readers from all writers.

/// Text formatting attributes for an [`Event::Text`] event.
#[expect(
    clippy::struct_excessive_bools,
    reason = "TextStyle intentionally stores one boolean per formatting attribute for a simple builder API"
)]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TextStyle {
    /// Bold formatting.
    pub bold: bool,
    /// Monospace/code formatting.
    pub code: bool,
    /// Italic formatting.
    pub italic: bool,
    /// Highlight/mark color.
    pub mark: Option<crate::Color>,
    /// Strikethrough formatting.
    pub strikethrough: bool,
    /// Subscript formatting.
    pub subscript: bool,
    /// Superscript formatting.
    pub superscript: bool,
    /// Underline formatting.
    pub underline: bool,
}

impl TextStyle {
    /// Enables bold formatting.
    #[inline]
    #[must_use]
    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    /// Enables monospace/code formatting.
    #[inline]
    #[must_use]
    pub fn code(mut self) -> Self {
        self.code = true;
        self
    }

    /// Enables italic formatting.
    #[inline]
    #[must_use]
    pub fn italic(mut self) -> Self {
        self.italic = true;
        self
    }

    /// Sets the highlight/mark color.
    #[inline]
    #[must_use]
    pub fn mark(mut self, color: crate::Color) -> Self {
        self.mark = Some(color);
        self
    }

    /// Enables strikethrough formatting.
    #[inline]
    #[must_use]
    pub fn strikethrough(mut self) -> Self {
        self.strikethrough = true;
        self
    }

    /// Enables subscript formatting.
    #[inline]
    #[must_use]
    pub fn subscript(mut self) -> Self {
        self.subscript = true;
        self
    }

    /// Enables superscript formatting.
    #[inline]
    #[must_use]
    pub fn superscript(mut self) -> Self {
        self.superscript = true;
        self
    }

    /// Enables underline formatting.
    #[inline]
    #[must_use]
    pub fn underline(mut self) -> Self {
        self.underline = true;
        self
    }
}

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

    /// End an ordered (numbered) list item.
    EndOrderedListItem,

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

    /// End an unordered (bulleted) list item.
    EndUnorderedListItem,

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

    /// Begin an ordered (numbered) list item.
    StartOrderedListItem {
        /// Optional block identifier.
        id: Option<String>,
        /// Zero-indexed nesting depth (0 = top-level list).
        level: u32,
        /// Starting number for the list, populated only on the first item of an ordered list
        /// (subsequent items in the same list: `None`).
        start: Option<u64>,
        /// Visual style of the list marker. Writers tolerate mismatches per `ListStyleType` convention.
        style_type: crate::ListStyleType,
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

    /// Begin an unordered (bulleted) list item.
    StartUnorderedListItem {
        /// Optional block identifier.
        id: Option<String>,
        /// Zero-indexed nesting depth (0 = top-level list).
        level: u32,
        /// Visual style of the list marker. Writers tolerate mismatches per `ListStyleType` convention.
        style_type: crate::ListStyleType,
    },

    /// A text run with formatting attributes.
    Text {
        /// The text content.
        content: String,
        /// Text formatting attributes.
        style: TextStyle,
    },

    /// A horizontal rule / thematic break.
    ThematicBreak {
        /// Optional block identifier.
        id: Option<String>,
    },
}
