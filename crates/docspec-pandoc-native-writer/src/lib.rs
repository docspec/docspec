#![forbid(unsafe_code)]

//! Streaming Pandoc native (block-list) writer for `DocSpec` events.

mod escape;

use docspec_core::{Event, EventSink, Result};
use std::io::Write;

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
    reason = "PandocNativeWriter uses one boolean per state flag for a direct state machine; a separate state enum would be more complex without benefit"
)]
pub struct PandocNativeWriter<W: Write> {
    finished: bool,
    first_block: bool,
    in_paragraph: bool,
    paragraph_has_inline: bool,
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
            in_paragraph: false,
            paragraph_has_inline: false,
            started: false,
            writer,
        }
    }

    #[inline]
    fn write_block_literal(&mut self, token: &[u8]) -> Result<()> {
        if self.started && !self.finished && !self.in_paragraph {
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
        if self.in_paragraph {
            if self.paragraph_has_inline {
                self.writer.write_all(b",")?;
            }
            self.writer.write_all(token)?;
            self.paragraph_has_inline = true;
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
            if self.in_paragraph {
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
                    if self.in_paragraph {
                        self.writer.write_all(b"]")?;
                        self.in_paragraph = false;
                    }
                    self.writer.write_all(b"]")?;
                    self.finished = true;
                }
            }
            Event::StartParagraph { .. } => {
                if self.started && !self.finished && !self.in_paragraph {
                    if !self.first_block {
                        self.writer.write_all(b",")?;
                    }
                    self.writer.write_all(b"Para [")?;
                    self.in_paragraph = true;
                    self.paragraph_has_inline = false;
                    self.first_block = false;
                }
            }
            Event::EndParagraph => {
                if self.in_paragraph {
                    self.writer.write_all(b"]")?;
                    self.in_paragraph = false;
                }
            }
            Event::Text { content } if self.in_paragraph => {
                if self.paragraph_has_inline {
                    self.writer.write_all(b",")?;
                }
                self.writer.write_all(b"Str ")?;
                escape::write_haskell_string(&mut self.writer, &content)?;
                self.paragraph_has_inline = true;
            }
            Event::ThematicBreak { .. } => self.write_block_literal(b"HorizontalRule")?,
            Event::LineBreak => self.write_inline_literal(b"LineBreak")?,
            Event::SoftBreak => self.write_inline_literal(b"SoftBreak")?,
            _ => {}
        }
        Ok(())
    }
}
