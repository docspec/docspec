//! Event types for the streaming document pipeline.
//!
//! `DocSpec` documents are streams of typed events. Readers ([`crate::EventSource`])
//! emit events; writers ([`crate::EventSink`]) consume them in document order.
//! This module defines every event type and the rules for well-formed streams.
//!
//! For higher-level design decisions, see the
//! [Architecture document](https://github.com/docspec/docspec/blob/main/ARCHITECTURE.md).
//!
//! # Error Handling
//!
//! Events never carry errors; errors flow out-of-band via [`crate::Result`]. See
//! [`crate::EventSource::next_event`] for full semantics. Readers recover silently
//! when possible (missing optional attributes, unrecognized elements, unsupported
//! features) and return `Err` only on fatal conditions (malformed structure,
//! truncated stream, invalid encoding).
//!
//! # Asset References
//!
//! [`Event::Image`] carries an [`crate::ImageSource`] (asset handle or URI), not bytes.
//! Writers resolve bytes lazily via [`crate::AssetHandle`]; assets must remain
//! accessible until [`Event::EndDocument`].
//!
//! # Well-Formedness Rules
//!
//! Readers MUST produce well-formed streams; writers MAY assume well-formedness.
//!
//! 1. Every `Start*` has exactly one matching `End*`. They nest but never overlap.
//! 2. Exactly one root: [`Event::StartDocument`]. Empty containers are valid.
//! 3. [`Event::Text`] appears only inside containers, never at root.
//! 4. [`Event::StartLink`] appears inside inline-accepting blocks (paragraphs,
//!    headings, list items, cells, definition details) and does not nest.
//! 5. List items ([`Event::StartOrderedListItem`], [`Event::StartUnorderedListItem`])
//!    appear inside block containers and may nest; `level` is 0-indexed.
//! 6. [`Event::StartCaption`] appears at most once per table, before any rows.
//! 7. Each footnote id appears in exactly one [`Event::FootnoteRef`] and one
//!    [`Event::StartFootnote`].
//! 8. [`Event::StartTableRow`] appears only inside [`Event::StartTable`];
//!    [`Event::StartTableCell`] and [`Event::StartTableHeader`] only inside
//!    [`Event::StartTableRow`].
//! 9. Readers MUST normalize overlapping source styles into nested
//!    [`Event::StartTextStyle`] spans via close-and-reopen.
//! 10. All open [`Event::StartTextStyle`] spans MUST close before the enclosing
//!     block-end event ([`Event::EndParagraph`], [`Event::EndHeading`],
//!     [`Event::EndOrderedListItem`], [`Event::EndUnorderedListItem`],
//!     [`Event::EndTableCell`], [`Event::EndTableHeader`], [`Event::EndCaption`],
//!     [`Event::EndDefinitionTerm`], [`Event::EndDefinitionDetail`]).
//! 11. [`Event::StartTextStyle`] and [`Event::StartPreformatted`] MUST NOT nest
//!     inside each other.
//! 12. Inside a link, [`Event::StartLink`] SHOULD be the outer container and
//!     [`Event::StartTextStyle`] the inner.
//! 13. Empty [`Event::StartTextStyle`] spans MUST NOT be emitted: at least one
//!     [`Event::Text`] event must appear before the matching
//!     [`Event::EndTextStyle`].

/// The kind of text formatting carried by a [`Event::StartTextStyle`] event.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TextStyleKind {
    /// Bold formatting.
    Bold,
    /// Italic formatting.
    Italic,
    /// Monospace/code formatting.
    Code,
    /// Strikethrough formatting.
    Strikethrough,
    /// Underline formatting.
    Underline,
    /// Subscript formatting.
    ///
    /// May be active simultaneously with [`TextStyleKind::Superscript`] by nesting;
    /// writers that cannot represent both prefer `Superscript`.
    Subscript,
    /// Superscript formatting.
    ///
    /// May be active simultaneously with [`TextStyleKind::Subscript`] by nesting;
    /// writers that cannot represent both prefer `Superscript`.
    Superscript,
    /// Highlight/mark color formatting. The variant carries the highlight color.
    Mark(crate::Color),
    /// Foreground (text) color. Carries an explicit RGB color.
    TextColor(crate::Color),
}

/// A streaming document event.
///
/// Events flow from [`crate::EventSource`] readers to [`crate::EventSink`] writers.
/// The enum is marked `#[non_exhaustive]` to allow adding new event types in
/// future versions; downstream consumers must include a wildcard `_ =>` arm when
/// matching.
///
/// Events come in three categories:
///
/// - **Start/End pairs**: Container elements like headings, paragraphs, tables.
///   Every `Start*` has exactly one matching `End*` (Rule 1 in module docs).
/// - **Self-contained**: Standalone elements like text, images, line breaks.
/// - **Block vs Inline**: Block events create new vertical sections; inline
///   events flow within blocks.
///
/// See the [module-level documentation](self) for error handling, asset
/// references, and the full well-formedness ruleset.
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

    /// End an inline text style span.
    EndTextStyle,

    /// End an unordered (bulleted) list item.
    EndUnorderedListItem,

    /// A reference to a footnote.
    ///
    /// Inline marker; the corresponding [`Event::StartFootnote`] definition appears
    /// elsewhere in the stream (before or after this reference, depending on source
    /// format). Each footnote ID appears in exactly one `FootnoteRef` and one
    /// [`Event::StartFootnote`] (Rule 7 in module docs).
    FootnoteRef {
        /// The footnote identifier being referenced.
        id: u32,
    },

    /// An image reference.
    ///
    /// Asset bytes resolve lazily via [`crate::AssetHandle`]. `decorative` means
    /// purely visual — no alt text is needed for accessibility. Images may appear
    /// inline within paragraphs/headings or directly in block containers.
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
    ///
    /// Explicit hard break (e.g., markdown two-space-newline, HTML `<br>`).
    LineBreak,

    /// A soft line break in source markup, such as a markdown line wrap.
    ///
    /// Soft breaks correspond to source line wraps that do not enforce a
    /// visible break. Writers choose rendering policy: space, newline,
    /// `<br>`, etc.
    SoftBreak,

    /// Begin a block quote.
    ///
    /// May contain any block element.
    StartBlockQuote {
        /// Optional block identifier.
        id: Option<String>,
    },

    /// Begin a table caption.
    ///
    /// Appears at most once per table, before any rows (Rule 6 in module docs).
    StartCaption {
        /// Optional block identifier.
        id: Option<String>,
    },

    /// Begin a definition detail (description).
    ///
    /// Details can contain any block element.
    StartDefinitionDetail {
        /// Optional block identifier.
        id: Option<String>,
    },

    /// Begin a definition list.
    ///
    /// Contains [`Event::StartDefinitionTerm`] / [`Event::StartDefinitionDetail`]
    /// pairs.
    StartDefinitionList {
        /// Optional block identifier.
        id: Option<String>,
    },

    /// Begin a definition term.
    ///
    /// Terms contain inline content only.
    StartDefinitionTerm {
        /// Optional block identifier.
        id: Option<String>,
    },

    /// Begin a document with optional language and metadata.
    ///
    /// The root container — exactly one per stream (Rule 2 in module docs).
    /// `language` is a BCP 47 tag (e.g., `"en"`, `"en-US"`, `"zh-Hans"`).
    StartDocument {
        /// Optional block identifier.
        id: Option<String>,
        /// BCP 47 language tag (e.g., "en", "en-US", "zh-Hans").
        language: Option<String>,
        /// Document metadata including title, authors, and description.
        metadata: Option<crate::DocumentMeta>,
    },

    /// Begin a footnote definition.
    ///
    /// Readers emit `StartFootnote` as soon as practical; placement varies by
    /// source format. The corresponding [`Event::FootnoteRef`] may appear before
    /// or after this definition. Writers decide final placement and must buffer
    /// if needed. Footnotes contain paragraphs only; this restriction may relax
    /// in future versions.
    StartFootnote {
        /// Unique identifier for this footnote.
        id: u32,
    },

    /// Begin a heading of the given level.
    ///
    /// Levels 1–6 are standard (HTML). DOCX/ODT/RTF support 1–9. Writers clamp
    /// higher levels to their format's maximum. Heading levels are 1-based (range
    /// 1–9); list item `level` (on [`Event::StartOrderedListItem`] and
    /// [`Event::StartUnorderedListItem`]) is 0-indexed.
    StartHeading {
        /// Optional block identifier for the heading.
        id: Option<String>,
        /// Heading level, 1–9 (1 is most prominent).
        level: u8,
    },

    /// Begin a hyperlink.
    ///
    /// An inline container (uses Start/End because it carries `href`). Valid
    /// inside paragraphs, headings, list items, cells, and definition details.
    /// Links do not nest (Rule 4 in module docs).
    StartLink {
        /// URL or URI target of the link.
        href: String,
        /// Optional block identifier.
        id: Option<String>,
        /// Optional tooltip text.
        title: Option<String>,
    },

    /// Begin an ordered (numbered) list item.
    ///
    /// See [`Event::StartUnorderedListItem`] for nesting and list-boundary
    /// semantics — they apply identically here. `start` is populated only on
    /// the first item of an ordered list; subsequent items use `None`.
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
    ///
    /// Inside `StartPreformatted`/[`Event::EndPreformatted`], no
    /// [`Event::StartTextStyle`] events appear (Rule 11 in module docs). When
    /// `syntax` is present, the block has code semantics. Newlines in content
    /// are literal.
    StartPreformatted {
        /// Optional block identifier.
        id: Option<String>,
        /// Language identifier for syntax highlighting (e.g., "rust", "python").
        syntax: Option<String>,
    },

    /// Begin a table.
    ///
    /// Contains an optional [`Event::StartCaption`] (at most one, before any
    /// rows), then [`Event::StartTableRow`] events. Cells may contain any block
    /// element.
    StartTable {
        /// Optional block identifier.
        id: Option<String>,
    },

    /// Begin a table data cell.
    ///
    /// Data cells omit the `scope` and `abbr` fields carried by
    /// [`Event::StartTableHeader`]; use the header variant for cells that
    /// describe other cells.
    StartTableCell {
        /// Number of columns this cell spans.
        colspan: Option<u32>,
        /// Optional block identifier.
        id: Option<String>,
        /// Number of rows this cell spans.
        rowspan: Option<u32>,
    },

    /// Begin a table header cell.
    ///
    /// Header cells carry `scope` and `abbr` for accessibility; data cells (use
    /// [`Event::StartTableCell`]) omit these.
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

    /// Begin an inline text style span.
    ///
    /// Valid inside paragraphs, headings, list items, cells, and definition
    /// details. Style spans nest but never overlap (Rules 1 and 9 in module
    /// docs); readers MUST close-and-reopen to express overlapping source
    /// styles. The [`TextStyleKind::Mark`] variant carries the highlight color;
    /// [`TextStyleKind::TextColor`] carries the foreground text color.
    StartTextStyle {
        /// The style kind opened by this span.
        kind: TextStyleKind,
        /// Optional block identifier for the style span.
        id: Option<String>,
    },

    /// Begin an unordered (bulleted) list item.
    ///
    /// Child items nest inside the parent's `Start*`/`End*` pair; the parent's
    /// [`Event::EndUnorderedListItem`] appears AFTER all children and any
    /// continuation content (paragraphs, line breaks) belonging to the parent.
    /// `level` is 0-indexed and authoritative — writers may rely on it alone.
    ///
    /// **List boundaries** (applies to [`Event::StartOrderedListItem`] as well):
    /// a new list begins when (a) a non-list block intervenes, (b) ordered vs.
    /// unordered changes at the same level, or (c) level decreases then
    /// increases without a parent.
    StartUnorderedListItem {
        /// Optional block identifier.
        id: Option<String>,
        /// Zero-indexed nesting depth (0 = top-level list).
        level: u32,
        /// Visual style of the list marker. Writers tolerate mismatches per `ListStyleType` convention.
        style_type: crate::ListStyleType,
    },

    /// A text run.
    ///
    /// Whitespace is significant. Outside preformatted blocks, newlines in
    /// content are collapsed to whitespace; readers emit [`Event::LineBreak`]
    /// for explicit hard breaks (e.g., markdown two-space-newline, HTML `<br>`)
    /// and [`Event::SoftBreak`] for soft breaks (e.g., source line wraps
    /// within a paragraph). Inline formatting is expressed via surrounding
    /// [`Event::StartTextStyle`]/[`Event::EndTextStyle`] wrapper events; the
    /// `Text` event itself carries content only.
    Text {
        /// The text content.
        content: String,
    },

    /// A horizontal rule / thematic break.
    ///
    /// Section separator. Self-contained block event.
    ThematicBreak {
        /// Optional block identifier.
        id: Option<String>,
    },
}
