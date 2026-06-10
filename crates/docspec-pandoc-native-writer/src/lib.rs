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

#[derive(Debug)]
enum CodeFrame {
    Block {
        id: Option<String>,
        syntax: Option<String>,
        buffer: String,
    },
    Inline {
        id: Option<String>,
        buffer: String,
        nested_style_depth: u32,
    },
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
    fn write_attr(&mut self, id: &str, classes: &[&str]) -> Result<()> {
        self.writer.write_all(b"(")?;
        escape::write_haskell_string(&mut self.writer, id)?;
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

    #[inline]
    fn handle_event_in_code_frame(&mut self, event: Event) -> Result<()> {
        match event {
            Event::Text { content } => {
                if let Some(frame) = self.code_frame.as_mut() {
                    let buffer = match frame {
                        CodeFrame::Block { buffer, .. } | CodeFrame::Inline { buffer, .. } => {
                            buffer
                        }
                    };
                    buffer.push_str(&content);
                }
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
                Some(CodeFrame::Inline { .. }) => self.flush_code_frame()?,
                _ => {}
            },
            Event::EndPreformatted if matches!(&self.code_frame, Some(CodeFrame::Block { .. })) => {
                self.flush_code_frame()?;
            }
            _ => {}
        }
        Ok(())
    }

    #[inline]
    fn handle_start_text_style(&mut self, kind: &TextStyleKind, id: Option<String>) -> Result<()> {
        if matches!(kind, TextStyleKind::Code) {
            self.code_frame = Some(CodeFrame::Inline {
                id,
                buffer: String::new(),
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

    #[inline]
    fn handle_end_document(&mut self) -> Result<()> {
        if matches!(&self.code_frame, Some(CodeFrame::Inline { .. })) {
            self.flush_code_frame()?;
        }
        while !self.inline_has_content.is_empty() {
            self.writer.write_all(b"]")?;
            self.inline_has_content.pop();
        }
        self.text_style_frames.clear();
        self.inline_block = None;
        if self.code_frame.is_some() {
            self.flush_code_frame()?;
        }
        self.writer.write_all(b"]")?;
        self.finished = true;
        Ok(())
    }

    #[inline]
    fn handle_start_preformatted(
        &mut self,
        id: Option<String>,
        syntax: Option<String>,
    ) -> Result<()> {
        if !self.first_block {
            self.writer.write_all(b",")?;
        }
        self.code_frame = Some(CodeFrame::Block {
            id,
            syntax,
            buffer: String::new(),
        });
        self.first_block = false;
        Ok(())
    }

    #[inline]
    fn flush_code_frame(&mut self) -> Result<()> {
        match self.code_frame.take() {
            Some(CodeFrame::Block { id, syntax, buffer }) => {
                self.writer.write_all(b"CodeBlock ")?;
                match syntax.as_deref() {
                    Some(s) => self.write_attr(id.as_deref().unwrap_or(""), &[s])?,
                    None => self.write_attr(id.as_deref().unwrap_or(""), &[])?,
                }
                self.writer.write_all(b" ")?;
                escape::write_haskell_string(&mut self.writer, &buffer)?;
            }
            Some(CodeFrame::Inline { id, buffer, .. }) => {
                self.write_inline_separator()?;
                self.writer.write_all(b"Code ")?;
                self.write_attr(id.as_deref().unwrap_or(""), &[])?;
                self.writer.write_all(b" ")?;
                escape::write_haskell_string(&mut self.writer, &buffer)?;
            }
            None => {}
        }
        Ok(())
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
            if matches!(&self.code_frame, Some(CodeFrame::Inline { .. })) {
                self.flush_code_frame()?;
            }
            while !self.inline_has_content.is_empty() {
                self.writer.write_all(b"]")?;
                self.inline_has_content.pop();
            }
            if self.code_frame.is_some() {
                self.flush_code_frame()?;
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
    /// - `StartTextStyle` / `EndTextStyle` — `Strong`/`Emph`/`Strikeout`/`Underline`/`Subscript`/`Superscript` wrappers,
    ///   and `Code Attr Text` for the `Code` kind (kinds `Mark`, `TextColor` are silently flattened)
    /// - `StartPreformatted` / `EndPreformatted` — `CodeBlock ("id",["syntax"],[]) "..."`
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
            Event::StartDocument { .. } => {
                if !self.started && !self.finished {
                    self.writer.write_all(b"[")?;
                    self.started = true;
                }
            }
            Event::EndDocument => {
                if self.started && !self.finished {
                    self.handle_end_document()?;
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
                    if matches!(&self.code_frame, Some(CodeFrame::Inline { .. })) {
                        self.flush_code_frame()?;
                    }
                    self.close_orphaned_inline_frames()?;
                    self.writer.write_all(b"]")?;
                    self.inline_has_content.pop();
                    self.inline_block = None;
                }
            }
            Event::EndHeading => {
                if self.inline_block == Some(InlineBlockKind::Heading) {
                    if matches!(&self.code_frame, Some(CodeFrame::Inline { .. })) {
                        self.flush_code_frame()?;
                    }
                    self.close_orphaned_inline_frames()?;
                    self.writer.write_all(b"]")?;
                    self.inline_has_content.pop();
                    self.inline_block = None;
                }
            }
            Event::StartTextStyle { kind, id } if !self.inline_has_content.is_empty() => {
                self.handle_start_text_style(&kind, id)?;
            }
            Event::StartPreformatted { id, syntax }
                if self.started && !self.finished && self.inline_block.is_none() =>
            {
                self.handle_start_preformatted(id, syntax)?;
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
