#![forbid(unsafe_code)]

//! Streaming Pandoc native (block-list) writer for `DocSpec` events.
//!
//! **Experimental.** This writer covers paragraphs, headings, text styles, code blocks
//! and breaks. Lists, tables, links and images are silently dropped. Do not rely on it
//! where fidelity matters.

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

/// Code whose string literal has been opened in the output and is being
/// streamed into. Only the escaper's one-character lookahead is held, never the
/// code itself.
#[derive(Debug)]
enum CodeFrame {
    /// A `CodeBlock` opened by `StartPreformatted`.
    Block { literal: escape::HaskellString },
    /// An inline `Code` opened by a `Code` text style. `nested_style_depth`
    /// counts text styles opened inside it, which are absorbed into the code.
    Inline {
        literal: escape::HaskellString,
        nested_style_depth: u32,
    },
}

impl CodeFrame {
    /// The open string literal, whichever kind of code this is.
    #[inline]
    const fn literal_mut(&mut self) -> &mut escape::HaskellString {
        match self {
            Self::Block { literal } | Self::Inline { literal, .. } => literal,
        }
    }
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
    code_frame: Option<CodeFrame>,
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
            code_frame: None,
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
    fn write_attr(&mut self, id: Option<&str>, classes: &[&str]) -> Result<()> {
        self.writer.write_all(b"(")?;
        escape::write_haskell_string(&mut self.writer, id.unwrap_or(""))?;
        self.writer.write_all(b",[")?;
        for (i, class) in classes.iter().enumerate() {
            if i > 0 {
                self.writer.write_all(b",")?;
            }
            escape::write_haskell_string(&mut self.writer, class)?;
        }
        self.writer.write_all(b"],[])")?;
        Ok(())
    }

    /// Escapes `content` straight into the open code string literal.
    #[inline]
    fn write_code_text(&mut self, content: &str) -> Result<()> {
        if let Some(frame) = self.code_frame.as_mut() {
            frame.literal_mut().write(&mut self.writer, content)?;
        }
        Ok(())
    }

    #[inline]
    fn handle_event_in_code_frame(&mut self, event: Event) -> Result<()> {
        match event {
            Event::Text { content } => self.write_code_text(&content)?,
            // A break inside code is part of the code string. In a code block both
            // kinds end a line, as in the BlockNote writer. In inline code a hard
            // break is a newline (Pandoc's DOCX reader does the same) and a soft
            // break is a space (CommonMark §6.1: line endings in code spans).
            Event::LineBreak => self.write_code_text("\n")?,
            Event::SoftBreak => {
                let separator = if matches!(&self.code_frame, Some(CodeFrame::Inline { .. })) {
                    " "
                } else {
                    "\n"
                };
                self.write_code_text(separator)?;
            }
            Event::StartTextStyle { .. } => {
                if let Some(CodeFrame::Inline {
                    nested_style_depth, ..
                }) = self.code_frame.as_mut()
                {
                    *nested_style_depth = nested_style_depth.saturating_add(1);
                }
            }
            Event::EndTextStyle => match self.code_frame.as_mut() {
                Some(CodeFrame::Inline {
                    nested_style_depth, ..
                }) if *nested_style_depth > 0 => {
                    *nested_style_depth = nested_style_depth.saturating_sub(1);
                }
                Some(CodeFrame::Inline { .. }) => self.close_code_frame()?,
                _ => {}
            },
            Event::EndPreformatted if matches!(&self.code_frame, Some(CodeFrame::Block { .. })) => {
                self.close_code_frame()?;
            }
            _ => {}
        }
        Ok(())
    }

    #[inline]
    fn handle_start_text_style(&mut self, kind: &TextStyleKind, id: Option<&str>) -> Result<()> {
        if matches!(kind, TextStyleKind::Code) {
            self.write_inline_separator()?;
            self.writer.write_all(b"Code ")?;
            self.write_attr(id, &[])?;
            self.writer.write_all(b" ")?;
            let literal = escape::HaskellString::open(&mut self.writer)?;
            self.code_frame = Some(CodeFrame::Inline {
                literal,
                nested_style_depth: 0,
            });
            return Ok(());
        }
        let opener: Option<&[u8]> = match kind {
            TextStyleKind::Bold => Some(b"Strong ["),
            TextStyleKind::Italic => Some(b"Emph ["),
            TextStyleKind::Strikethrough => Some(b"Strikeout ["),
            TextStyleKind::Underline => Some(b"Underline ["),
            TextStyleKind::Subscript => Some(b"Subscript ["),
            TextStyleKind::Superscript => Some(b"Superscript ["),
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
        Ok(())
    }

    /// Whether a new top-level block may open: the document is open and no
    /// paragraph or heading is in progress.
    #[inline]
    fn can_start_block(&self) -> bool {
        self.started && !self.finished && self.inline_block.is_none()
    }

    /// Writes the `,` between top-level blocks, except before the first one.
    #[inline]
    fn write_block_separator(&mut self) -> Result<()> {
        if !self.first_block {
            self.writer.write_all(b",")?;
        }
        self.first_block = false;
        Ok(())
    }

    /// Closes the open paragraph or heading: closes open inline code, closes
    /// any orphaned text-style wrappers, and writes the block's closing `]`.
    #[inline]
    fn close_inline_block(&mut self) -> Result<()> {
        if matches!(&self.code_frame, Some(CodeFrame::Inline { .. })) {
            self.close_code_frame()?;
        }
        self.close_orphaned_inline_frames()?;
        self.writer.write_all(b"]")?;
        self.inline_has_content.pop();
        self.inline_block = None;
        Ok(())
    }

    #[inline]
    fn handle_end_document(&mut self) -> Result<()> {
        if matches!(&self.code_frame, Some(CodeFrame::Inline { .. })) {
            self.close_code_frame()?;
        }
        while !self.inline_has_content.is_empty() {
            self.writer.write_all(b"]")?;
            self.inline_has_content.pop();
        }
        self.text_style_frames.clear();
        self.inline_block = None;
        self.close_code_frame()?;
        self.writer.write_all(b"]")?;
        self.finished = true;
        Ok(())
    }

    #[inline]
    fn handle_start_preformatted(&mut self, id: Option<&str>, syntax: Option<&str>) -> Result<()> {
        self.write_block_separator()?;
        self.writer.write_all(b"CodeBlock ")?;
        match syntax {
            Some(s) => self.write_attr(id, &[s])?,
            None => self.write_attr(id, &[])?,
        }
        self.writer.write_all(b" ")?;
        let literal = escape::HaskellString::open(&mut self.writer)?;
        self.code_frame = Some(CodeFrame::Block { literal });
        Ok(())
    }

    /// Writes the closing `"` of the open code string, if any, and leaves code.
    #[inline]
    fn close_code_frame(&mut self) -> Result<()> {
        if let Some(CodeFrame::Block { literal } | CodeFrame::Inline { literal, .. }) =
            self.code_frame.take()
        {
            literal.close(&mut self.writer)?;
        }
        Ok(())
    }

    #[inline]
    fn write_block_literal(&mut self, token: &[u8]) -> Result<()> {
        if self.can_start_block() {
            self.write_block_separator()?;
            self.writer.write_all(token)?;
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
            self.handle_end_document()?;
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
    /// - `StartTextStyle` / `EndTextStyle` — `Strong`/`Emph`/`Strikeout`/`Underline`/`Subscript`/`Superscript` wrappers,
    ///   and `Code Attr Text` for the `Code` kind (kinds `Mark`, `TextColor` are silently flattened)
    /// - `StartPreformatted` / `EndPreformatted` — `CodeBlock ("id",["syntax"],[]) "..."`
    ///
    /// Inside a code block, `LineBreak` and `SoftBreak` each add a newline to the
    /// code string. Inside inline code, `LineBreak` adds a newline and `SoftBreak`
    /// a space.
    ///
    /// All other events are silently ignored.
    #[inline]
    fn handle_event(&mut self, event: Event) -> Result<()> {
        if self.code_frame.is_some()
            && !matches!(
                event,
                Event::EndParagraph | Event::EndHeading | Event::EndDocument
            )
        {
            return self.handle_event_in_code_frame(event);
        }
        match event {
            Event::StartDocument { .. } if !self.started && !self.finished => {
                self.writer.write_all(b"[")?;
                self.started = true;
            }
            Event::EndDocument if self.started && !self.finished => {
                self.handle_end_document()?;
            }
            Event::StartParagraph { .. } if self.can_start_block() => {
                self.write_block_separator()?;
                self.writer.write_all(b"Para [")?;
                self.inline_block = Some(InlineBlockKind::Paragraph);
                self.inline_has_content.push(false);
            }
            Event::StartHeading { id, level } if self.can_start_block() => {
                self.write_block_separator()?;
                write!(self.writer, "Header {level} ")?;
                self.write_attr(id.as_deref(), &[])?;
                self.writer.write_all(b" [")?;
                self.inline_block = Some(InlineBlockKind::Heading);
                self.inline_has_content.push(false);
            }
            Event::EndParagraph if self.inline_block == Some(InlineBlockKind::Paragraph) => {
                self.close_inline_block()?;
            }
            Event::EndHeading if self.inline_block == Some(InlineBlockKind::Heading) => {
                self.close_inline_block()?;
            }
            Event::StartTextStyle { kind, id } if !self.inline_has_content.is_empty() => {
                self.handle_start_text_style(&kind, id.as_deref())?;
            }
            Event::StartPreformatted { id, syntax } if self.can_start_block() => {
                self.handle_start_preformatted(id.as_deref(), syntax.as_deref())?;
            }
            Event::EndTextStyle => match self.text_style_frames.pop() {
                Some(TextStyleFrame::Wrapped) => {
                    self.writer.write_all(b"]")?;
                    self.inline_has_content.pop();
                }
                Some(TextStyleFrame::Flattened) | None => {}
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
