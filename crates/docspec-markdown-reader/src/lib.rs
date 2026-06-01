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
//! let mut reader = MarkdownReader::new(markdown);
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
//! - Bold text → `Text { style: TextStyle { bold: true, .. }, .. }`
//! - Italic text → `Text { style: TextStyle { italic: true, .. }, .. }`
//! - Inline code → `Text { style: TextStyle { code: true, .. }, .. }`
//! - Strikethrough → `Text { style: TextStyle { strikethrough: true, .. }, .. }`
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
//! # Unsupported Elements
//!
//! The following elements are not emitted as structured events. Text content is
//! recursively extracted where applicable; structure is silently dropped:
//! - Definition lists and footnotes
//! - HTML blocks and inline HTML
//! - Math blocks and inline math
//! - Subscript and superscript formatting

extern crate alloc;

use alloc::collections::VecDeque;

pub use docspec_core::EventSource;
use docspec_core::{Depth, Event, ImageSource, ListStyleType, Result, TableHeaderScope, TextStyle};
use pulldown_cmark::{CodeBlockKind, CowStr, HeadingLevel, Options, Parser, Tag, TagEnd};

/// Whether content is inside a block-level element.
#[derive(Clone, Copy, PartialEq, Eq)]
enum BlockState {
    /// Inside an auto-opened paragraph (text arrived outside any explicit block).
    AutoParagraph,
    /// Inside an explicit block (from a `StartParagraph` or `StartHeading` tag).
    Explicit,
    /// Not inside any block context.
    None,
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
/// let mut reader = MarkdownReader::new("**bold** and *italic*");
/// while let Some(event) = reader.next_event()? {
///     // Process events...
/// }
/// # Ok::<(), docspec_core::Error>(())
/// ```
pub struct MarkdownReader<'a> {
    /// Current block-level context.
    block_state: BlockState,
    /// Nesting depth for bold (strong) formatting.
    bold_depth: Depth,
    /// Buffered code block text (accumulated until `EndCodeBlock` to strip trailing newline).
    code_block_buffer: Option<String>,
    /// Buffered image being processed (alt text accumulation).
    image: Option<ImageBuffer>,
    /// Whether the parser is currently inside a table header row.
    in_table_head: bool,
    /// Nesting depth for italic (emphasis) formatting.
    italic_depth: Depth,
    /// Buffered link being processed (deferred Start emission for image-in-link extraction).
    link: Option<LinkBuffer>,
    /// LIFO stack of list contexts. `len()` gives the current nesting depth;
    /// `level = list_stack.len().saturating_sub(1)` at item-emit time.
    list_stack: alloc::vec::Vec<ListContext>,
    /// The pulldown-cmark parser.
    parser: Parser<'a>,
    /// Document processing phase.
    phase: Phase,
    /// Queue of `DocSpec` events to emit.
    queue: VecDeque<Event>,
    /// Nesting depth for strikethrough formatting.
    strikethrough_depth: Depth,
}

impl<'a> MarkdownReader<'a> {
    fn close_current_item_if_open(&mut self) {
        if let Some(ctx) = self.list_stack.last_mut() {
            if ctx.item_open {
                if ctx.ordered {
                    self.queue.push_back(Event::EndOrderedListItem);
                } else {
                    self.queue.push_back(Event::EndUnorderedListItem);
                }
                ctx.item_open = false;
                self.block_state = BlockState::None;
            }
        }
    }

    fn current_text_style(&self) -> TextStyle {
        let mut style = TextStyle::default();
        if self.bold_depth.is_positive() {
            style = style.bold();
        }
        if self.italic_depth.is_positive() {
            style = style.italic();
        }
        if self.strikethrough_depth.is_positive() {
            style = style.strikethrough();
        }
        style
    }

    /// Emits `StartLink` for the buffered link if it hasn't been emitted yet.
    /// Called before any inline event that would belong inside a link.
    fn emit_pending_link_start(&mut self) {
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
            self.queue.push_back(Event::Text {
                content,
                style: self.current_text_style().code(),
            });
        }
    }

    /// Emits the buffered code block content (stripping the parser-added trailing newline)
    /// followed by `EndPreformatted`. Skips the text event if the buffer is empty.
    fn handle_end_code_block(&mut self) {
        if let Some(buf) = self.code_block_buffer.take() {
            let content = buf.strip_suffix('\n').unwrap_or(&buf).to_owned();
            if !content.is_empty() {
                self.queue.push_back(Event::Text {
                    content,
                    style: TextStyle::default().code(),
                });
            }
        }
        self.push_event_end(Event::EndPreformatted);
    }

    /// Emits an `Image` event from the accumulated image buffer, deriving
    /// `decorative = true` when the trimmed alt text is empty. Consumes the
    /// in-progress image state; does nothing if no image is in progress.
    fn handle_end_image(&mut self) {
        let Some(img) = self.image.take() else { return };
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
            TagEnd::Emphasis => self.italic_depth.dec(),
            TagEnd::Heading(_) => self.push_event_end(Event::EndHeading),
            TagEnd::Image => self.handle_end_image(),
            TagEnd::Item => self.handle_end_item(),
            TagEnd::Link => self.handle_end_link(),
            TagEnd::List(_) => self.handle_end_list(),
            TagEnd::Paragraph => self.push_event_end(Event::EndParagraph),
            TagEnd::Strikethrough => self.strikethrough_depth.dec(),
            TagEnd::Strong => self.bold_depth.dec(),
            TagEnd::Table => self.push_event_end(Event::EndTable),
            TagEnd::TableCell => self.handle_end_table_cell(),
            TagEnd::TableHead => self.handle_end_table_head(),
            TagEnd::TableRow => self.push_event_end(Event::EndTableRow),
            // Tags intentionally ignored (structure dropped, text extracted elsewhere):
            TagEnd::DefinitionList
            | TagEnd::DefinitionListDefinition
            | TagEnd::DefinitionListTitle
            | TagEnd::FootnoteDefinition
            | TagEnd::HtmlBlock
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
    fn handle_start_code_block(&mut self, kind: CodeBlockKind<'a>) {
        let syntax = match kind {
            CodeBlockKind::Fenced(lang) if !lang.is_empty() => Some(lang.into_string()),
            CodeBlockKind::Fenced(_) | CodeBlockKind::Indented => None,
        };
        self.code_block_buffer = Some(String::new());
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
    fn handle_start_image(&mut self, dest_url: CowStr<'a>, title: CowStr<'a>) {
        // Image-in-link extraction: close the link before processing the image so the
        // image can be emitted as a sibling block (BlockNote and similar schemas do not
        // allow block-level images inside inline links). When `link.started` is true, the
        // link already contains preceding inline content — emit only `EndLink`. When it
        // is false (image is the sole link label, e.g. `[![alt](img)](url)`), emit an
        // empty `StartLink`/`EndLink` pair so the URL is preserved. `TagEnd::Image` fires
        // `Event::Image` before `TagEnd::Paragraph`, so downstream writers close the
        // surrounding paragraph before serialising the image as a sibling block.
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
            title: if title.is_empty() {
                None
            } else {
                Some(title.into_string())
            },
            url: dest_url.into_string(),
        });
    }

    /// Stores link state for deferred `StartLink` emission.
    ///
    /// Emission is deferred until the first inline event arrives (lazy emission).
    /// This allows image-in-link to be detected before any `StartLink` is emitted.
    fn handle_start_link(&mut self, dest_url: CowStr<'a>, title: CowStr<'a>) {
        self.link = Some(LinkBuffer {
            href: dest_url.into_string(),
            started: false,
            title: if title.is_empty() {
                None
            } else {
                Some(title.into_string())
            },
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
    fn handle_start_tag(&mut self, tag: Tag<'a>) {
        match tag {
            Tag::BlockQuote(_) => self.push_event_start(Event::StartBlockQuote { id: None }),
            Tag::CodeBlock(kind) => self.handle_start_code_block(kind),
            Tag::Emphasis => self.italic_depth.inc(),
            Tag::Heading { level, .. } => self.handle_start_heading(level),
            Tag::Image {
                dest_url, title, ..
            } => self.handle_start_image(dest_url, title),
            Tag::Item => self.handle_item_start(),
            Tag::Link {
                dest_url, title, ..
            } => self.handle_start_link(dest_url, title),
            Tag::List(start_opt) => self.handle_list_start(start_opt),
            Tag::Paragraph => self.push_event_start(Event::StartParagraph {
                alignment: None,
                id: None,
            }),
            Tag::Strikethrough => self.strikethrough_depth.inc(),
            Tag::Strong => self.bold_depth.inc(),
            Tag::Table(_) => self.push_event_start(Event::StartTable { id: None }),
            Tag::TableCell => self.handle_start_table_cell(),
            Tag::TableHead => self.handle_start_table_head(),
            Tag::TableRow => self.push_event_start(Event::StartTableRow { id: None }),
            // Tags intentionally ignored (structure dropped, text extracted elsewhere):
            Tag::DefinitionList
            | Tag::DefinitionListDefinition
            | Tag::DefinitionListTitle
            | Tag::FootnoteDefinition(_)
            | Tag::HtmlBlock
            | Tag::MetadataBlock(_)
            | Tag::Subscript
            | Tag::Superscript => {}
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
            self.queue.push_back(Event::Text {
                content,
                style: self.current_text_style(),
            });
        }
    }

    /// Creates a new `MarkdownReader` from the given Markdown string.
    ///
    /// The reader will emit `StartDocument` as its first event and `EndDocument`
    /// as its last event, with the parsed content events in between.
    ///
    /// # Example
    ///
    /// ```
    /// use docspec_markdown_reader::MarkdownReader;
    ///
    /// let reader = MarkdownReader::new("# Hello World");
    /// ```
    #[inline]
    #[must_use]
    pub fn new(markdown: &'a str) -> Self {
        let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
        let parser = Parser::new_ext(markdown, options);
        Self {
            block_state: BlockState::None,
            bold_depth: Depth::default(),
            code_block_buffer: None,
            image: None,
            in_table_head: false,
            italic_depth: Depth::default(),
            link: None,
            list_stack: Vec::new(),
            parser,
            phase: Phase::NotStarted,
            queue: VecDeque::new(),
            strikethrough_depth: Depth::default(),
        }
    }

    fn process_next_pulldown_event(&mut self) {
        let Some(pm_event) = self.parser.next() else {
            if self.phase != Phase::Finished {
                self.phase = Phase::Finished;
                self.queue.push_back(Event::EndDocument);
            }
            return;
        };

        match pm_event {
            pulldown_cmark::Event::Start(tag) => self.handle_start_tag(tag),
            pulldown_cmark::Event::End(tag_end) => self.handle_end_tag(tag_end),
            pulldown_cmark::Event::Text(text) => self.handle_text(text.into_string()),
            pulldown_cmark::Event::Code(code) => self.handle_code(code.into_string()),
            pulldown_cmark::Event::HardBreak => {
                if let Some(img) = &mut self.image {
                    img.alt_buf.push(' ');
                } else {
                    self.emit_pending_link_start();
                    self.queue.push_back(Event::LineBreak);
                }
            }
            pulldown_cmark::Event::SoftBreak => {
                if let Some(img) = &mut self.image {
                    img.alt_buf.push(' ');
                } else {
                    self.emit_pending_link_start();
                    self.queue.push_back(Event::SoftBreak);
                }
            }
            pulldown_cmark::Event::Rule => {
                self.queue.push_back(Event::ThematicBreak { id: None });
            }
            pulldown_cmark::Event::DisplayMath(_)
            | pulldown_cmark::Event::FootnoteReference(_)
            | pulldown_cmark::Event::Html(_)
            | pulldown_cmark::Event::InlineHtml(_)
            | pulldown_cmark::Event::InlineMath(_)
            | pulldown_cmark::Event::TaskListMarker(_) => {}
        }
    }

    fn push_event(&mut self, event: Event, state: BlockState) {
        self.queue.push_back(event);
        self.block_state = state;
    }

    fn push_event_end(&mut self, event: Event) {
        self.push_event(event, BlockState::None);
    }

    fn push_event_start(&mut self, event: Event) {
        self.push_event(event, BlockState::Explicit);
    }
}

impl EventSource for MarkdownReader<'_> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_code_without_open_block_auto_opens_paragraph() {
        let mut reader = MarkdownReader::new("");
        reader.handle_code("code".to_string());

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
                content: "code".to_string(),
                style: TextStyle::default().code(),
            })
        );
    }
}
