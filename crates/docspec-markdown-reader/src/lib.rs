//! Markdown to `DocSpec` event stream reader.
//!
//! This crate provides a [`MarkdownReader`] that implements [`EventSource`] to convert
//! Markdown documents into the `DocSpec` event stream format. It uses `pulldown-cmark`
//! to parse CommonMark-compliant Markdown and emits typed events representing document
//! structure.
//!
//! # Quick Start
//!
//! ```
//! use docspec_markdown_reader::{MarkdownReader, EventSource};
//!
//! let markdown = "# Hello\n\nWorld";
//! let mut reader = MarkdownReader::from_str(markdown);
//!
//! while let Some(event) = reader.next_event()? {
//!     println!("{event:?}");
//! }
//! # Ok::<(), docspec_core::Error>(())
//! ```
//!
//! # Supported Elements
//!
//! - Headings (h1–h6) → `StartHeading` / `EndHeading`
//! - Paragraphs → `StartParagraph` / `EndParagraph`
//! - Block quotes → `StartBlockQuote` / `EndBlockQuote`
//! - Code blocks → `StartPreformatted` / `EndPreformatted`
//! - Bold text → `StartTextStyle { kind: Bold }` / `EndTextStyle`
//! - Italic text → `StartTextStyle { kind: Italic }` / `EndTextStyle`
//! - Inline code → `StartTextStyle { kind: Code }` / `EndTextStyle`
//! - Strikethrough → `StartTextStyle { kind: Strikethrough }` / `EndTextStyle`
//! - Images → `Image { source: Uri, alt, title, decorative }`
//! - Hard line breaks → `LineBreak`
//! - Soft line breaks → `SoftBreak`
//! - Thematic breaks → `ThematicBreak`
//! - Tables → `StartTable` / `EndTable`, `StartTableRow` / `EndTableRow`,
//!   `StartTableHeader` / `EndTableHeader`, `StartTableCell` / `EndTableCell`
//!   (GFM column alignment syntax is parsed, but alignment data is discarded)
//! - Bullet lists → `StartUnorderedListItem` / `EndUnorderedListItem`
//! - Numbered lists → `StartOrderedListItem` / `EndOrderedListItem`
//!   (`start: Option<u64>` is `Some(n)` on the first item of each list, `None` on subsequent items;
//!   child items may nest inside their parent's `Start*`/`End*` pair with `level` indicating
//!   indent depth; task list markers (`- [ ]`/`- [x]`) are parsed as literal text)
//! - Links → `StartLink { href, title }` / `EndLink` (inline, reference, collapsed,
//!   shortcut, autolink, and email autolink variants — all resolved to inline form
//!   by pulldown-cmark; image-inside-link closes the link before emitting the image
//!   as a sibling block: content preceding the image stays inside the link, content
//!   following the image is outside the link, and the link is empty only when the
//!   image is the sole link label, e.g. `[![alt](img)](url)`)
//!
//! # Supported Raw HTML Tags
//!
//! The following raw HTML tags embedded in markdown source are translated into
//! `DocSpec` events. All attributes on these tags are silently ignored. All other
//! HTML tags continue to be silently dropped.
//!
//! ## Inline formatting (translated to `StartTextStyle` / `EndTextStyle`)
//! - `<b>`, `<strong>` → `TextStyleKind::Bold`
//! - `<i>`, `<em>` → `TextStyleKind::Italic`
//! - `<u>` → `TextStyleKind::Underline`
//! - `<s>`, `<strike>`, `<del>` → `TextStyleKind::Strikethrough`
//! - `<code>` → `TextStyleKind::Code`
//! - `<sub>` → `TextStyleKind::Subscript`
//! - `<sup>` → `TextStyleKind::Superscript`
//! - `<mark>` → `TextStyleKind::Mark` with constant yellow `#FFFF00`
//!
//! ## Self-closing / void
//! - `<br>`, `<br/>`, `<br />` → `Event::LineBreak`
//! - `<hr>` → `Event::ThematicBreak` (block context only; ignored in paragraph context)
//!
//! ## Block (only inside an `HtmlBlock`)
//! - `<h1>`...`<h6>` → `Event::StartHeading { level: N }` + content + `Event::EndHeading`
//!
//! ## Known limitations
//! - Raw HTML `<pre><code>...</code></pre>` is NOT treated as a code block; the `<pre>` is dropped
//!   (out of scope) and the `<code>` becomes an inline style. Use markdown fenced code blocks instead.
//! - HTML attributes (id, class, style, href, src, etc.) are NOT extracted.
//! - Unclosed tags are auto-closed at the end of the containing block.
//!
//! # Unsupported Elements
//!
//! The following elements are not emitted as structured events. Text content is
//! recursively extracted where applicable; structure is silently dropped:
//! - Definition lists and footnotes
//! - Math blocks and inline math
//! - Subscript and superscript formatting (use `<sub>` / `<sup>` raw HTML instead)
//!
//! # Memory Model
//!
//! `MarkdownReader` owns its source text for the parser's lifetime. While events
//! are emitted one at a time via [`EventSource::next_event`] (the stream-event
//! guarantee is preserved), the source `String` is held in memory until the reader
//! is dropped. This is a constraint of `pulldown-cmark`, which is permanently
//! borrow-based by design (see [pulldown-cmark issue #463]).
//!
//! For contrast, `HtmlReader` (from `docspec-html-reader`) streams its source via a
//! 16 KB sliding-window buffer and does not hold the full document in memory.
//!
//! [pulldown-cmark issue #463]: https://github.com/raphlinus/pulldown-cmark/issues/463

extern crate alloc;

#[cfg_attr(all(), allow(clippy::mem_forget))]
mod parser_cell {
    use self_cell::self_cell;

    use super::MarkdownParser;

    self_cell!(
        pub(super) struct ParserCell {
            owner: String,
            #[covariant]
            dependent: MarkdownParser,
        }
    );
}

mod html;

use alloc::collections::VecDeque;
use std::io::{Read, Seek};

pub use docspec_core::EventSource;
use docspec_core::{Event, ImageSource, ListStyleType, Result, TableHeaderScope, TextStyleKind};
use parser_cell::ParserCell;
use pulldown_cmark::{CodeBlockKind, CowStr, HeadingLevel, Options, Parser, Tag, TagEnd};

struct MarkdownParser<'a>(Parser<'a>);

/// Whether content is inside a block-level element.
#[derive(Clone, Copy, PartialEq, Eq)]
enum BlockState {
    /// Inside an auto-opened paragraph (text arrived outside any explicit block).
    AutoParagraph,
    /// Inside an explicit block (from a `StartParagraph` or `StartHeading` tag).
    Explicit,
    /// Not inside any block context.
    None,
    /// Explicit block whose `StartParagraph` is deferred until the first real event.
    PendingExplicit,
}

/// Document processing phase.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// `EndDocument` has been emitted.
    Finished,
    /// `StartDocument` not yet emitted.
    NotStarted,
    /// Processing events between `StartDocument` and `EndDocument`.
    Running,
}

/// Context for a single list level tracked by [`MarkdownReader`].
struct ListContext {
    /// Whether the item at this list level is currently open (start emitted, end not yet emitted).
    item_open: bool,
    /// Whether this list is ordered (numbered) rather than unordered (bulleted).
    ordered: bool,
    /// Start number to attach to the next item emitted; `Some(n)` only before the first
    /// item is emitted, then `None` for all subsequent items in the same list.
    pending_start: Option<u64>,
}

/// Buffered image state during image alt text collection.
struct ImageBuffer {
    /// Accumulated alt text.
    alt_buf: String,
    /// Image title if provided.
    title: Option<String>,
    /// Image URL.
    url: String,
}

enum MarkdownPulldownEvent {
    BlockHtml(String),
    Code(String),
    End(TagEnd),
    HardBreak,
    InlineHtml(String),
    Ignored,
    Rule,
    SoftBreak,
    Start(MarkdownStartTag),
    Text(String),
}

enum MarkdownStartTag {
    BlockQuote,
    CodeBlock {
        syntax: Option<String>,
    },
    Emphasis,
    Heading {
        level: HeadingLevel,
    },
    Image {
        dest_url: String,
        title: Option<String>,
    },
    Item,
    Link {
        dest_url: String,
        title: Option<String>,
    },
    HtmlBlock,
    List(Option<u64>),
    Paragraph,
    Strikethrough,
    Strong,
    Table,
    TableCell,
    TableHead,
    TableRow,
}

/// Buffered link state during link inline content collection.
struct LinkBuffer {
    /// Link target URL.
    href: String,
    /// Whether `StartLink` has been emitted yet (deferred until first inline event arrives).
    started: bool,
    /// Optional link title (from `CommonMark` `[text](url "title")` syntax).
    title: Option<String>,
}

/// A streaming Markdown reader that implements [`EventSource`].
///
/// `MarkdownReader` parses Markdown using `pulldown-cmark` and emits `DocSpec` events
/// one at a time. It handles the mapping from `pulldown-cmark`'s event model to `DocSpec`'s
/// event model, including tracking inline formatting state.
///
/// # Example
///
/// ```
/// use docspec_markdown_reader::{MarkdownReader, EventSource};
///
/// let mut reader = MarkdownReader::from_str("**bold** and *italic*");
/// while let Some(event) = reader.next_event()? {
///     // Process events...
/// }
/// # Ok::<(), docspec_core::Error>(())
/// ```
pub struct MarkdownReader {
    /// Current block-level context.
    block_state: BlockState,
    /// Owned source text and parser borrowing from it.
    cell: ParserCell,
    /// Buffered code block text (accumulated until `EndCodeBlock` to strip trailing newline).
    code_block_buffer: Option<String>,
    /// Buffered image being processed (alt text accumulation).
    image: Option<ImageBuffer>,
    /// Heading accumulator for block HTML fragments.
    html_block_heading_acc: crate::html::translator::BlockHeadingAccumulator,
    /// Inline style stack scoped to block HTML headings.
    html_block_inline_stack: docspec_core::StyleStack,
    /// Whether the parser is currently inside a pulldown HTML block wrapper.
    in_html_block: bool,
    /// Whether the parser is currently inside a preformatted code block.
    in_preformatted: bool,
    /// Whether the parser is currently inside a table header row.
    in_table_head: bool,
    /// Buffered link being processed (deferred Start emission for image-in-link extraction).
    link: Option<LinkBuffer>,
    /// LIFO stack of list contexts. `len()` gives the current nesting depth;
    /// `level = list_stack.len().saturating_sub(1)` at item-emit time.
    list_stack: alloc::vec::Vec<ListContext>,
    /// Unified inline style stack shared by markdown emphasis and inline HTML.
    inline_style_stack: docspec_core::StyleStack,
    /// Document processing phase.
    phase: Phase,
    /// Queue of `DocSpec` events to emit.
    queue: VecDeque<Event>,
}

impl MarkdownReader {
    fn close_current_item_if_open(&mut self) {
        let Some(ctx) = self.list_stack.last() else {
            return;
        };
        if !ctx.item_open {
            return;
        }

        let ordered = ctx.ordered;
        self.flush_html_styles();
        if ordered {
            self.queue.push_back(Event::EndOrderedListItem);
        } else {
            self.queue.push_back(Event::EndUnorderedListItem);
        }
        if let Some(current_ctx) = self.list_stack.last_mut() {
            current_ctx.item_open = false;
        }
        self.block_state = BlockState::None;
    }

    fn close_style(&mut self, kind: &TextStyleKind) {
        if self.in_preformatted {
            return;
        }

        for event in self.inline_style_stack.close(kind) {
            self.queue.push_back(event);
        }
    }

    fn open_style(&mut self, kind: &TextStyleKind) {
        if !self.in_preformatted {
            for event in self.inline_style_stack.open(kind.clone()) {
                self.queue.push_back(event);
            }
        }
    }

    fn enqueue_text(&mut self, content: String) {
        for event in self.inline_style_stack.note_text() {
            self.queue.push_back(event);
        }
        let text_event = Event::Text { content };
        self.queue.push_back(text_event);
    }

    fn flush_html_styles(&mut self) {
        for event in self.inline_style_stack.close_all() {
            self.queue.push_back(event);
        }
    }

    /// Emits `StartLink` for the buffered link if it hasn't been emitted yet.
    /// Called before any inline event that would belong inside a link.
    fn emit_pending_link_start(&mut self) {
        self.flush_pending_paragraph_start();
        if let Some(link) = self.link.as_mut() {
            if !link.started {
                self.queue.push_back(Event::StartLink {
                    href: link.href.clone(),
                    id: None,
                    title: link.title.clone(),
                });
                link.started = true;
            }
        }
    }

    /// Emits `StartParagraph` for the deferred paragraph if it hasn't been emitted yet.
    /// Called before any committing event that would belong inside a paragraph.
    fn flush_pending_paragraph_start(&mut self) {
        if self.block_state == BlockState::PendingExplicit {
            self.queue.push_back(Event::StartParagraph {
                alignment: None,
                id: None,
            });
            self.block_state = BlockState::Explicit;
        }
    }

    fn from_owned_string(source: String) -> Self {
        let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
        let cell = ParserCell::new(source, |s| MarkdownParser(Parser::new_ext(s, options)));
        Self {
            block_state: BlockState::None,
            cell,
            code_block_buffer: None,
            image: None,
            html_block_heading_acc: crate::html::translator::BlockHeadingAccumulator::default(),
            html_block_inline_stack: docspec_core::StyleStack::default(),
            in_html_block: false,
            in_preformatted: false,
            in_table_head: false,
            link: None,
            list_stack: Vec::new(),
            inline_style_stack: docspec_core::StyleStack::default(),
            phase: Phase::NotStarted,
            queue: VecDeque::new(),
        }
    }

    /// Creates a `MarkdownReader` from any `Read + Seek` source.
    ///
    /// Reads the entire source into memory (required by `pulldown_cmark`'s
    /// borrow-based parser).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`](docspec_core::Error::Io) if reading fails.
    #[inline]
    pub fn from_reader<R>(mut reader: R) -> Result<Self>
    where
        R: Read + Seek + Send + 'static,
    {
        let mut source = String::new();
        reader.read_to_string(&mut source)?;
        Ok(Self::from_owned_string(source))
    }

    /// Creates a `MarkdownReader` from a string slice.
    ///
    /// The input is copied into an owned `String` for the parser's lifetime.
    ///
    /// # Example
    ///
    /// ```
    /// use docspec_markdown_reader::MarkdownReader;
    ///
    /// let reader = MarkdownReader::from_str("# Hello World");
    /// ```
    #[inline]
    #[must_use]
    #[expect(
        clippy::should_implement_trait,
        reason = "constructor name is required for reader API consistency"
    )]
    pub fn from_str(input: &str) -> Self {
        Self::from_owned_string(input.to_owned())
    }

    fn handle_code(&mut self, content: String) {
        if let Some(img) = &mut self.image {
            img.alt_buf.push_str(&content);
        } else {
            self.emit_pending_link_start();
            if self.block_state == BlockState::None {
                self.queue.push_back(Event::StartParagraph {
                    alignment: None,
                    id: None,
                });
                self.block_state = BlockState::AutoParagraph;
            }
            self.open_style(&TextStyleKind::Code);
            self.enqueue_text(content);
            self.close_style(&TextStyleKind::Code);
        }
    }

    /// Emits the buffered code block content (stripping the parser-added trailing newline)
    /// followed by `EndPreformatted`. Skips the text event if the buffer is empty.
    fn handle_end_code_block(&mut self) {
        if let Some(buf) = self.code_block_buffer.take() {
            let content = buf.strip_suffix('\n').unwrap_or(&buf).to_owned();
            if !content.is_empty() {
                self.enqueue_text(content);
            }
        }
        self.in_preformatted = false;
        self.push_event_end(Event::EndPreformatted);
    }

    /// Emits an `Image` event from the accumulated image buffer, deriving
    /// `decorative = true` when the trimmed alt text is empty. Consumes the
    /// in-progress image state; does nothing if no image is in progress.
    fn handle_end_image(&mut self) {
        let Some(img) = self.image.take() else { return };
        self.flush_pending_paragraph_start();
        let trimmed = img.alt_buf.trim();
        let alt = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_owned())
        };
        let decorative = alt.is_none();
        self.queue.push_back(Event::Image {
            source: ImageSource::Uri { uri: img.url },
            alt,
            title: img.title,
            decorative,
            id: None,
        });
    }

    /// Closes an auto-opened paragraph if one is open, then closes the current
    /// list item and resets block state.
    fn handle_end_item(&mut self) {
        if self.block_state == BlockState::AutoParagraph {
            self.flush_html_styles();
            self.queue.push_back(Event::EndParagraph);
        }
        self.close_current_item_if_open();
        self.block_state = BlockState::None;
    }

    /// Emits `EndLink` (and `StartLink` if not yet emitted) for the buffered link.
    fn handle_end_link(&mut self) {
        let Some(link) = self.link.take() else { return };
        if link.started {
            self.queue.push_back(Event::EndLink);
        } else {
            self.flush_pending_paragraph_start();
            self.queue.push_back(Event::StartLink {
                href: link.href,
                id: None,
                title: link.title,
            });
            self.queue.push_back(Event::EndLink);
        }
    }

    /// Closes the current list item if open, pops the list context, and resets block state.
    fn handle_end_list(&mut self) {
        self.close_current_item_if_open();
        self.list_stack.pop();
        self.block_state = BlockState::None;
    }

    /// Emits `EndTableCell` or `EndTableHeader` depending on whether the parser
    /// is currently inside a table header row.
    fn handle_end_table_cell(&mut self) {
        if self.in_table_head {
            self.push_event_end(Event::EndTableHeader);
        } else {
            self.push_event_end(Event::EndTableCell);
        }
    }

    /// Emits `EndTableRow` and clears the table-head flag for a table head closing tag.
    fn handle_end_table_head(&mut self) {
        self.push_event_end(Event::EndTableRow);
        self.in_table_head = false;
    }

    /// Dispatches a `pulldown-cmark` end tag to the appropriate per-tag handler.
    ///
    /// Tags in the explicit ignore list below are known-unsupported elements whose
    /// structure is intentionally dropped (text content may still be extracted by
    /// other event handlers).
    fn handle_end_tag(&mut self, tag_end: TagEnd) {
        match tag_end {
            TagEnd::BlockQuote(_) => self.push_event_end(Event::EndBlockQuote),
            TagEnd::CodeBlock => self.handle_end_code_block(),
            TagEnd::Emphasis => self.close_style(&TextStyleKind::Italic),
            TagEnd::Heading(_) => self.push_event_end(Event::EndHeading),
            TagEnd::HtmlBlock => {
                self.in_html_block = false;
                for event in self.html_block_inline_stack.close_all() {
                    self.queue.push_back(event);
                }
                if let Some(event) = self.html_block_heading_acc.finish_block() {
                    self.queue.push_back(event);
                }
            }
            TagEnd::Image => self.handle_end_image(),
            TagEnd::Item => self.handle_end_item(),
            TagEnd::Link => self.handle_end_link(),
            TagEnd::List(_) => self.handle_end_list(),
            TagEnd::Paragraph => {
                if self.block_state == BlockState::PendingExplicit {
                    self.flush_html_styles();
                    self.block_state = BlockState::None;
                } else {
                    self.push_event_end(Event::EndParagraph);
                }
            }
            TagEnd::Strikethrough => self.close_style(&TextStyleKind::Strikethrough),
            TagEnd::Strong => self.close_style(&TextStyleKind::Bold),
            TagEnd::Table => self.push_event_end(Event::EndTable),
            TagEnd::TableCell => self.handle_end_table_cell(),
            TagEnd::TableHead => self.handle_end_table_head(),
            TagEnd::TableRow => self.push_event_end(Event::EndTableRow),
            // Tags intentionally ignored (structure dropped, text extracted elsewhere):
            TagEnd::DefinitionList
            | TagEnd::DefinitionListDefinition
            | TagEnd::DefinitionListTitle
            | TagEnd::FootnoteDefinition
            | TagEnd::MetadataBlock(_)
            | TagEnd::Subscript
            | TagEnd::Superscript => {}
        }
    }

    fn handle_item_start(&mut self) {
        let depth = self.list_stack.len().saturating_sub(1);
        let level = u32::try_from(depth).map_or(u32::MAX, |v| v);
        if let Some(ctx) = self.list_stack.last_mut() {
            if ctx.ordered {
                self.queue.push_back(Event::StartOrderedListItem {
                    start: ctx.pending_start.take(),
                    style_type: ListStyleType::Decimal,
                    level,
                    id: None,
                });
            } else {
                self.queue.push_back(Event::StartUnorderedListItem {
                    style_type: ListStyleType::Disc,
                    level,
                    id: None,
                });
            }
            ctx.item_open = true;
            self.block_state = BlockState::Explicit;
        }
    }

    fn handle_list_start(&mut self, start_opt: Option<u64>) {
        self.list_stack.push(ListContext {
            item_open: false,
            ordered: start_opt.is_some(),
            pending_start: start_opt,
        });
    }

    /// Emits `StartPreformatted` for a code block opening tag, initialising
    /// the internal code-block buffer for content accumulation.
    fn handle_start_code_block(&mut self, syntax: Option<String>) {
        self.code_block_buffer = Some(String::new());
        self.in_preformatted = true;
        self.push_event_start(Event::StartPreformatted { id: None, syntax });
    }

    /// Emits `StartHeading` after mapping a `pulldown-cmark` `HeadingLevel` to a `u8` level.
    fn handle_start_heading(&mut self, level: HeadingLevel) {
        let level_u8 = match level {
            HeadingLevel::H1 => 1,
            HeadingLevel::H2 => 2,
            HeadingLevel::H3 => 3,
            HeadingLevel::H4 => 4,
            HeadingLevel::H5 => 5,
            HeadingLevel::H6 => 6,
        };
        self.push_event_start(Event::StartHeading {
            level: level_u8,
            id: None,
        });
    }

    /// Initialises image state for alt-text accumulation when an image opening tag is
    /// encountered. The title is stored as `None` when the pulldown-cmark title string
    /// is empty.
    fn handle_start_image(&mut self, dest_url: String, title: Option<String>) {
        // Image-in-link extraction: close the link before processing the image so the
        // image can be emitted as a sibling block (BlockNote and similar schemas do not
        // allow block-level images inside inline links). When `link.started` is true, the
        // link already contains preceding inline content — emit only `EndLink`. When it
        // is false (image is the sole link label, e.g. `[![alt](img)](url)`), emit an
        // empty `StartLink`/`EndLink` pair so the URL is preserved. `TagEnd::Image` fires
        // `Event::Image` before `TagEnd::Paragraph`, so downstream writers close the
        // surrounding paragraph before serialising the image as a sibling block.
        self.flush_pending_paragraph_start();
        if let Some(link) = self.link.take() {
            if link.started {
                self.queue.push_back(Event::EndLink);
            } else {
                self.queue.push_back(Event::StartLink {
                    href: link.href,
                    id: None,
                    title: link.title,
                });
                self.queue.push_back(Event::EndLink);
            }
        }

        self.image = Some(ImageBuffer {
            alt_buf: String::new(),
            title,
            url: dest_url,
        });
    }

    /// Stores link state for deferred `StartLink` emission.
    ///
    /// Emission is deferred until the first inline event arrives (lazy emission).
    /// This allows image-in-link to be detected before any `StartLink` is emitted.
    fn handle_start_link(&mut self, dest_url: String, title: Option<String>) {
        self.link = Some(LinkBuffer {
            href: dest_url,
            started: false,
            title,
        });
    }

    /// Emits `StartTableHeader` or `StartTableCell` depending on whether the parser
    /// is currently inside a table header row.
    fn handle_start_table_cell(&mut self) {
        if self.in_table_head {
            self.push_event_start(Event::StartTableHeader {
                scope: Some(TableHeaderScope::Column),
                abbr: None,
                colspan: None,
                rowspan: None,
                id: None,
            });
        } else {
            self.push_event_start(Event::StartTableCell {
                colspan: None,
                rowspan: None,
                id: None,
            });
        }
    }

    /// Sets the table-head flag and emits `StartTableRow` for a table head opening tag.
    fn handle_start_table_head(&mut self) {
        self.in_table_head = true;
        self.push_event_start(Event::StartTableRow { id: None });
    }

    /// Dispatches a `pulldown-cmark` start tag to the appropriate per-tag handler.
    ///
    /// Tags in the explicit ignore list below are known-unsupported elements whose
    /// structure is intentionally dropped (text content may still be extracted by
    /// other event handlers).
    fn handle_start_tag(&mut self, tag: MarkdownStartTag) {
        match tag {
            MarkdownStartTag::BlockQuote => {
                self.push_event_start(Event::StartBlockQuote { id: None });
            }
            MarkdownStartTag::CodeBlock { syntax } => self.handle_start_code_block(syntax),
            MarkdownStartTag::Emphasis => self.open_style(&TextStyleKind::Italic),
            MarkdownStartTag::Heading { level } => self.handle_start_heading(level),
            MarkdownStartTag::HtmlBlock => self.in_html_block = true,
            MarkdownStartTag::Image { dest_url, title } => self.handle_start_image(dest_url, title),
            MarkdownStartTag::Item => self.handle_item_start(),
            MarkdownStartTag::Link { dest_url, title } => self.handle_start_link(dest_url, title),
            MarkdownStartTag::List(start_opt) => self.handle_list_start(start_opt),
            MarkdownStartTag::Paragraph => self.block_state = BlockState::PendingExplicit,
            MarkdownStartTag::Strikethrough => self.open_style(&TextStyleKind::Strikethrough),
            MarkdownStartTag::Strong => self.open_style(&TextStyleKind::Bold),
            MarkdownStartTag::Table => self.push_event_start(Event::StartTable { id: None }),
            MarkdownStartTag::TableCell => self.handle_start_table_cell(),
            MarkdownStartTag::TableHead => self.handle_start_table_head(),
            MarkdownStartTag::TableRow => self.push_event_start(Event::StartTableRow { id: None }),
        }
    }

    fn handle_text(&mut self, content: String) {
        if let Some(img) = &mut self.image {
            img.alt_buf.push_str(&content);
        } else if let Some(buf) = &mut self.code_block_buffer {
            buf.push_str(&content);
        } else {
            self.emit_pending_link_start();
            if self.block_state == BlockState::None {
                self.queue.push_back(Event::StartParagraph {
                    alignment: None,
                    id: None,
                });
                self.block_state = BlockState::AutoParagraph;
            }
            self.enqueue_text(content);
        }
    }

    fn next_pulldown_event(&mut self) -> Option<MarkdownPulldownEvent> {
        self.cell.with_dependent_mut(|_, dep| {
            dep.0.next().map(|event| match event {
                pulldown_cmark::Event::Start(tag) => markdown_start_tag(tag)
                    .map_or(MarkdownPulldownEvent::Ignored, MarkdownPulldownEvent::Start),
                pulldown_cmark::Event::End(tag_end) => MarkdownPulldownEvent::End(tag_end),
                pulldown_cmark::Event::Text(text) => {
                    MarkdownPulldownEvent::Text(text.into_string())
                }
                pulldown_cmark::Event::Code(code) => {
                    MarkdownPulldownEvent::Code(code.into_string())
                }
                pulldown_cmark::Event::HardBreak => MarkdownPulldownEvent::HardBreak,
                pulldown_cmark::Event::SoftBreak => MarkdownPulldownEvent::SoftBreak,
                pulldown_cmark::Event::Rule => MarkdownPulldownEvent::Rule,
                pulldown_cmark::Event::InlineHtml(tag_str) => {
                    MarkdownPulldownEvent::InlineHtml(tag_str.into_string())
                }
                pulldown_cmark::Event::Html(fragment) => {
                    MarkdownPulldownEvent::BlockHtml(fragment.into_string())
                }
                pulldown_cmark::Event::DisplayMath(_)
                | pulldown_cmark::Event::FootnoteReference(_)
                | pulldown_cmark::Event::InlineMath(_)
                | pulldown_cmark::Event::TaskListMarker(_) => MarkdownPulldownEvent::Ignored,
            })
        })
    }

    fn process_next_pulldown_event(&mut self) {
        let Some(pm_event) = self.next_pulldown_event() else {
            if self.phase != Phase::Finished {
                self.phase = Phase::Finished;
                self.flush_html_styles();
                self.queue.push_back(Event::EndDocument);
            }
            return;
        };

        match pm_event {
            MarkdownPulldownEvent::BlockHtml(fragment) => {
                let events = crate::html::translator::translate_block(
                    &fragment,
                    &mut self.html_block_heading_acc,
                    &mut self.html_block_inline_stack,
                    self.in_preformatted,
                );
                for event in events {
                    match event {
                        Event::Text { content } => self.enqueue_text(content),
                        other => self.queue.push_back(other),
                    }
                }
            }
            MarkdownPulldownEvent::Start(tag) => self.handle_start_tag(tag),
            MarkdownPulldownEvent::End(tag_end) => self.handle_end_tag(tag_end),
            MarkdownPulldownEvent::Text(text) => self.handle_text(text),
            MarkdownPulldownEvent::Code(code) => self.handle_code(code),
            MarkdownPulldownEvent::InlineHtml(fragment) => {
                let events = crate::html::translator::translate_inline(
                    &fragment,
                    &mut self.inline_style_stack,
                    self.in_preformatted,
                );
                for event in events {
                    match event {
                        Event::Text { content } => self.enqueue_text(content),
                        other => self.queue.push_back(other),
                    }
                }
            }
            MarkdownPulldownEvent::HardBreak => {
                if let Some(img) = &mut self.image {
                    img.alt_buf.push(' ');
                } else if self.block_state == BlockState::PendingExplicit {
                    // emitting a break before StartParagraph would be malformed — discard
                } else {
                    self.emit_pending_link_start();
                    self.queue.push_back(Event::LineBreak);
                }
            }
            MarkdownPulldownEvent::SoftBreak => {
                if let Some(img) = &mut self.image {
                    img.alt_buf.push(' ');
                } else if self.block_state == BlockState::PendingExplicit {
                    // emitting a break before StartParagraph would be malformed — discard
                } else {
                    self.emit_pending_link_start();
                    self.queue.push_back(Event::SoftBreak);
                }
            }
            MarkdownPulldownEvent::Rule => {
                self.queue.push_back(Event::ThematicBreak { id: None });
            }
            MarkdownPulldownEvent::Ignored => {}
        }
    }

    fn push_event(&mut self, event: Event, state: BlockState) {
        self.queue.push_back(event);
        self.block_state = state;
    }

    fn push_event_end(&mut self, event: Event) {
        self.flush_html_styles();
        self.push_event(event, BlockState::None);
    }

    fn push_event_start(&mut self, event: Event) {
        self.push_event(event, BlockState::Explicit);
    }
}

impl EventSource for MarkdownReader {
    #[inline]
    fn next_event(&mut self) -> Result<Option<Event>> {
        if self.phase == Phase::NotStarted {
            self.phase = Phase::Running;
            return Ok(Some(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            }));
        }

        if self.phase == Phase::Finished && self.queue.is_empty() {
            return Ok(None);
        }

        while self.queue.is_empty() && self.phase != Phase::Finished {
            self.process_next_pulldown_event();
        }

        Ok(self.queue.pop_front())
    }
}

fn markdown_start_tag(tag: Tag<'_>) -> Option<MarkdownStartTag> {
    match tag {
        Tag::BlockQuote(_) => Some(MarkdownStartTag::BlockQuote),
        Tag::CodeBlock(kind) => Some(MarkdownStartTag::CodeBlock {
            syntax: code_block_syntax(kind),
        }),
        Tag::Emphasis => Some(MarkdownStartTag::Emphasis),
        Tag::Heading { level, .. } => Some(MarkdownStartTag::Heading { level }),
        Tag::Image {
            dest_url, title, ..
        } => Some(MarkdownStartTag::Image {
            dest_url: dest_url.into_string(),
            title: cow_to_optional_string(title),
        }),
        Tag::HtmlBlock => Some(MarkdownStartTag::HtmlBlock),
        Tag::Item => Some(MarkdownStartTag::Item),
        Tag::Link {
            dest_url, title, ..
        } => Some(MarkdownStartTag::Link {
            dest_url: dest_url.into_string(),
            title: cow_to_optional_string(title),
        }),
        Tag::List(start_opt) => Some(MarkdownStartTag::List(start_opt)),
        Tag::Paragraph => Some(MarkdownStartTag::Paragraph),
        Tag::Strikethrough => Some(MarkdownStartTag::Strikethrough),
        Tag::Strong => Some(MarkdownStartTag::Strong),
        Tag::Table(_) => Some(MarkdownStartTag::Table),
        Tag::TableCell => Some(MarkdownStartTag::TableCell),
        Tag::TableHead => Some(MarkdownStartTag::TableHead),
        Tag::TableRow => Some(MarkdownStartTag::TableRow),
        Tag::DefinitionList
        | Tag::DefinitionListDefinition
        | Tag::DefinitionListTitle
        | Tag::FootnoteDefinition(_)
        | Tag::MetadataBlock(_)
        | Tag::Subscript
        | Tag::Superscript => None,
    }
}

fn code_block_syntax(kind: CodeBlockKind<'_>) -> Option<String> {
    match kind {
        CodeBlockKind::Fenced(lang) if !lang.is_empty() => Some(lang.into_string()),
        CodeBlockKind::Fenced(_) | CodeBlockKind::Indented => None,
    }
}

fn cow_to_optional_string(value: CowStr<'_>) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value.into_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_code_without_open_block_auto_opens_paragraph() {
        let mut reader = MarkdownReader::from_str("");
        reader.handle_code("code".to_string());

        assert_eq!(reader.queue.len(), 4);
        assert_eq!(
            reader.queue.front(),
            Some(&Event::StartParagraph {
                alignment: None,
                id: None,
            })
        );
        assert_eq!(
            reader.queue.get(1),
            Some(&Event::StartTextStyle {
                kind: TextStyleKind::Code,
                id: None,
            })
        );
        assert_eq!(
            reader.queue.get(2),
            Some(&Event::Text {
                content: "code".to_string(),
            })
        );
        assert_eq!(reader.queue.get(3), Some(&Event::EndTextStyle));
    }

    #[test]
    fn handle_text_without_open_block_auto_opens_paragraph() {
        let mut reader = MarkdownReader::from_str("");
        reader.handle_text("hello".to_string());

        assert_eq!(reader.queue.len(), 2);
        assert_eq!(
            reader.queue.front(),
            Some(&Event::StartParagraph {
                alignment: None,
                id: None,
            })
        );
        assert_eq!(
            reader.queue.get(1),
            Some(&Event::Text {
                content: "hello".to_string(),
            })
        );
    }
}

#[cfg(test)]
mod send_static_assertions {
    fn assert_send_static<T>()
    where
        T: Send + 'static,
    {
    }

    #[test]
    fn markdown_reader_is_send_static() {
        assert_send_static::<crate::MarkdownReader>();
    }
}
