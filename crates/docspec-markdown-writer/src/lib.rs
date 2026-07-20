#![forbid(unsafe_code)]

//! Streaming Markdown (`CommonMark`) writer for `DocSpec` events.
//!
//! Converts a `DocSpec` event stream into `CommonMark`-compliant Markdown
//! output, supporting paragraphs and headings with text-only content. All
//! other events (text styles, breaks, images, tables, etc.) are silently
//! dropped.
//!
//! ## Output format
//!
//! - ATX headings: `# ` through `###### ` (levels 1–6; clamped from any u8)
//! - Blank-line separator between blocks (`\n\n` after each non-empty block)
//! - Empty paragraphs produce zero bytes
//! - Empty headings produce only their ATX prefix + `\n\n` (e.g., `## \n\n`)
//! - No document framing: Markdown has no equivalent of `[` / `]`
//!
//! ## Streaming
//!
//! Implements the [`EventSink`] trait and emits output directly to any
//! [`Write`] target as events arrive — no intermediate document
//! representation, constant memory regardless of file size.

use std::io::Write;

use docspec_core::{Event, EventSink, Result};

mod escape;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockKind {
    Paragraph,
    Heading(u8),
}

/// Streaming `CommonMark` writer.
///
/// Accepts a [`Write`] target and implements [`EventSink`] to convert a
/// `DocSpec` event stream to Markdown output byte-by-byte without buffering
/// the document.
///
/// # Usage
///
/// ```rust
/// use docspec_markdown_writer::MarkdownWriter;
/// use docspec_core::{Event, EventSink};
///
/// let mut buf = Vec::<u8>::new();
/// let mut writer = MarkdownWriter::new(&mut buf);
///
/// writer.handle_event(Event::StartDocument { id: None, language: None, metadata: None })?;
/// writer.handle_event(Event::EndDocument)?;
/// writer.finish()?;
///
/// let output = String::from_utf8(buf)?;
/// assert_eq!(output, "");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct MarkdownWriter<W: Write> {
    writer: W,
    started: bool,
    finished: bool,
    in_block: Option<BlockKind>,
    text_in_current_block: bool,
}

impl<W: Write> MarkdownWriter<W> {
    /// Creates a new `MarkdownWriter` that writes to the given writer.
    #[inline]
    #[must_use]
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            started: false,
            finished: false,
            in_block: None,
            text_in_current_block: false,
        }
    }
}

fn write_atx_prefix<W>(writer: &mut W, level: u8) -> Result<()>
where
    W: Write,
{
    let prefix: &[u8] = match level {
        1 => b"# ",
        2 => b"## ",
        3 => b"### ",
        4 => b"#### ",
        5 => b"##### ",
        _ => b"###### ",
    };
    writer.write_all(prefix)?;
    Ok(())
}

impl<W: Write> EventSink for MarkdownWriter<W> {
    /// Finalizes the output by flushing the underlying writer.
    ///
    /// If the document was started but not properly ended and a block is still
    /// open, this method closes the block before flushing — emitting a trailing
    /// `\n\n` if text was written, or the ATX prefix for empty headings.
    ///
    /// Unlike the Pandoc native writer, Markdown has no document framing,
    /// so no closing bytes are emitted for the document itself.
    #[inline]
    fn finish(mut self) -> Result<()> {
        if self.started && !self.finished {
            match self.in_block {
                Some(BlockKind::Paragraph) => {
                    if self.text_in_current_block {
                        self.writer.write_all(b"\n\n")?;
                    }
                }
                Some(BlockKind::Heading(level)) => {
                    if self.text_in_current_block {
                        self.writer.write_all(b"\n\n")?;
                    } else {
                        write_atx_prefix(&mut self.writer, level)?;
                        self.writer.write_all(b"\n\n")?;
                    }
                }
                None => {}
            }
        }
        self.writer.flush()?;
        Ok(())
    }

    /// Handles a single `DocSpec` event.
    ///
    /// The following events produce output or update writer state:
    /// - `StartDocument` — marks the document as started (idempotent; no output)
    /// - `EndDocument` — marks the document as finished (idempotent; no output)
    /// - `StartParagraph` — opens a paragraph block (no output)
    /// - `EndParagraph` — closes a paragraph; emits `\n\n` if text was written
    /// - `StartHeading { level, .. }` — opens a heading block; level is clamped to 1–6
    /// - `EndHeading` — closes a heading; emits `\n\n` (prefix was emitted lazily)
    /// - `Text { content }` — writes escaped text; for headings, emits the ATX
    ///   prefix lazily on the first text event of each heading
    ///
    /// All other events are silently ignored.
    #[inline]
    fn handle_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::StartDocument { .. } => {
                if !self.started && !self.finished {
                    self.started = true;
                }
            }
            Event::EndDocument => {
                if self.started && !self.finished {
                    self.finished = true;
                }
            }
            Event::StartParagraph { .. }
                if self.started && !self.finished && self.in_block.is_none() =>
            {
                self.in_block = Some(BlockKind::Paragraph);
                self.text_in_current_block = false;
            }
            Event::EndParagraph if self.in_block == Some(BlockKind::Paragraph) => {
                if self.text_in_current_block {
                    self.writer.write_all(b"\n\n")?;
                }
                self.in_block = None;
                self.text_in_current_block = false;
            }
            Event::StartHeading { level, .. }
                if self.started && !self.finished && self.in_block.is_none() =>
            {
                let clamped: u8 = level.clamp(1, 6);
                self.in_block = Some(BlockKind::Heading(clamped));
                self.text_in_current_block = false;
            }
            Event::EndHeading if matches!(self.in_block, Some(BlockKind::Heading(_))) => {
                match (self.in_block, self.text_in_current_block) {
                    (Some(BlockKind::Heading(level)), false) => {
                        write_atx_prefix(&mut self.writer, level)?;
                        self.writer.write_all(b"\n\n")?;
                    }
                    _ => {
                        self.writer.write_all(b"\n\n")?;
                    }
                }
                self.in_block = None;
                self.text_in_current_block = false;
            }
            Event::Text { content } if self.in_block.is_some() => {
                if self.text_in_current_block {
                    escape::write_escaped_inline(&mut self.writer, &content)?;
                } else {
                    if let Some(BlockKind::Heading(level)) = self.in_block {
                        write_atx_prefix(&mut self.writer, level)?;
                    }
                    escape::write_escaped_block_start(&mut self.writer, &content)?;
                    self.text_in_current_block = true;
                }
            }
            _ => {}
        }
        Ok(())
    }
}
