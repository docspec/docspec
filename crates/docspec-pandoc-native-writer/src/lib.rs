#![forbid(unsafe_code)]

//! Streaming Pandoc native (block-list) writer for `DocSpec` events.

mod escape;

use docspec_core::{Event, EventSink, Result, TextStyleKind};
use std::io::Write;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InlineBlockKind {
    Paragraph,
    Heading,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextStyleFrame {
    Wrapped,
    Flattened,
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
pub struct PandocNativeWriter<W: Write> {
    finished: bool,
    first_block: bool,
    inline_block: Option<InlineBlockKind>,
    inline_has_content: Vec<bool>,
    started: bool,
    text_style_frames: Vec<TextStyleFrame>,
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
            inline_has_content: Vec::new(),
            started: false,
            text_style_frames: Vec::new(),
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
    fn write_inline_separator(&mut self) -> Result<()> {
        if let Some(has_content) = self.inline_has_content.last_mut() {
            if *has_content {
                self.writer.write_all(b",")?;
            }
            *has_content = true;
        }
        Ok(())
    }

    #[inline]
    fn write_inline_literal(&mut self, token: &[u8]) -> Result<()> {
        if !self.inline_has_content.is_empty() {
            self.write_inline_separator()?;
            self.writer.write_all(token)?;
        }
        Ok(())
    }

    #[inline]
    fn close_orphaned_inline_frames(&mut self) -> Result<()> {
        while self.inline_has_content.len() > 1 {
            self.writer.write_all(b"]")?;
            self.inline_has_content.pop();
        }
        self.text_style_frames.clear();
        Ok(())
    }
}

impl<W: Write> EventSink for PandocNativeWriter<W> {
    /// Finalizes the output.
    ///
    /// If the document was started but not finished (i.e., `EndDocument` was never
    /// received), performs a best-effort close: closes any open inline wrappers and
    /// emits the closing `]` for the block list.
    #[inline]
    fn finish(mut self) -> Result<()> {
        if self.started && !self.finished {
            while !self.inline_has_content.is_empty() {
                self.writer.write_all(b"]")?;
                self.inline_has_content.pop();
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
    /// - `StartTextStyle` / `EndTextStyle` — `Strong`/`Emph`/`Strikeout`/`Underline`/`Subscript`/`Superscript` wrappers
    ///   (kinds `Code`, `Mark`, `TextColor` are silently flattened — text inside is preserved without a wrapper)
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
                    while !self.inline_has_content.is_empty() {
                        self.writer.write_all(b"]")?;
                        self.inline_has_content.pop();
                    }
                    self.text_style_frames.clear();
                    self.inline_block = None;
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
                    self.inline_has_content.push(false);
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
                    self.inline_has_content.push(false);
                    self.first_block = false;
                }
            }
            Event::EndParagraph => {
                if self.inline_block == Some(InlineBlockKind::Paragraph) {
                    self.close_orphaned_inline_frames()?;
                    self.writer.write_all(b"]")?;
                    self.inline_has_content.pop();
                    self.inline_block = None;
                }
            }
            Event::EndHeading => {
                if self.inline_block == Some(InlineBlockKind::Heading) {
                    self.close_orphaned_inline_frames()?;
                    self.writer.write_all(b"]")?;
                    self.inline_has_content.pop();
                    self.inline_block = None;
                }
            }
            Event::StartTextStyle { kind, .. } if !self.inline_has_content.is_empty() => {
                let opener: Option<&[u8]> = match kind {
                    TextStyleKind::Bold => Some(b"Strong ["),
                    TextStyleKind::Italic => Some(b"Emph ["),
                    TextStyleKind::Strikethrough => Some(b"Strikeout ["),
                    TextStyleKind::Underline => Some(b"Underline ["),
                    TextStyleKind::Subscript => Some(b"Subscript ["),
                    TextStyleKind::Superscript => Some(b"Superscript ["),
                    TextStyleKind::Code | TextStyleKind::Mark(_) | TextStyleKind::TextColor(_) => {
                        None
                    }
                    _ => None,
                };
                if let Some(open) = opener {
                    self.write_inline_separator()?;
                    self.writer.write_all(open)?;
                    self.inline_has_content.push(false);
                    self.text_style_frames.push(TextStyleFrame::Wrapped);
                } else {
                    self.text_style_frames.push(TextStyleFrame::Flattened);
                }
            }
            Event::EndTextStyle => match self.text_style_frames.pop() {
                Some(TextStyleFrame::Wrapped) if self.inline_has_content.len() > 1 => {
                    self.writer.write_all(b"]")?;
                    self.inline_has_content.pop();
                }
                Some(TextStyleFrame::Wrapped | TextStyleFrame::Flattened) | None => {}
            },
            Event::Text { content } if !self.inline_has_content.is_empty() => {
                self.write_inline_separator()?;
                self.writer.write_all(b"Str ")?;
                escape::write_haskell_string(&mut self.writer, &content)?;
            }
            Event::ThematicBreak { .. } => self.write_block_literal(b"HorizontalRule")?,
            Event::LineBreak => self.write_inline_literal(b"LineBreak")?,
            Event::SoftBreak => self.write_inline_literal(b"SoftBreak")?,
            _ => {}
        }
        Ok(())
    }
}
