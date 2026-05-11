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
//! - `StartOrderedListItem` / `EndOrderedListItem` — numbered list items
//! - `StartUnorderedListItem` / `EndUnorderedListItem` — bullet list items
//! - `StartCheckListItem` / `EndCheckListItem` — check list items with checked state
//! - `Text` — inline text content with bold/italic/code/strikethrough/underline styles
//! - `Image` — image blocks
//! - `LineBreak` — line breaks within content blocks
//! - `ThematicBreak` — divider blocks
//!
//! Table structure events (`StartTable*`, etc.) are silently ignored by this writer.
//!
//! # Example
//!
//! ```
//! use docspec_blocknote_writer::BlockNoteWriter;
//! use docspec_core::{Event, EventSink, StackTrackingSink};
//!
//! let mut buf = Vec::<u8>::new();
//! let mut writer = StackTrackingSink::new(BlockNoteWriter::new(&mut buf));
//!
//! writer.handle_event(Event::StartDocument { id: None, language: None, metadata: None })?;
//! writer.handle_event(Event::StartParagraph { alignment: None, id: None })?;
//! writer.handle_event(Event::Text {
//!     content: "Hello".to_string(),
//!     bold: false, italic: false, code: false,
//!     strikethrough: false, underline: false,
//!     subscript: false, superscript: false, mark: None,
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
use docspec_core::{AssetProvider, Error, Event, EventSink, ImageSource, Result};
use struson::writer::{JsonStreamWriter, JsonWriter as _};

/// Tracks a list item's state for nesting support.
#[derive(Debug, Clone)]
struct ListItemState {
    /// Whether the content array has been closed (ready for children).
    content_closed: bool,
    /// The nesting level of this list item (1 = top-level).
    level: u8,
}

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
    /// Stack of list item states for nesting support.
    list_item_stack: Vec<ListItemState>,
    /// The underlying JSON stream writer.
    writer: JsonStreamWriter<W>,
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
        self.writer.end_array().map_err(io_err)?;
        self.writer.name("children").map_err(io_err)?;
        self.writer.begin_array().map_err(io_err)?;
        self.writer.end_array().map_err(io_err)?;
        self.writer.end_object().map_err(io_err)?;
        Ok(())
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

    fn close_list_items_to_level(&mut self, target_level: u8) -> Result<()> {
        while let Some(item_state) = self.list_item_stack.last() {
            if item_state.level < target_level {
                break;
            }

            let Some(popped_state) = self.list_item_stack.pop() else {
                break;
            };

            if !popped_state.content_closed {
                self.writer.end_array().map_err(io_err)?;
                self.writer.name("children").map_err(io_err)?;
                self.writer.begin_array().map_err(io_err)?;
            }
            self.writer.end_array().map_err(io_err)?;
            self.writer.end_object().map_err(io_err)?;
        }
        self.in_text_block = !self.list_item_stack.is_empty();
        Ok(())
    }

    fn handle_blockquote(&mut self, id: Option<&String>) -> Result<()> {
        self.writer.begin_object().map_err(io_err)?;
        self.writer.name("type").map_err(io_err)?;
        self.writer.string_value("quote").map_err(io_err)?;
        self.write_id(id)?;
        self.writer.name("content").map_err(io_err)?;
        self.writer.begin_array().map_err(io_err)?;
        self.blockquote_depth = self.blockquote_depth.saturating_add(1);
        self.blockquote_has_content = false;
        self.in_text_block = true;
        Ok(())
    }

    fn handle_divider(&mut self, id: Option<&String>) -> Result<()> {
        self.writer.begin_object().map_err(io_err)?;
        self.writer.name("type").map_err(io_err)?;
        self.writer.string_value("divider").map_err(io_err)?;
        self.write_id(id)?;
        self.writer.end_object().map_err(io_err)?;
        Ok(())
    }

    fn handle_end_blockquote(&mut self) -> Result<()> {
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

    fn handle_end_list_item(&mut self) -> Result<()> {
        if let Some(state) = self.list_item_stack.pop() {
            if !state.content_closed {
                self.writer.end_array().map_err(io_err)?;
                self.writer.name("children").map_err(io_err)?;
                self.writer.begin_array().map_err(io_err)?;
            }
            self.writer.end_array().map_err(io_err)?;
            self.writer.end_object().map_err(io_err)?;
        }
        self.in_text_block = self
            .list_item_stack
            .last()
            .is_some_and(|parent| !parent.content_closed);
        Ok(())
    }

    fn handle_end_paragraph(&mut self) -> Result<()> {
        let in_list_content = self
            .list_item_stack
            .last()
            .is_some_and(|item| !item.content_closed);
        if self.blockquote_depth > 0 || !self.in_text_block || in_list_content {
            return Ok(());
        }
        self.close_content_block()?;
        self.in_text_block = false;
        Ok(())
    }

    fn handle_heading(&mut self, level: u8, id: Option<&String>) -> Result<()> {
        self.writer.begin_object().map_err(io_err)?;
        self.writer.name("type").map_err(io_err)?;
        self.writer.string_value("heading").map_err(io_err)?;
        self.write_id(id)?;
        self.writer.name("props").map_err(io_err)?;
        self.writer.begin_object().map_err(io_err)?;
        self.writer.name("level").map_err(io_err)?;
        self.writer.number_value(level).map_err(io_err)?;
        self.writer.name("textAlignment").map_err(io_err)?;
        self.writer.string_value("left").map_err(io_err)?;
        self.writer.end_object().map_err(io_err)?;
        self.writer.name("content").map_err(io_err)?;
        self.writer.begin_array().map_err(io_err)?;
        self.in_text_block = true;
        Ok(())
    }

    fn handle_image(
        &mut self,
        source: ImageSource,
        alt: Option<String>,
        id: Option<&String>,
    ) -> Result<()> {
        if let Some(parent) = self.list_item_stack.last_mut() {
            if !parent.content_closed {
                self.writer.end_array().map_err(io_err)?;
                self.writer.name("children").map_err(io_err)?;
                self.writer.begin_array().map_err(io_err)?;
                parent.content_closed = true;
                self.in_text_block = false;
            }
        } else {
            self.close_for_block_sibling()?;
        }
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

        self.writer.begin_object().map_err(io_err)?;
        self.write_id(id)?;
        self.writer.name("type").map_err(io_err)?;
        self.writer.string_value("image").map_err(io_err)?;
        self.writer.name("props").map_err(io_err)?;
        self.writer.begin_object().map_err(io_err)?;
        self.writer.name("url").map_err(io_err)?;
        self.writer.string_value(&url).map_err(io_err)?;
        self.writer.name("caption").map_err(io_err)?;
        self.writer.string_value(&caption).map_err(io_err)?;
        self.writer.end_object().map_err(io_err)?;
        self.writer.name("content").map_err(io_err)?;
        self.writer.null_value().map_err(io_err)?;
        self.writer.name("children").map_err(io_err)?;
        self.writer.begin_array().map_err(io_err)?;
        self.writer.end_array().map_err(io_err)?;
        self.writer.end_object().map_err(io_err)?;

        Ok(())
    }

    fn handle_list_item(
        &mut self,
        block_type: &str,
        level: u8,
        id: Option<&String>,
        checked: Option<bool>,
    ) -> Result<()> {
        self.close_list_items_to_level(level)?;

        let is_nested_child = self
            .list_item_stack
            .last()
            .is_some_and(|parent| level > parent.level);

        if !is_nested_child {
            self.close_for_block_sibling()?;
        }

        if let Some(parent) = self.list_item_stack.last_mut() {
            if level > parent.level && !parent.content_closed {
                self.writer.end_array().map_err(io_err)?;
                self.writer.name("children").map_err(io_err)?;
                self.writer.begin_array().map_err(io_err)?;
                parent.content_closed = true;
            }
        }

        self.writer.begin_object().map_err(io_err)?;
        self.writer.name("type").map_err(io_err)?;
        self.writer.string_value(block_type).map_err(io_err)?;
        self.write_id(id)?;
        self.writer.name("props").map_err(io_err)?;
        self.writer.begin_object().map_err(io_err)?;
        self.writer.name("textAlignment").map_err(io_err)?;
        self.writer.string_value("left").map_err(io_err)?;
        if let Some(is_checked) = checked {
            self.writer.name("checked").map_err(io_err)?;
            self.writer.bool_value(is_checked).map_err(io_err)?;
        }
        self.writer.end_object().map_err(io_err)?;
        self.writer.name("content").map_err(io_err)?;
        self.writer.begin_array().map_err(io_err)?;

        self.list_item_stack.push(ListItemState {
            content_closed: false,
            level,
        });
        self.in_text_block = true;
        Ok(())
    }

    fn handle_paragraph(&mut self, id: Option<&String>) -> Result<()> {
        if self.blockquote_depth > 0 {
            if self.blockquote_has_content {
                self.handle_text("\n\n", false, false, false, false, false)?;
            }
            return Ok(());
        }
        if self
            .list_item_stack
            .last()
            .is_some_and(|item| !item.content_closed)
        {
            return Ok(());
        }
        self.writer.begin_object().map_err(io_err)?;
        self.write_id(id)?;
        self.writer.name("type").map_err(io_err)?;
        self.writer.string_value("paragraph").map_err(io_err)?;
        self.writer.name("props").map_err(io_err)?;
        self.writer.begin_object().map_err(io_err)?;
        self.writer.name("textAlignment").map_err(io_err)?;
        self.writer.string_value("left").map_err(io_err)?;
        self.writer.end_object().map_err(io_err)?;
        self.writer.name("content").map_err(io_err)?;
        self.writer.begin_array().map_err(io_err)?;
        self.in_text_block = true;
        Ok(())
    }

    fn handle_preformatted(&mut self, id: Option<&String>, syntax: Option<&String>) -> Result<()> {
        self.writer.begin_object().map_err(io_err)?;
        self.writer.name("type").map_err(io_err)?;
        self.writer.string_value("codeBlock").map_err(io_err)?;
        self.write_id(id)?;
        if let Some(lang) = syntax {
            self.writer.name("props").map_err(io_err)?;
            self.writer.begin_object().map_err(io_err)?;
            self.writer.name("language").map_err(io_err)?;
            self.writer.string_value(lang).map_err(io_err)?;
            self.writer.end_object().map_err(io_err)?;
        }
        self.writer.name("content").map_err(io_err)?;
        self.writer.begin_array().map_err(io_err)?;
        self.in_text_block = true;
        Ok(())
    }

    fn handle_text(
        &mut self,
        content: &str,
        bold: bool,
        italic: bool,
        code: bool,
        strikethrough: bool,
        underline: bool,
    ) -> Result<()> {
        if !self.in_text_block {
            return Ok(());
        }
        if self.blockquote_depth > 0 {
            self.blockquote_has_content = true;
        }
        self.writer.begin_object().map_err(io_err)?;
        self.writer.name("type").map_err(io_err)?;
        self.writer.string_value("text").map_err(io_err)?;
        self.writer.name("text").map_err(io_err)?;
        self.writer.string_value(content).map_err(io_err)?;
        self.writer.name("styles").map_err(io_err)?;
        self.writer.begin_object().map_err(io_err)?;
        if bold {
            self.writer.name("bold").map_err(io_err)?;
            self.writer.bool_value(true).map_err(io_err)?;
        }
        if italic {
            self.writer.name("italic").map_err(io_err)?;
            self.writer.bool_value(true).map_err(io_err)?;
        }
        if code {
            self.writer.name("code").map_err(io_err)?;
            self.writer.bool_value(true).map_err(io_err)?;
        }
        if strikethrough {
            self.writer.name("strike").map_err(io_err)?;
            self.writer.bool_value(true).map_err(io_err)?;
        }
        if underline {
            self.writer.name("underline").map_err(io_err)?;
            self.writer.bool_value(true).map_err(io_err)?;
        }
        self.writer.end_object().map_err(io_err)?;
        self.writer.end_object().map_err(io_err)?;
        Ok(())
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
            list_item_stack: Vec::new(),
            writer: JsonStreamWriter::new(writer),
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
            in_text_block: false,
            list_item_stack: Vec::new(),
            writer: JsonStreamWriter::new(writer),
        }
    }

    fn write_id(&mut self, id: Option<&String>) -> Result<()> {
        if let Some(id_val) = id {
            self.writer.name("id").map_err(io_err)?;
            self.writer.string_value(id_val).map_err(io_err)?;
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
            Event::StartDocument { .. } => self.writer.begin_array().map_err(io_err),
            Event::EndDocument => self.writer.end_array().map_err(io_err),
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
            Event::EndParagraph => self.handle_end_paragraph(),
            Event::StartBlockQuote { id, .. } => {
                self.close_for_block_sibling()?;
                self.handle_blockquote(id.as_ref())
            }
            Event::EndBlockQuote => self.handle_end_blockquote(),
            Event::StartPreformatted { id, syntax, .. } => {
                self.close_for_block_sibling()?;
                self.handle_preformatted(id.as_ref(), syntax.as_ref())
            }
            Event::ThematicBreak { id, .. } => {
                self.close_for_block_sibling()?;
                self.handle_divider(id.as_ref())
            }
            Event::Text {
                content,
                bold,
                italic,
                code,
                strikethrough,
                underline,
                ..
            } => {
                // Auto-open paragraph for orphan text (e.g., text after image closed paragraph)
                if !self.in_text_block && self.blockquote_depth == 0 {
                    self.handle_paragraph(None)?;
                }
                self.handle_text(&content, bold, italic, code, strikethrough, underline)
            }
            Event::Image {
                source, alt, id, ..
            } => self.handle_image(source, alt, id.as_ref()),
            Event::LineBreak => {
                if self.in_text_block {
                    self.handle_text("\n", false, false, false, false, false)
                } else {
                    Ok(())
                }
            }
            Event::StartOrderedListItem { id, level, .. } => {
                self.handle_list_item("numberedListItem", level, id.as_ref(), None)
            }
            Event::StartUnorderedListItem { id, level, .. } => {
                self.handle_list_item("bulletListItem", level, id.as_ref(), None)
            }
            Event::StartCheckListItem { id, level, checked } => {
                self.handle_list_item("checkListItem", level, id.as_ref(), Some(checked))
            }
            Event::EndOrderedListItem | Event::EndUnorderedListItem | Event::EndCheckListItem => {
                self.handle_end_list_item()
            }
            Event::EndCaption
            | Event::EndDefinitionTerm
            | Event::EndLink
            | Event::EndDefinitionDetail
            | Event::EndDefinitionList
            | Event::EndFootnote
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
