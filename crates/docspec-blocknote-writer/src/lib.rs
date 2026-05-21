//! `DocSpec` event stream to `BlockNote` JSON writer.
//!
//! This crate provides a streaming [`BlockNoteWriter`] that implements [`EventSink`] to convert
//! `DocSpec` event streams into `BlockNote` JSON format. `BlockNote` is a block-based rich text
//! editor format.
//!
//! # Design
//!
//! The writer emits JSON tokens directly to the underlying `Write` as events arrive using
//! `struson` for streaming JSON output. For text and URI-based images, memory usage is constant
//! regardless of document size. Asset-based images (`ImageSource::Asset`) are base64-encoded
//! into an in-memory data URI before writing, so memory scales with individual asset size.
//!
//! # Supported Events
//!
//! - `StartDocument` / `EndDocument` — array start/end
//! - `StartHeading` / `EndHeading` — heading blocks
//! - `StartParagraph` / `EndParagraph` — paragraph blocks
//! - `StartBlockQuote` / `EndBlockQuote` — quote blocks
//! - `StartPreformatted` / `EndPreformatted` — code blocks
//! - `Text` — inline text content with bold/italic/code/strikethrough/underline styles
//! - `Image` — image blocks
//! - `LineBreak` — line breaks within content blocks
//! - `ThematicBreak` — divider blocks
//!
//! List and table structure events (`StartListItem`, `StartTable*`, etc.) are silently ignored
//! by this writer. Use `StackTrackingSink` from `docspec_core` to wrap the writer for automatic
//! paragraph insertion within list items and table cells.
//!
//! # Example
//!
//! ```
//! use docspec_blocknote_writer::BlockNoteWriter;
//! use docspec_core::{Event, EventSink, StackTrackingSink, TextStyle};
//!
//! let mut buf = Vec::<u8>::new();
//! let mut writer = StackTrackingSink::new(BlockNoteWriter::new(&mut buf));
//!
//! writer.handle_event(Event::StartDocument { id: None, language: None, metadata: None })?;
//! writer.handle_event(Event::StartParagraph { alignment: None, id: None })?;
//! writer.handle_event(Event::Text {
//!     content: "Hello".to_string(),
//!     style: TextStyle::default(),
//! })?;
//! writer.handle_event(Event::EndParagraph)?;
//! writer.handle_event(Event::EndDocument)?;
//! writer.finish()?;
//!
//! let json = String::from_utf8(buf)?;
//! assert!(json.starts_with('['));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! [`EventSink`]: docspec_core::EventSink

use std::io::Write;

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::write::EncoderWriter as Base64Encoder;
use docspec_core::{AssetProvider, Error, Event, EventSink, ImageSource, Result, TextStyle};
use struson::writer::{JsonStreamWriter, JsonWriter as _};

/// A streaming `BlockNote` JSON writer.
///
/// Writes JSON tokens directly to the underlying `Write` as events arrive using `struson`.
/// Implements [`EventSink`] for integration with the `DocSpec` pipeline.
///
/// Use [`BlockNoteWriter::with_assets`] to provide an [`AssetProvider`] for resolving
/// embedded asset images as base64 data URIs.
///
/// # Type Parameters
///
/// * `W` - Any type implementing [`Write`]
pub struct BlockNoteWriter<'a, W: Write> {
    /// Optional asset provider for resolving embedded asset references.
    assets: Option<&'a dyn AssetProvider>,
    /// Depth of blockquote nesting (0 = not inside blockquote).
    blockquote_depth: u32,
    /// Count of blockquotes force-closed by sibling emission (`EndBlockQuote` events to ignore).
    blockquote_force_closed_count: usize,
    /// Whether any inline content has been written to the current blockquote's content array.
    blockquote_has_content: bool,
    /// Whether we are inside a text-bearing content block (heading, paragraph, preformatted, or blockquote).
    in_text_block: bool,
    /// The underlying JSON stream writer.
    writer: JsonStreamWriter<W>,
}

struct Null;
struct StartArray;

trait WriteVal {
    fn write_val<W: Write>(self, w: &mut BlockNoteWriter<'_, W>) -> Result<()>;
}

impl WriteVal for Null {
    fn write_val<W: Write>(self, w: &mut BlockNoteWriter<'_, W>) -> Result<()> {
        w.writer.null_value().map_err(io_err)
    }
}

impl WriteVal for StartArray {
    fn write_val<W: Write>(self, w: &mut BlockNoteWriter<'_, W>) -> Result<()> {
        w.writer.begin_array().map_err(io_err)
    }
}

impl WriteVal for &str {
    fn write_val<W: Write>(self, w: &mut BlockNoteWriter<'_, W>) -> Result<()> {
        w.writer.string_value(self).map_err(io_err)
    }
}

impl WriteVal for bool {
    fn write_val<W: Write>(self, w: &mut BlockNoteWriter<'_, W>) -> Result<()> {
        w.writer.bool_value(self).map_err(io_err)
    }
}

impl WriteVal for u8 {
    fn write_val<W: Write>(self, w: &mut BlockNoteWriter<'_, W>) -> Result<()> {
        w.writer.number_value(self).map_err(io_err)
    }
}

impl<'a, W: Write> BlockNoteWriter<'a, W> {
    fn array<F>(&mut self, key: &str, f: F) -> Result<()>
    where
        F: FnOnce(&mut Self) -> Result<()>,
    {
        self.entry(key, StartArray)?;
        f(self)?;
        self.end_array()
    }

    fn begin_array(&mut self) -> Result<()> {
        self.writer.begin_array().map_err(io_err)
    }

    fn begin_object(&mut self) -> Result<()> {
        self.writer.begin_object().map_err(io_err)
    }

    fn block<F>(&mut self, f: F) -> Result<()>
    where
        F: FnOnce(&mut Self) -> Result<()>,
    {
        self.begin_object()?;
        f(self)?;
        self.end_object()
    }

    fn close_blockquote_for_sibling(&mut self) -> Result<()> {
        self.close_content_block()?;
        self.blockquote_depth = self.blockquote_depth.saturating_sub(1);
        self.blockquote_force_closed_count = self.blockquote_force_closed_count.saturating_add(1);
        self.in_text_block = self.blockquote_depth > 0;
        Ok(())
    }

    fn close_content_block(&mut self) -> Result<()> {
        self.end_array()?;
        self.array("children", |_| Ok(()))?;
        self.end_object()
    }

    fn close_for_block_sibling(&mut self) -> Result<()> {
        if self.blockquote_depth > 0 {
            return self.close_blockquote_for_sibling();
        }
        if self.in_text_block {
            self.close_content_block()?;
            self.in_text_block = false;
        }
        Ok(())
    }

    fn end_array(&mut self) -> Result<()> {
        self.writer.end_array().map_err(io_err)
    }

    fn end_object(&mut self) -> Result<()> {
        self.writer.end_object().map_err(io_err)
    }

    fn entry<V: WriteVal>(&mut self, key: &str, value: V) -> Result<()> {
        self.writer.name(key).map_err(io_err)?;
        value.write_val(self)
    }

    fn handle_blockquote(&mut self, id: Option<&String>) -> Result<()> {
        self.begin_object()?;
        self.entry("type", "quote")?;
        self.write_id(id)?;
        self.entry("content", StartArray)?;
        self.blockquote_depth = self.blockquote_depth.saturating_add(1);
        self.blockquote_has_content = false;
        self.in_text_block = true;
        Ok(())
    }

    fn handle_divider(&mut self, id: Option<&String>) -> Result<()> {
        self.block(|w| {
            w.entry("type", "divider")?;
            w.write_id(id)
        })
    }

    fn handle_heading(&mut self, level: u8, id: Option<&String>) -> Result<()> {
        self.begin_object()?;
        self.entry("type", "heading")?;
        self.write_id(id)?;
        self.object("props", |w| {
            w.entry("level", level)?;
            w.entry("textAlignment", "left")
        })?;
        self.entry("content", StartArray)?;
        self.in_text_block = true;
        Ok(())
    }

    fn handle_image(
        &mut self,
        source: ImageSource,
        alt: Option<String>,
        id: Option<&String>,
    ) -> Result<()> {
        self.close_for_block_sibling()?;
        let url = match source {
            ImageSource::Uri { uri } => uri,
            ImageSource::Asset { asset_id } => {
                let provider = self.assets.ok_or_else(|| Error::Other {
                    message: "no AssetProvider configured".to_string(),
                })?;
                let content_type =
                    provider
                        .content_type(&asset_id)
                        .ok_or_else(|| Error::Other {
                            message: format!("asset not found: {asset_id}"),
                        })?;
                let prefix = format!("data:{content_type};base64,");
                let mut data_uri = Vec::with_capacity(prefix.len());
                data_uri.extend_from_slice(prefix.as_bytes());
                {
                    let mut enc = Base64Encoder::new(&mut data_uri, &BASE64_STANDARD);
                    provider
                        .stream_to(&asset_id, &mut enc)
                        .ok_or_else(|| Error::Other {
                            message: format!("asset not found: {asset_id}"),
                        })?
                        .map_err(|e| Error::Io { source: e })?;
                    enc.finish().map_err(|e| Error::Io { source: e })?
                };
                String::from_utf8(data_uri).map_err(|e| Error::Other {
                    message: format!("base64 encoding produced invalid UTF-8: {e}"),
                })?
            }
        };
        let caption = alt.unwrap_or_default();

        self.block(|w| {
            w.write_id(id)?;
            w.entry("type", "image")?;
            w.object("props", |p| {
                p.entry("url", url.as_str())?;
                p.entry("caption", caption.as_str())
            })?;
            w.entry("content", Null)?;
            w.array("children", |_| Ok(()))
        })
    }

    fn handle_paragraph(&mut self, id: Option<&String>) -> Result<()> {
        if self.blockquote_depth > 0 {
            if self.blockquote_has_content {
                self.handle_text("\n\n", &TextStyle::default())?;
            }
            return Ok(());
        }
        self.begin_object()?;
        self.write_id(id)?;
        self.entry("type", "paragraph")?;
        self.object("props", |w| w.entry("textAlignment", "left"))?;
        self.entry("content", StartArray)?;
        self.in_text_block = true;
        Ok(())
    }

    fn handle_preformatted(&mut self, id: Option<&String>, syntax: Option<&String>) -> Result<()> {
        self.begin_object()?;
        self.entry("type", "codeBlock")?;
        self.write_id(id)?;
        if let Some(lang) = syntax {
            self.object("props", |w| w.entry("language", lang.as_str()))?;
        }
        self.entry("content", StartArray)?;
        self.in_text_block = true;
        Ok(())
    }

    fn handle_text(&mut self, content: &str, style: &TextStyle) -> Result<()> {
        if !self.in_text_block {
            return Ok(());
        }
        if self.blockquote_depth > 0 {
            self.blockquote_has_content = true;
        }
        self.block(|w| {
            w.entry("type", "text")?;
            w.entry("text", content)?;
            w.object("styles", |s| {
                for (key, enabled) in [
                    ("bold", style.bold),
                    ("italic", style.italic),
                    ("code", style.code),
                    ("strike", style.strikethrough),
                    ("underline", style.underline),
                ] {
                    if enabled {
                        s.entry(key, true)?;
                    }
                }
                Ok(())
            })
        })
    }

    /// Creates a new `BlockNoteWriter` that writes to the given writer.
    ///
    /// # Arguments
    ///
    /// * `writer` - The underlying writer to emit JSON to
    #[inline]
    #[must_use]
    pub fn new(writer: W) -> Self {
        Self {
            assets: None,
            blockquote_depth: 0,
            blockquote_force_closed_count: 0,
            blockquote_has_content: false,
            in_text_block: false,
            writer: JsonStreamWriter::new(writer),
        }
    }

    fn object<F>(&mut self, key: &str, f: F) -> Result<()>
    where
        F: FnOnce(&mut Self) -> Result<()>,
    {
        self.writer.name(key).map_err(io_err)?;
        self.begin_object()?;
        f(self)?;
        self.end_object()
    }

    /// Creates a new `BlockNoteWriter` with an [`AssetProvider`] for resolving embedded assets.
    ///
    /// When an [`Event::Image`] with [`ImageSource::Asset`] is encountered, the provider is called
    /// to resolve the asset bytes. The bytes are base64-encoded and written as a data URI
    /// (`data:{content_type};base64,{encoded}`) in the `BlockNote` JSON `url` field.
    ///
    /// # Arguments
    ///
    /// * `writer` - The underlying writer to emit JSON to
    /// * `assets` - The asset provider for resolving embedded asset references
    #[inline]
    #[must_use]
    pub fn with_assets(writer: W, assets: &'a dyn AssetProvider) -> Self {
        Self {
            assets: Some(assets),
            blockquote_depth: 0,
            blockquote_force_closed_count: 0,
            blockquote_has_content: false,
            in_text_block: false,
            writer: JsonStreamWriter::new(writer),
        }
    }

    fn write_id(&mut self, id: Option<&String>) -> Result<()> {
        if let Some(id_val) = id {
            self.entry("id", id_val.as_str())?;
        }
        Ok(())
    }
}

impl<W: Write> EventSink for BlockNoteWriter<'_, W> {
    #[inline]
    fn finish(self) -> Result<()> {
        self.writer.finish_document().map_err(io_err)?;
        Ok(())
    }

    #[inline]
    fn handle_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::StartDocument { .. } => self.begin_array(),
            Event::EndDocument => self.end_array(),
            Event::StartHeading { level, id, .. } => {
                self.close_for_block_sibling()?;
                self.handle_heading(level, id.as_ref())
            }
            Event::EndHeading | Event::EndPreformatted => {
                if !self.in_text_block {
                    return Ok(());
                }
                self.close_content_block()?;
                self.in_text_block = false;
                Ok(())
            }
            Event::StartParagraph { id, .. } => self.handle_paragraph(id.as_ref()),
            Event::EndParagraph => {
                if self.blockquote_depth > 0 || !self.in_text_block {
                    return Ok(());
                }
                self.close_content_block()?;
                self.in_text_block = false;
                Ok(())
            }
            Event::StartBlockQuote { id, .. } => {
                self.close_for_block_sibling()?;
                self.handle_blockquote(id.as_ref())
            }
            Event::EndBlockQuote => {
                if self.blockquote_force_closed_count > 0 {
                    self.blockquote_force_closed_count =
                        self.blockquote_force_closed_count.saturating_sub(1);
                    return Ok(());
                }
                self.close_content_block()?;
                self.blockquote_depth = self.blockquote_depth.saturating_sub(1);
                self.in_text_block = self.blockquote_depth > 0;
                Ok(())
            }
            Event::StartPreformatted { id, syntax, .. } => {
                self.close_for_block_sibling()?;
                self.handle_preformatted(id.as_ref(), syntax.as_ref())
            }
            Event::ThematicBreak { id, .. } => {
                self.close_for_block_sibling()?;
                self.handle_divider(id.as_ref())
            }
            Event::Text { content, style, .. } => {
                // Auto-open paragraph for orphan text (e.g., text after image closed paragraph)
                if !self.in_text_block && self.blockquote_depth == 0 {
                    self.handle_paragraph(None)?;
                }
                self.handle_text(&content, &style)
            }
            Event::Image {
                source, alt, id, ..
            } => self.handle_image(source, alt, id.as_ref()),
            Event::LineBreak => {
                if self.in_text_block {
                    self.handle_text("\n", &TextStyle::default())
                } else {
                    Ok(())
                }
            }
            Event::EndCaption
            | Event::EndDefinitionTerm
            | Event::EndLink
            | Event::EndDefinitionDetail
            | Event::EndDefinitionList
            | Event::EndFootnote
            | Event::EndListItem
            | Event::EndTable
            | Event::EndTableCell
            | Event::EndTableHeader
            | Event::EndTableRow
            | Event::FootnoteRef { .. }
            | Event::StartCaption { .. }
            | Event::StartDefinitionDetail { .. }
            | Event::StartDefinitionList { .. }
            | Event::StartDefinitionTerm { .. }
            | Event::StartFootnote { .. }
            | Event::StartLink { .. }
            | Event::StartListItem { .. }
            | Event::StartTable { .. }
            | Event::StartTableCell { .. }
            | Event::StartTableHeader { .. }
            | Event::StartTableRow { .. }
            | _ => Ok(()),
        }
    }
}

/// Maps a struson I/O error to a docspec error.
fn io_err(e: std::io::Error) -> Error {
    Error::Io { source: e }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn io_err_maps_correctly() {
        let io_error = std::io::Error::other("test");
        let docspec_error = super::io_err(io_error);
        assert!(matches!(docspec_error, Error::Io { .. }));
    }
}
