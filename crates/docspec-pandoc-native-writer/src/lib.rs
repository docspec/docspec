#![forbid(unsafe_code)]

//! Streaming Pandoc native (block-list) writer for `DocSpec` events.

mod escape;

use docspec_core::{Event, EventSink, Result};
use std::io::Write;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InlineBlockKind {
    Paragraph,
    Heading,
}

/// A streaming Pandoc native writer for `DocSpec` events.
///
/// Writes compact Pandoc native block-list syntax directly to the underlying
/// `Write` as events arrive. Implements [`EventSink`] for integration with
/// the `DocSpec` pipeline.
///
/// # Output format
///
/// Emits block-list form: `[Para [Str "..."],Para [Str "..."]]`.
/// No `Pandoc (Meta ...)` wrapper. Compact one-line output, no trailing newline.
///
/// # Type Parameters
///
/// * `W` - Any type implementing [`Write`]
#[expect(
    clippy::struct_excessive_bools,
    reason = "PandocNativeWriter uses independent boolean state flags for a direct state machine; the active inline-block kind is already an Option<InlineBlockKind>, but `started`, `finished`, `first_block`, and `inline_block_has_content` track orthogonal one-shot signals where a combined enum would obscure intent"
)]
pub struct PandocNativeWriter<W: Write> {
    finished: bool,
    first_block: bool,
    inline_block: Option<InlineBlockKind>,
    inline_block_has_content: bool,
    started: bool,
    writer: W,
}

impl<W: Write> PandocNativeWriter<W> {
    /// Creates a new `PandocNativeWriter` that writes to the given writer.
    #[inline]
    #[must_use]
    pub fn new(writer: W) -> Self {
        Self {
            finished: false,
            first_block: true,
            inline_block: None,
            inline_block_has_content: false,
            started: false,
            writer,
        }
    }

    #[inline]
    fn write_block_literal(&mut self, token: &[u8]) -> Result<()> {
        if self.started && !self.finished && self.inline_block.is_none() {
            if !self.first_block {
                self.writer.write_all(b",")?;
            }
            self.writer.write_all(token)?;
            self.first_block = false;
        }
        Ok(())
    }

    #[inline]
    fn write_inline_literal(&mut self, token: &[u8]) -> Result<()> {
        if self.inline_block.is_some() {
            if self.inline_block_has_content {
                self.writer.write_all(b",")?;
            }
            self.writer.write_all(token)?;
            self.inline_block_has_content = true;
        }
        Ok(())
    }
}

impl<W: Write> EventSink for PandocNativeWriter<W> {
    /// Finalizes the output.
    ///
    /// If the document was started but not finished (i.e., `EndDocument` was never
    /// received), performs a best-effort close: closes any open paragraph and emits
    /// the closing `]` for the block list.
    #[inline]
    fn finish(mut self) -> Result<()> {
        if self.started && !self.finished {
            if self.inline_block.is_some() {
                self.writer.write_all(b"]")?;
            }
            self.writer.write_all(b"]")?;
        }
        self.writer.flush()?;
        Ok(())
    }

    /// Handles a single `DocSpec` event.
    ///
    /// The following events produce output:
    /// - `StartDocument` / `EndDocument` — block-list framing
    /// - `StartParagraph` / `EndParagraph` — `Para [...]`
    /// - `StartHeading` / `EndHeading` — `Header N ("id",[],[]) [...]` (level passed through raw)
    /// - `Text` — `Str "..."`
    /// - `ThematicBreak` — `HorizontalRule`
    /// - `LineBreak` — `LineBreak`
    /// - `SoftBreak` — `SoftBreak`
    ///
    /// All other events are silently ignored.
    #[inline]
    fn handle_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::StartDocument { .. } => {
                if !self.started && !self.finished {
                    self.writer.write_all(b"[")?;
                    self.started = true;
                }
            }
            Event::EndDocument => {
                if self.started && !self.finished {
                    if self.inline_block.is_some() {
                        self.writer.write_all(b"]")?;
                        self.inline_block = None;
                    }
                    self.writer.write_all(b"]")?;
                    self.finished = true;
                }
            }
            Event::StartParagraph { .. } => {
                if self.started && !self.finished && self.inline_block.is_none() {
                    if !self.first_block {
                        self.writer.write_all(b",")?;
                    }
                    self.writer.write_all(b"Para [")?;
                    self.inline_block = Some(InlineBlockKind::Paragraph);
                    self.inline_block_has_content = false;
                    self.first_block = false;
                }
            }
            Event::StartHeading { id, level } => {
                if self.started && !self.finished && self.inline_block.is_none() {
                    if !self.first_block {
                        self.writer.write_all(b",")?;
                    }
                    write!(self.writer, "Header {level} (")?;
                    escape::write_haskell_string(&mut self.writer, id.as_deref().unwrap_or(""))?;
                    self.writer.write_all(b",[],[]) [")?;
                    self.inline_block = Some(InlineBlockKind::Heading);
                    self.inline_block_has_content = false;
                    self.first_block = false;
                }
            }
            Event::EndParagraph => {
                if self.inline_block == Some(InlineBlockKind::Paragraph) {
                    self.writer.write_all(b"]")?;
                    self.inline_block = None;
                }
            }
            Event::EndHeading => {
                if self.inline_block == Some(InlineBlockKind::Heading) {
                    self.writer.write_all(b"]")?;
                    self.inline_block = None;
                }
            }
            Event::Text { content } if self.inline_block.is_some() => {
                if self.inline_block_has_content {
                    self.writer.write_all(b",")?;
                }
                self.writer.write_all(b"Str ")?;
                escape::write_haskell_string(&mut self.writer, &content)?;
                self.inline_block_has_content = true;
            }
            Event::ThematicBreak { .. } => self.write_block_literal(b"HorizontalRule")?,
            Event::LineBreak => self.write_inline_literal(b"LineBreak")?,
            Event::SoftBreak => self.write_inline_literal(b"SoftBreak")?,
            _ => {}
        }
        Ok(())
    }
}
