//! `DocSpec` event stream to `BlockNote` JSON writer.
//!
//! This crate provides a streaming [`BlockNoteWriter`] that implements [`EventSink`] to convert
//! `DocSpec` event streams into `BlockNote` JSON format. `BlockNote` is a block-based rich text
//! editor format.
//!
//! # Design
//!
//! The writer emits JSON tokens directly to the underlying `Write` as events arrive using
//! `docspec-json` for streaming JSON output. For text and URI-based images, memory usage is
//! constant regardless of document size. Asset-based images (`ImageSource::Asset`) are
//! base64-encoded into an in-memory data URI before writing, so memory scales with individual
//! asset size.
//!
//! # Supported Events
//!
//! - `StartDocument` / `EndDocument` — array start/end
//! - `StartHeading` / `EndHeading` — heading blocks
//! - `StartParagraph` / `EndParagraph` — paragraph blocks
//! - `StartBlockQuote` / `EndBlockQuote` — quote blocks
//! - `StartPreformatted` / `EndPreformatted` — code blocks
//! - `StartTable` / `EndTable` — table blocks
//! - `StartTableRow` / `EndTableRow` — table rows
//! - `StartTableCell` / `EndTableCell` — table cells (data)
//! - `StartTableHeader` / `EndTableHeader` — table cells (header, emitted identically to data cells)
//! - `Text` — inline text content with bold/italic/code/strikethrough/underline styles
//! - `Image` — image blocks
//! - `LineBreak` — line breaks within content blocks
//! - `ThematicBreak` — divider blocks
//!
//! List structure events (`StartOrderedListItem`, `StartUnorderedListItem`, etc.) are silently ignored
//! by this writer. Use `StackTrackingSink` from `docspec_core` to wrap the writer for automatic
//! paragraph insertion within list items. As a consequence, list items are emitted as `paragraph`
//! blocks and list structure (ordered/unordered nesting) is lost.
//!
//! # Table Cell Content Semantics
//!
//! `BlockNote`'s `tableCell.content` is `InlineContent[]` — it cannot hold block-level types.
//! `EVENTS.md` declares that `DocSpec` cells may contain any block element, so this writer
//! flattens block-level events that appear inside a cell:
//!
//! - **Preserved**: [`Text`](docspec_core::Event::Text) (with all inline styles), [`LineBreak`](docspec_core::Event::LineBreak)
//! - **Absorbed silently**: [`StartParagraph`](docspec_core::Event::StartParagraph) / [`EndParagraph`](docspec_core::Event::EndParagraph) (paragraph boundaries are dropped — adjacent paragraphs concatenate without separator)
//! - **Dropped**: [`Image`](docspec_core::Event::Image), [`StartBlockQuote`](docspec_core::Event::StartBlockQuote), [`StartPreformatted`](docspec_core::Event::StartPreformatted), [`StartHeading`](docspec_core::Event::StartHeading), [`ThematicBreak`](docspec_core::Event::ThematicBreak), nested [`StartTable`](docspec_core::Event::StartTable) and their children — silently discarded
//!
//! Nested tables (a `StartTable` inside a cell) are entirely dropped: their rows, cells, text,
//! and closing events are all absorbed. Only the outer table is emitted. The current markdown
//! reader never produces multi-block cell content or nested tables — these guards exist for
//! future DOCX/ODT readers.
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
use docspec_json::{JsonEmitter, Null, StrusonBackend};

macro_rules! close_text_block {
    ($writer:expr) => {{
        $writer.close_content_block()?;
        $writer.in_text_block = false;
        Ok(())
    }};
}

macro_rules! return_if_table_cell {
    ($writer:expr) => {
        if $writer.in_table_cell {
            return Ok(());
        }
    };
}

/// A streaming `BlockNote` JSON writer.
///
/// Writes JSON tokens directly to the underlying `Write` as events arrive using `docspec-json`.
/// Implements [`EventSink`] for integration with the `DocSpec` pipeline.
///
/// Use [`BlockNoteWriter::with_assets`] to provide an [`AssetProvider`] for resolving
/// embedded asset images as base64 data URIs.
///
/// # Type Parameters
///
/// * `W` - Any type implementing [`Write`]
pub struct BlockNoteWriter<'a, W: Write> {
    assets: Option<&'a dyn AssetProvider>,
    blockquote_depth: u32,
    blockquote_force_closed_count: usize,
    blockquote_has_content: bool,
    in_table_cell: bool,
    in_text_block: bool,
    json: JsonEmitter<StrusonBackend<W>>,
    table_depth: u32,
}

impl<'a, W: Write> BlockNoteWriter<'a, W> {
    fn close_blockquote_for_sibling(&mut self) -> Result<()> {
        self.close_content_block()?;
        self.blockquote_depth = self.blockquote_depth.saturating_sub(1);
        self.blockquote_force_closed_count = self.blockquote_force_closed_count.saturating_add(1);
        self.in_text_block = self.blockquote_depth > 0;
        Ok(())
    }

    fn close_content_block(&mut self) -> Result<()> {
        self.json.close_array()?;
        self.json.key("children").array(|_| Ok(()))?;
        self.json.close_object()
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

    /// Resolves `asset_id` through the configured provider and encodes the asset bytes
    /// as a `data:<content-type>;base64,…` URI.
    ///
    /// Returns `Err` if no `AssetProvider` is configured, the asset cannot be found,
    /// or the underlying I/O fails while streaming the asset bytes through the
    /// base64 encoder.
    fn encode_asset_as_data_uri(&self, asset_id: &str) -> Result<String> {
        let provider = self.assets.ok_or_else(|| Error::Other {
            message: "no AssetProvider configured".to_string(),
        })?;
        let content_type = provider
            .content_type(asset_id)
            .ok_or_else(|| Error::Other {
                message: format!("asset not found: {asset_id}"),
            })?;
        let prefix = format!("data:{content_type};base64,");
        let mut data_uri = Vec::with_capacity(prefix.len());
        data_uri.extend_from_slice(prefix.as_bytes());
        {
            let mut enc = Base64Encoder::new(&mut data_uri, &BASE64_STANDARD);
            provider
                .stream_to(asset_id, &mut enc)
                .ok_or_else(|| Error::Other {
                    message: format!("asset not found: {asset_id}"),
                })?
                .map_err(io_err)?;
            enc.finish().map_err(io_err)?
        };
        String::from_utf8(data_uri).map_err(|e| Error::Other {
            message: format!("base64 encoding produced invalid UTF-8: {e}"),
        })
    }

    fn handle_blockquote(&mut self, id: Option<&String>) -> Result<()> {
        self.json.open_object()?;
        self.json.key("type").value("quote")?;
        self.write_id(id)?;
        self.json.key("content").open_array()?;
        self.blockquote_depth = self.blockquote_depth.saturating_add(1);
        self.blockquote_has_content = false;
        self.in_text_block = true;
        Ok(())
    }

    fn handle_divider(&mut self, id: Option<&String>) -> Result<()> {
        self.json.object(|j| {
            j.key("type").value("divider")?;
            if let Some(id_val) = id {
                j.key("id").value(id_val.as_str())?;
            }
            Ok(())
        })
    }

    fn handle_end_table(&mut self) -> Result<()> {
        if self.table_depth == 0 {
            return Ok(());
        }
        if self.table_depth > 1 {
            self.table_depth = self.table_depth.saturating_sub(1);
            return Ok(());
        }
        self.json.close_array()?;
        self.json.close_object()?;
        self.json.key("children").array(|_| Ok(()))?;
        self.json.close_object()?;
        self.table_depth = 0;
        Ok(())
    }

    fn handle_end_table_cell(&mut self) -> Result<()> {
        if self.table_depth > 1 {
            return Ok(());
        }
        self.json.close_array()?;
        self.json.close_object()?;
        self.in_table_cell = false;
        Ok(())
    }

    fn handle_end_table_row(&mut self) -> Result<()> {
        if self.table_depth > 1 {
            return Ok(());
        }
        self.json.close_array()?;
        self.json.close_object()
    }

    fn handle_heading(&mut self, level: u8, id: Option<&String>) -> Result<()> {
        self.json.open_object()?;
        self.json.key("type").value("heading")?;
        self.write_id(id)?;
        self.json.key("props").object(|j| {
            j.key("level").value(level)?;
            j.key("textAlignment").value("left")
        })?;
        self.json.key("content").open_array()?;
        self.in_text_block = true;
        Ok(())
    }

    fn handle_image(
        &mut self,
        source: ImageSource,
        alt: Option<String>,
        id: Option<&String>,
    ) -> Result<()> {
        if self.in_table_cell {
            return Ok(());
        }
        self.close_for_block_sibling()?;
        let url = match source {
            ImageSource::Uri { uri } => uri,
            ImageSource::Asset { asset_id } => self.encode_asset_as_data_uri(&asset_id)?,
        };
        let caption = alt.unwrap_or_default();

        self.json.object(|j| {
            if let Some(id_val) = id {
                j.key("id").value(id_val.as_str())?;
            }
            j.key("type").value("image")?;
            j.key("props").object(|p| {
                p.key("url").value(url.as_str())?;
                p.key("caption").value(caption.as_str())
            })?;
            j.key("content").value(Null)?;
            j.key("children").array(|_| Ok(()))
        })
    }

    fn handle_paragraph(&mut self, id: Option<&String>) -> Result<()> {
        // Inside a table cell, BlockNote's content type is InlineContent[] — block-level events are dropped.
        if self.in_table_cell {
            return Ok(());
        }
        if self.blockquote_depth > 0 {
            if self.blockquote_has_content {
                self.handle_text("\n\n", &TextStyle::default())?;
            }
            return Ok(());
        }
        self.json.open_object()?;
        self.write_id(id)?;
        self.json.key("type").value("paragraph")?;
        self.json
            .key("props")
            .object(|j| j.key("textAlignment").value("left"))?;
        self.json.key("content").open_array()?;
        self.in_text_block = true;
        Ok(())
    }

    fn handle_preformatted(&mut self, id: Option<&String>, syntax: Option<&String>) -> Result<()> {
        self.json.open_object()?;
        self.json.key("type").value("codeBlock")?;
        self.write_id(id)?;
        if let Some(lang) = syntax {
            self.json
                .key("props")
                .object(|j| j.key("language").value(lang.as_str()))?;
        }
        self.json.key("content").open_array()?;
        self.in_text_block = true;
        Ok(())
    }

    fn handle_start_table(&mut self, id: Option<&String>) -> Result<()> {
        if self.table_depth > 0 {
            self.table_depth = self.table_depth.saturating_add(1);
            return Ok(());
        }
        self.close_for_block_sibling()?;
        self.json.open_object()?;
        self.json.key("type").value("table")?;
        self.write_id(id)?;
        self.json
            .key("props")
            .object(|p| p.key("textColor").value("default"))?;
        self.json.key("content").open_object()?;
        self.json.key("type").value("tableContent")?;
        self.json.key("columnWidths").array(|_| Ok(()))?;
        self.json.key("rows").open_array()?;
        self.table_depth = 1;
        self.in_text_block = false;
        Ok(())
    }

    fn handle_start_table_row(&mut self, id: Option<&String>) -> Result<()> {
        if self.table_depth > 1 {
            return Ok(());
        }
        self.json.open_object()?;
        self.write_id(id)?;
        self.json.key("cells").open_array()
    }

    fn handle_table_cell(&mut self, id: Option<&String>) -> Result<()> {
        if self.table_depth > 1 {
            return Ok(());
        }
        self.json.open_object()?;
        self.json.key("type").value("tableCell")?;
        self.write_id(id)?;
        self.json.key("props").object(|p| {
            p.key("backgroundColor").value("default")?;
            p.key("textColor").value("default")?;
            p.key("textAlignment").value("left")
        })?;
        self.json.key("content").open_array()?;
        self.in_table_cell = true;
        self.in_text_block = false;
        Ok(())
    }

    fn handle_text(&mut self, content: &str, style: &TextStyle) -> Result<()> {
        if (!self.in_text_block && !self.in_table_cell) || self.table_depth > 1 {
            return Ok(());
        }
        if self.blockquote_depth > 0 {
            self.blockquote_has_content = true;
        }
        self.json.object(|j| {
            j.key("type").value("text")?;
            j.key("text").value(content)?;
            j.key("styles").object(|s| {
                for (key, enabled) in [
                    ("bold", style.bold),
                    ("italic", style.italic),
                    ("code", style.code),
                    ("strike", style.strikethrough),
                    ("underline", style.underline),
                ] {
                    if enabled {
                        s.key(key).value(true)?;
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
            in_table_cell: false,
            in_text_block: false,
            json: JsonEmitter::new(StrusonBackend::new(writer)),
            table_depth: 0,
        }
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
            in_table_cell: false,
            in_text_block: false,
            json: JsonEmitter::new(StrusonBackend::new(writer)),
            table_depth: 0,
        }
    }

    fn write_id(&mut self, id: Option<&String>) -> Result<()> {
        if let Some(id_val) = id {
            self.json.key("id").value(id_val.as_str())?;
        }
        Ok(())
    }
}

impl<W: Write> EventSink for BlockNoteWriter<'_, W> {
    #[inline]
    fn finish(self) -> Result<()> {
        self.json.finish().map(|_| ())
    }

    #[inline]
    fn handle_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::StartDocument { .. } => self.json.open_array(),
            Event::EndDocument => self.json.close_array(),
            Event::StartHeading { level, id, .. } => {
                return_if_table_cell!(self);
                self.close_for_block_sibling()?;
                self.handle_heading(level, id.as_ref())
            }
            Event::EndHeading => {
                if !self.in_text_block {
                    return Ok(());
                }
                close_text_block!(self)
            }
            Event::EndPreformatted => {
                return_if_table_cell!(self);
                if !self.in_text_block {
                    return Ok(());
                }
                close_text_block!(self)
            }
            Event::StartParagraph { id, .. } => self.handle_paragraph(id.as_ref()),
            Event::EndParagraph => {
                if self.blockquote_depth > 0 || !self.in_text_block || self.in_table_cell {
                    return Ok(());
                }
                close_text_block!(self)
            }
            Event::StartBlockQuote { id, .. } => {
                return_if_table_cell!(self);
                self.close_for_block_sibling()?;
                self.handle_blockquote(id.as_ref())
            }
            Event::EndBlockQuote => {
                return_if_table_cell!(self);
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
                return_if_table_cell!(self);
                self.close_for_block_sibling()?;
                self.handle_preformatted(id.as_ref(), syntax.as_ref())
            }
            Event::ThematicBreak { id, .. } => {
                return_if_table_cell!(self);
                self.close_for_block_sibling()?;
                self.handle_divider(id.as_ref())
            }
            Event::Text { content, style, .. } => {
                // Auto-open paragraph for orphan text (e.g., text after image closed paragraph)
                if !self.in_text_block && self.blockquote_depth == 0 && !self.in_table_cell {
                    self.handle_paragraph(None)?;
                }
                self.handle_text(&content, &style)
            }
            Event::Image {
                source, alt, id, ..
            } => self.handle_image(source, alt, id.as_ref()),
            Event::LineBreak => {
                if (self.in_text_block || self.in_table_cell) && self.table_depth <= 1 {
                    self.handle_text("\n", &TextStyle::default())
                } else {
                    Ok(())
                }
            }
            Event::StartTable { id, .. } => self.handle_start_table(id.as_ref()),
            Event::EndTable => self.handle_end_table(),
            Event::StartTableRow { id, .. } => self.handle_start_table_row(id.as_ref()),
            Event::EndTableRow => self.handle_end_table_row(),
            Event::StartTableCell { id, .. } | Event::StartTableHeader { id, .. } => {
                self.handle_table_cell(id.as_ref())
            }
            Event::EndTableCell | Event::EndTableHeader => self.handle_end_table_cell(),
            Event::EndCaption
            | Event::EndDefinitionDetail
            | Event::EndDefinitionList
            | Event::EndDefinitionTerm
            | Event::EndFootnote
            | Event::EndLink
            | Event::EndOrderedListItem
            | Event::EndUnorderedListItem
            | Event::FootnoteRef { .. }
            | Event::StartCaption { .. }
            | Event::StartDefinitionDetail { .. }
            | Event::StartDefinitionList { .. }
            | Event::StartDefinitionTerm { .. }
            | Event::StartFootnote { .. }
            | Event::StartLink { .. }
            | Event::StartOrderedListItem { .. }
            | Event::StartUnorderedListItem { .. }
            | _ => Ok(()),
        }
    }
}

/// Maps an I/O error to a `DocSpec` error.
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
