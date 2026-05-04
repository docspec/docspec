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
//! - Bold text → `Text { bold: true, ... }`
//! - Italic text → `Text { italic: true, ... }`
//! - Inline code → `Text { code: true, ... }`
//! - Images → `Image { source: Uri, alt, title, decorative }`
//! - Hard line breaks → `LineBreak`
//! - Thematic breaks → `ThematicBreak`
//!
//! # Unsupported Elements
//!
//! Tables, lists, block quotes, code blocks, and links are not emitted as structured
//! events. Their text content is recursively extracted; structure is silently dropped.

extern crate alloc;

use alloc::collections::VecDeque;

pub use docspec_core::EventSource;
use docspec_core::{Event, ImageSource, Result};
use pulldown_cmark::{HeadingLevel, Options, Parser, Tag, TagEnd};

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

/// Buffered image state during image alt text collection.
struct ImageBuffer {
    /// Accumulated alt text.
    alt_buf: String,
    /// Image title if provided.
    title: Option<String>,
    /// Image URL.
    url: String,
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
    bold_depth: usize,
    /// Buffered image being processed (alt text accumulation).
    image: Option<ImageBuffer>,
    /// Nesting depth for italic (emphasis) formatting.
    italic_depth: usize,
    /// The pulldown-cmark parser.
    parser: Parser<'a>,
    /// Document processing phase.
    phase: Phase,
    /// Queue of `DocSpec` events to emit.
    queue: VecDeque<Event>,
}

impl<'a> MarkdownReader<'a> {
    fn handle_code(&mut self, content: String) {
        if let Some(img) = &mut self.image {
            img.alt_buf.push_str(&content);
        } else {
            if self.block_state == BlockState::None {
                self.queue.push_back(Event::StartParagraph {
                    alignment: None,
                    id: None,
                });
                self.block_state = BlockState::AutoParagraph;
            }
            self.queue.push_back(Event::Text {
                content,
                bold: self.bold_depth > 0,
                italic: self.italic_depth > 0,
                code: true,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
            });
        }
    }

    fn handle_end_tag(&mut self, tag_end: TagEnd) {
        match tag_end {
            TagEnd::Heading(_) => self.push_event_end(Event::EndHeading),
            TagEnd::Paragraph => self.push_event_end(Event::EndParagraph),
            TagEnd::BlockQuote(_) | TagEnd::Item | TagEnd::TableCell => {
                if self.block_state == BlockState::AutoParagraph {
                    self.push_event_end(Event::EndParagraph);
                }
            }
            TagEnd::Emphasis => {
                self.italic_depth = self.italic_depth.saturating_sub(1);
            }
            TagEnd::Strong => {
                self.bold_depth = self.bold_depth.saturating_sub(1);
            }
            TagEnd::Image => {
                if let Some(img) = self.image.take() {
                    let alt = {
                        let trimmed = img.alt_buf.trim();
                        if trimmed.is_empty() {
                            None
                        } else {
                            Some(trimmed.to_owned())
                        }
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
            }
            TagEnd::CodeBlock
            | TagEnd::DefinitionList
            | TagEnd::DefinitionListDefinition
            | TagEnd::DefinitionListTitle
            | TagEnd::FootnoteDefinition
            | TagEnd::HtmlBlock
            | TagEnd::Link
            | TagEnd::List(_)
            | TagEnd::MetadataBlock(_)
            | TagEnd::Strikethrough
            | TagEnd::Subscript
            | TagEnd::Superscript
            | TagEnd::Table
            | TagEnd::TableHead
            | TagEnd::TableRow => {}
        }
    }

    fn handle_start_tag(&mut self, tag: Tag<'a>) {
        match tag {
            Tag::Heading { level, .. } => {
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
            Tag::Paragraph => self.push_event_start(Event::StartParagraph {
                alignment: None,
                id: None,
            }),
            Tag::Emphasis => {
                self.italic_depth = self.italic_depth.saturating_add(1);
            }
            Tag::Strong => {
                self.bold_depth = self.bold_depth.saturating_add(1);
            }
            Tag::Image {
                dest_url, title, ..
            } => {
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
            Tag::BlockQuote(_)
            | Tag::CodeBlock(_)
            | Tag::DefinitionList
            | Tag::DefinitionListDefinition
            | Tag::DefinitionListTitle
            | Tag::FootnoteDefinition(_)
            | Tag::HtmlBlock
            | Tag::Item
            | Tag::Link { .. }
            | Tag::List(_)
            | Tag::MetadataBlock(_)
            | Tag::Strikethrough
            | Tag::Subscript
            | Tag::Superscript
            | Tag::Table(_)
            | Tag::TableCell
            | Tag::TableHead
            | Tag::TableRow => {}
        }
    }

    fn handle_text(&mut self, content: String) {
        if let Some(img) = &mut self.image {
            img.alt_buf.push_str(&content);
        } else {
            if self.block_state == BlockState::None {
                self.queue.push_back(Event::StartParagraph {
                    alignment: None,
                    id: None,
                });
                self.block_state = BlockState::AutoParagraph;
            }
            self.queue.push_back(Event::Text {
                content,
                bold: self.bold_depth > 0,
                italic: self.italic_depth > 0,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
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
        let options = Options::ENABLE_TABLES;
        let parser = Parser::new_ext(markdown, options);
        Self {
            block_state: BlockState::None,
            bold_depth: 0,
            image: None,
            italic_depth: 0,
            parser,
            phase: Phase::NotStarted,
            queue: VecDeque::new(),
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
                    self.queue.push_back(Event::LineBreak);
                }
            }
            pulldown_cmark::Event::SoftBreak => {
                if let Some(img) = &mut self.image {
                    img.alt_buf.push(' ');
                } else {
                    self.handle_text(" ".to_string());
                }
            }
            pulldown_cmark::Event::Rule => {
                self.queue.push_back(Event::ThematicBreak);
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
