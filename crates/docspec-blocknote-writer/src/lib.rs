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
//! - `LineBreak` / `SoftBreak` — line breaks within content blocks
//! - `ThematicBreak` — divider blocks
//! - `StartOrderedListItem` / `EndOrderedListItem` — `numberedListItem` blocks with optional `start` prop
//! - `StartUnorderedListItem` / `EndUnorderedListItem` — `bulletListItem` blocks
//!
//! # Table Cell Content Semantics
//!
//! `BlockNote`'s `tableCell.content` is `InlineContent[]` — it cannot hold block-level types.
//! `EVENTS.md` declares that `DocSpec` cells may contain any block element, so this writer
//! flattens block-level events that appear inside a cell:
//!
//! - **Preserved**: [`Text`](docspec_core::Event::Text) (with all inline styles), [`LineBreak`](docspec_core::Event::LineBreak), [`SoftBreak`](docspec_core::Event::SoftBreak)
//! - **Absorbed silently**: [`StartParagraph`](docspec_core::Event::StartParagraph) / [`EndParagraph`](docspec_core::Event::EndParagraph) (paragraph boundaries are dropped — adjacent paragraphs concatenate without separator)
//! - **Dropped**: [`Image`](docspec_core::Event::Image), [`StartBlockQuote`](docspec_core::Event::StartBlockQuote), [`StartPreformatted`](docspec_core::Event::StartPreformatted), [`StartHeading`](docspec_core::Event::StartHeading), [`ThematicBreak`](docspec_core::Event::ThematicBreak), nested [`StartTable`](docspec_core::Event::StartTable) and their children — silently discarded
//!
//! Nested tables (a `StartTable` inside a cell) are entirely dropped: their rows, cells, text,
//! and closing events are all absorbed. Only the outer table is emitted. The current markdown
//! reader never produces multi-block cell content or nested tables — these guards exist for
//! future DOCX/ODT readers.
//!
//! # List Support
//!
//! **Required**: wrap `BlockNoteWriter` in [`StackTrackingSink`](docspec_core::StackTrackingSink)
//! before feeding list events. Raw list events without that wrapper are undefined behavior.
//! `StackTrackingSink` auto-inserts `StartParagraph` inside list items, which is how the writer
//! knows where each item's inline content begins.
//!
//! `DocSpec` list events translate to `BlockNote` block types as follows:
//!
//! - [`StartUnorderedListItem`](docspec_core::Event::StartUnorderedListItem) → `bulletListItem`
//! - [`StartOrderedListItem`](docspec_core::Event::StartOrderedListItem) → `numberedListItem`
//!   (the `start` field, when present on the first item, becomes the `start` prop)
//!
//! Nesting uses `BlockNote`'s native `children: Block[]` arrays. `DocSpec`'s `level: u32` field
//! drives the nesting depth: a level increase opens a new `children` array; a level decrease
//! closes the appropriate number of open items and children arrays.
//!
//! **Multi-paragraph items**: the first paragraph's inline content populates the list item's
//! `content[]` array. Each subsequent paragraph becomes a child `paragraph` block inside the
//! item's `children[]` array.
//!
//! **Non-paragraph blocks inside list items** (headings, images, code blocks, blockquotes,
//! tables, thematic breaks) are dropped silently along with all of their inline contents —
//! including any `Text` events nested within them. The drop applies anywhere inside a list
//! item: both in the inline `content[]` slot (around the first paragraph) and after the item
//! has transitioned to `children[]` (for multi-paragraph items or items containing nested
//! lists). This differs from the table cell policy: cells preserve `Text` while absorbing
//! block boundaries (so a heading inside a cell becomes plain text), whereas list items
//! suppress text inside dropped blocks entirely.
//!
//! ## Container Interactions
//!
//! - **Inside a table cell**: list items are dropped entirely, consistent with other block-level
//!   content inside cells.
//! - **Inside a blockquote**: the blockquote is force-closed and the list item is emitted at the
//!   top level as a sibling. This matches the existing sibling-emit behavior for headings and
//!   images inside blockquotes.
//!
//! ## Out of Scope
//!
//! - **`checkListItem`**: requires upstream `DocSpec` event support not yet defined.
//! - **`toggleListItem`**: no `DocSpec` event equivalent exists.
//! - **Custom `style_type` markers**: `BlockNote`'s default schema has no equivalent field; the
//!   `style_type` value from `StartOrderedListItem` / `StartUnorderedListItem` is silently dropped.
//!
//! # Example
//!
//! ```
//! use docspec_blocknote_writer::BlockNoteWriter;
//! use docspec_core::{Event, EventSink, ListStyleType, StackTrackingSink, TextStyle};
//!
//! let mut buf = Vec::<u8>::new();
//! let mut writer = StackTrackingSink::new(BlockNoteWriter::new(&mut buf));
//!
//! writer.handle_event(Event::StartDocument { id: None, language: None, metadata: None })?;
//!
//! // Plain paragraph
//! writer.handle_event(Event::StartParagraph { alignment: None, id: None })?;
//! writer.handle_event(Event::Text {
//!     content: "Hello".to_string(),
//!     style: TextStyle::default(),
//! })?;
//! writer.handle_event(Event::EndParagraph)?;
//!
//! // Unordered list item (StackTrackingSink auto-inserts the paragraph)
//! writer.handle_event(Event::StartUnorderedListItem {
//!     id: None,
//!     level: 0,
//!     style_type: ListStyleType::Disc,
//! })?;
//! writer.handle_event(Event::Text {
//!     content: "First bullet".to_string(),
//!     style: TextStyle::default(),
//! })?;
//! writer.handle_event(Event::EndUnorderedListItem)?;
//!
//! // Ordered list item with explicit start number
//! writer.handle_event(Event::StartOrderedListItem {
//!     id: None,
//!     level: 0,
//!     start: Some(1),
//!     style_type: ListStyleType::Decimal,
//! })?;
//! writer.handle_event(Event::Text {
//!     content: "Step one".to_string(),
//!     style: TextStyle::default(),
//! })?;
//! writer.handle_event(Event::EndOrderedListItem)?;
//!
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
use docspec_core::{AssetProvider, Depth, Error, Event, EventSink, ImageSource, Result, TextStyle};
use docspec_json::{JsonEmitter, Null, StrusonBackend};

macro_rules! close_text_block {
    ($writer:expr) => {{
        $writer.close_open_link_if_any()?;
        $writer.close_content_block()?;
        $writer.context.in_text_block = false;
        Ok(())
    }};
}

macro_rules! return_if_table_cell {
    ($writer:expr) => {
        if $writer.context.in_table_cell {
            return Ok(());
        }
    };
}

macro_rules! drop_block_in_list_start {
    ($writer:expr) => {
        if $writer.in_any_list_item() || $writer.drop_inside_list_depth.is_positive() {
            $writer.drop_inside_list_depth.inc();
            return Ok(());
        }
    };
}

macro_rules! drop_block_in_list_end {
    ($writer:expr) => {
        if $writer.drop_inside_list_depth.is_positive() {
            $writer.drop_inside_list_depth.dec();
            return Ok(());
        }
    };
}

/// Represents the kind of list (ordered or unordered).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ListKind {
    /// Ordered list (numbered).
    Ordered,
    /// Unordered list (bulleted).
    Unordered,
}

/// Represents a single entry in the list stack, tracking list nesting state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ListStackEntry {
    /// Whether the children array for this list has been opened.
    children_array_open: bool,
    /// Whether the content array for this list has been opened.
    content_array_open: bool,
    /// Whether the first paragraph in this list item has been consumed.
    first_paragraph_consumed: bool,
    /// The kind of list (ordered or unordered).
    kind: ListKind,
    /// The nesting level of this list (0-based).
    level: u32,
}

#[derive(Default)]
struct BlockContext {
    blockquote_has_content: bool,
    in_table_cell: bool,
    in_text_block: bool,
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
    blockquote_depth: Depth,
    blockquote_force_closed_count: Depth,
    context: BlockContext,
    drop_inside_list_depth: Depth,
    dropped_list_depth: Depth,
    /// Whether the writer is currently inside an open link inline container.
    in_link: bool,
    json: JsonEmitter<StrusonBackend<W>>,
    /// Whether at least one `StyledText` has been emitted into the current link's content array.
    link_emitted_styled_text: bool,
    list_stack: Vec<ListStackEntry>,
    table_depth: Depth,
}

impl<'a, W: Write> BlockNoteWriter<'a, W> {
    fn close_blockquote_for_sibling(&mut self) -> Result<()> {
        self.close_open_link_if_any()?;
        self.close_content_block()?;
        self.blockquote_depth.dec();
        self.blockquote_force_closed_count.inc();
        self.context.in_text_block = self.blockquote_depth.is_positive();
        Ok(())
    }

    fn close_content_block(&mut self) -> Result<()> {
        self.json.close_array()?;
        self.json.key("children").array(|_| Ok(()))?;
        self.json.close_object()
    }

    fn close_current_list_item_object(&mut self) -> Result<()> {
        let popped_entry = self.list_stack.pop();
        if let Some(list_entry) = popped_entry {
            if list_entry.content_array_open {
                self.close_open_link_if_any()?;
                self.json.close_array()?;
            }
            if list_entry.children_array_open {
                self.json.close_array()?;
            } else {
                self.json.key("children").array(|_| Ok(()))?;
            }
            self.json.close_object()?;
        }
        Ok(())
    }

    fn close_for_block_sibling(&mut self) -> Result<()> {
        if !self.list_stack.is_empty() {
            self.close_open_list_items()?;
        }
        if self.blockquote_depth.is_positive() {
            return self.close_blockquote_for_sibling();
        }
        if self.context.in_text_block {
            self.close_open_link_if_any()?;
            self.close_content_block()?;
            self.context.in_text_block = false;
        }
        Ok(())
    }

    /// Defensive: close any open inline link before closing a surrounding block.
    ///
    /// Under the canonical reader + `StackTrackingSink` contract this is a no-op,
    /// but it hardens the writer against direct API misuse where a block may end
    /// while a link is still open.
    fn close_open_link_if_any(&mut self) -> Result<()> {
        if self.in_link {
            self.handle_end_link()?;
        }
        Ok(())
    }

    fn close_open_list_items(&mut self) -> Result<()> {
        while !self.list_stack.is_empty() {
            self.close_current_list_item_object()?;
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
                .map_err(Error::from)?;
            enc.finish().map_err(Error::from)?
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
        self.blockquote_depth.inc();
        self.context.blockquote_has_content = false;
        self.context.in_text_block = true;
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

    /// Closes the current inline link object.
    ///
    /// If no `StyledText` was emitted into the link's `content` array, inserts an empty
    /// `StyledText` (`{"type":"text","text":"","styles":{}}`) to satisfy the `BlockNote`
    /// schema (links must have at least one content item).
    fn handle_end_link(&mut self) -> Result<()> {
        if !self.in_link {
            return Ok(());
        }
        if !self.link_emitted_styled_text {
            self.json.open_object()?;
            self.json.key("type").value("text")?;
            self.json.key("text").value("")?;
            self.json.key("styles").open_object()?;
            self.json.close_object()?;
            self.json.close_object()?;
        }
        self.json.close_array()?;
        self.json.close_object()?;
        self.in_link = false;
        self.link_emitted_styled_text = false;
        Ok(())
    }

    fn handle_end_list_item(&mut self) -> Result<()> {
        if self.dropped_list_depth.is_positive() {
            self.dropped_list_depth.dec();
            return Ok(());
        }
        if self.list_stack.is_empty() {
            return Ok(());
        }
        self.close_current_list_item_object()
    }

    fn handle_end_paragraph(&mut self) -> Result<()> {
        if self.drop_inside_list_depth.is_positive() || self.dropped_list_depth.is_positive() {
            return Ok(());
        }
        if !self.list_stack.is_empty()
            && self
                .list_stack
                .last()
                .is_some_and(|e| e.first_paragraph_consumed)
            && self.context.in_text_block
        {
            self.close_open_link_if_any()?;
            self.json.close_array()?;
            self.json.key("children").array(|_| Ok(()))?;
            self.json.close_object()?;
            self.context.in_text_block = false;
            return Ok(());
        }
        if self.in_list_item_content() {
            if let Some(entry) = self.list_stack.last_mut() {
                entry.first_paragraph_consumed = true;
            }
            return Ok(());
        }
        if self.blockquote_depth.is_positive()
            || !self.context.in_text_block
            || self.context.in_table_cell
        {
            return Ok(());
        }
        close_text_block!(self)
    }

    fn handle_end_table(&mut self) -> Result<()> {
        drop_block_in_list_end!(self);
        if self.table_depth.is_zero() {
            return Ok(());
        }
        if self.table_depth.get() > 1 {
            self.table_depth.dec();
            return Ok(());
        }
        self.json.close_array()?;
        self.json.close_object()?;
        self.json.key("children").array(|_| Ok(()))?;
        self.json.close_object()?;
        self.table_depth.reset();
        Ok(())
    }

    fn handle_end_table_cell(&mut self) -> Result<()> {
        if self.drop_inside_list_depth.is_positive() || self.table_depth.get() > 1 {
            return Ok(());
        }
        self.close_open_link_if_any()?;
        self.json.close_array()?;
        self.json.close_object()?;
        self.context.in_table_cell = false;
        Ok(())
    }

    fn handle_end_table_row(&mut self) -> Result<()> {
        if self.drop_inside_list_depth.is_positive() || self.table_depth.get() > 1 {
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
        self.context.in_text_block = true;
        Ok(())
    }

    fn handle_image(
        &mut self,
        source: ImageSource,
        alt: Option<String>,
        id: Option<&String>,
    ) -> Result<()> {
        if self.context.in_table_cell || self.drop_inside_list_depth.is_positive() {
            return Ok(());
        }
        if self.in_any_list_item() {
            return Ok(());
        }
        self.close_for_block_sibling()?;
        let url = match source {
            ImageSource::Uri { uri } => uri,
            ImageSource::Asset { asset_id } => self.encode_asset_as_data_uri(&asset_id)?,
            _ => return Ok(()),
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

    fn handle_line_break(&mut self) -> Result<()> {
        if self.drop_inside_list_depth.is_positive() {
            return Ok(());
        }
        if (self.context.in_text_block || self.context.in_table_cell || self.in_list_item_content())
            && self.table_depth.get() <= 1
        {
            self.handle_text("\n", &TextStyle::default())
        } else {
            Ok(())
        }
    }

    fn handle_paragraph(&mut self, id: Option<&String>) -> Result<()> {
        // Inside a table cell, BlockNote's content type is InlineContent[] — block-level events are dropped.
        if self.context.in_table_cell {
            return Ok(());
        }
        // Paragraphs nested inside a dropped block must not mutate list state or emit JSON;
        // they are absorbed along with the surrounding dropped block.
        if self.drop_inside_list_depth.is_positive() || self.dropped_list_depth.is_positive() {
            return Ok(());
        }
        // Second and subsequent paragraphs inside a list item dispatch as child paragraph blocks
        // in the item's children[] array (T11). Must be checked before in_list_item_content()
        // because content_array_open may still be true when first_paragraph_consumed is set.
        if !self.list_stack.is_empty()
            && self
                .list_stack
                .last()
                .is_some_and(|e| e.first_paragraph_consumed)
        {
            if self.list_stack.last().is_some_and(|e| e.content_array_open) {
                self.json.close_array()?;
                if let Some(e) = self.list_stack.last_mut() {
                    e.content_array_open = false;
                }
            }
            if !self
                .list_stack
                .last()
                .is_some_and(|e| e.children_array_open)
            {
                self.json.key("children").open_array()?;
                if let Some(e) = self.list_stack.last_mut() {
                    e.children_array_open = true;
                }
            }
            self.json.open_object()?;
            self.json.key("type").value("paragraph")?;
            self.json
                .key("props")
                .object(|j| j.key("textAlignment").value("left"))?;
            self.json.key("content").open_array()?;
            self.context.in_text_block = true;
            return Ok(());
        }
        if self.in_list_item_content() {
            return Ok(());
        }
        if !self.list_stack.is_empty() {
            self.close_open_list_items()?;
        }
        if self.blockquote_depth.is_positive() {
            if self.context.blockquote_has_content {
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
        self.context.in_text_block = true;
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
        self.context.in_text_block = true;
        Ok(())
    }

    /// Opens a `BlockNote` inline link object and its `content` array.
    ///
    /// Drops `title` and `id` — `BlockNote`'s inline link schema has no slot for these.
    fn handle_start_link(&mut self, href: &str) -> Result<()> {
        if self.drop_inside_list_depth.is_positive()
            || self.dropped_list_depth.is_positive()
            || (!self.context.in_text_block
                && !self.context.in_table_cell
                && !self.in_list_item_content())
            || self.table_depth.get() > 1
        {
            return Ok(());
        }
        if self.in_link {
            return Ok(());
        }
        if self.blockquote_depth.is_positive() {
            self.context.blockquote_has_content = true;
        }
        self.json.open_object()?;
        self.json.key("type").value("link")?;
        self.json.key("href").value(href)?;
        self.json.key("content").open_array()?;
        self.in_link = true;
        self.link_emitted_styled_text = false;
        Ok(())
    }

    fn handle_start_list_item(
        &mut self,
        kind: ListKind,
        id: Option<&String>,
        level: u32,
        start: Option<u64>,
    ) -> Result<()> {
        if self.context.in_table_cell
            || self.table_depth.get() > 1
            || self.drop_inside_list_depth.is_positive()
        {
            self.dropped_list_depth.inc();
            return Ok(());
        }
        if self.blockquote_depth.is_positive() {
            self.close_blockquote_for_sibling()?;
        }
        if self.list_stack.is_empty() {
            self.close_for_block_sibling()?;
            self.open_list_item_object(kind, id, level, start)?;
            return Ok(());
        }

        let stack_top_level = self.list_stack.last().map_or(0, |entry| entry.level);

        // Level-jump clamping: silently absorb invalid multi-level forward jumps from broken
        // source documents by treating any skip-ahead as a single step beyond the current top.
        let effective_level = if level > stack_top_level.saturating_add(1) {
            stack_top_level.saturating_add(1)
        } else {
            level
        };

        if effective_level > stack_top_level {
            self.open_current_list_item_children()?;
            self.open_list_item_object(kind, id, effective_level, start)?;
            return Ok(());
        }

        if effective_level == stack_top_level {
            self.close_current_list_item_object()?;
            if self.list_stack.is_empty() {
                self.close_for_block_sibling()?;
            }
            self.open_list_item_object(kind, id, effective_level, start)?;
            return Ok(());
        }

        // Level-down: pop stack entries until the top's level is strictly below effective_level
        // (i.e., at effective_level - 1, the parent). Then open the new item as a sibling.
        while let Some(top) = self.list_stack.last() {
            if top.level < effective_level {
                break;
            }
            self.close_current_list_item_object()?;
        }
        if self.list_stack.is_empty() {
            self.close_for_block_sibling()?;
        }
        self.open_list_item_object(kind, id, effective_level, start)?;
        Ok(())
    }

    fn handle_start_table(&mut self, id: Option<&String>) -> Result<()> {
        drop_block_in_list_start!(self);
        if self.table_depth.is_positive() {
            self.table_depth.inc();
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
        self.table_depth.inc();
        self.context.in_text_block = false;
        Ok(())
    }

    fn handle_start_table_row(&mut self, id: Option<&String>) -> Result<()> {
        if self.drop_inside_list_depth.is_positive() || self.table_depth.get() > 1 {
            return Ok(());
        }
        self.json.open_object()?;
        self.write_id(id)?;
        self.json.key("cells").open_array()
    }

    fn handle_table_cell(&mut self, id: Option<&String>) -> Result<()> {
        if self.drop_inside_list_depth.is_positive() || self.table_depth.get() > 1 {
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
        self.context.in_table_cell = true;
        self.context.in_text_block = false;
        Ok(())
    }

    fn handle_text(&mut self, content: &str, style: &TextStyle) -> Result<()> {
        if self.drop_inside_list_depth.is_positive()
            || self.dropped_list_depth.is_positive()
            || (!self.context.in_text_block
                && !self.context.in_table_cell
                && !self.in_list_item_content())
            || self.table_depth.get() > 1
        {
            return Ok(());
        }
        if self.blockquote_depth.is_positive() {
            self.context.blockquote_has_content = true;
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
        })?;
        if self.in_link {
            self.link_emitted_styled_text = true;
        }
        Ok(())
    }

    fn handle_text_event(&mut self, content: &str, style: &TextStyle) -> Result<()> {
        // Auto-open paragraph for orphan text (e.g., text after image closed paragraph)
        if !self.context.in_text_block
            && self.blockquote_depth.is_zero()
            && !self.context.in_table_cell
            && !self.in_list_item_content()
        {
            self.handle_paragraph(None)?;
        }
        self.handle_text(content, style)
    }

    /// Returns true when any list item is currently open on the stack, regardless of whether
    /// its `content[]` or `children[]` array is the active emission target. Drop trigger for
    /// non-paragraph block events that must be suppressed anywhere inside a list item per the
    /// module-level policy (headings, images, code blocks, blockquotes, tables, thematic
    /// breaks). Broader than `in_list_item_content`, which is true only while `content[]` is
    /// open — `in_any_list_item` also returns true after a multi-paragraph or nested-list
    /// transition has moved emission into `children[]`.
    fn in_any_list_item(&self) -> bool {
        !self.list_stack.is_empty()
    }

    fn in_list_item_content(&self) -> bool {
        self.list_stack
            .last()
            .is_some_and(|entry| entry.content_array_open)
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
            blockquote_depth: Depth::default(),
            blockquote_force_closed_count: Depth::default(),
            context: BlockContext::default(),
            drop_inside_list_depth: Depth::default(),
            dropped_list_depth: Depth::default(),
            in_link: false,
            json: JsonEmitter::new(StrusonBackend::new(writer)),
            link_emitted_styled_text: false,
            list_stack: Vec::new(),
            table_depth: Depth::default(),
        }
    }

    fn open_current_list_item_children(&mut self) -> Result<()> {
        let content_array_open = self
            .list_stack
            .last()
            .is_some_and(|entry| entry.content_array_open);
        if content_array_open {
            self.json.close_array()?;
            if let Some(entry) = self.list_stack.last_mut() {
                entry.content_array_open = false;
            }
        }

        let children_array_open = self
            .list_stack
            .last()
            .is_some_and(|entry| entry.children_array_open);
        if !children_array_open {
            self.json.key("children").open_array()?;
            if let Some(entry) = self.list_stack.last_mut() {
                entry.children_array_open = true;
            }
        }
        Ok(())
    }

    fn open_list_item_object(
        &mut self,
        kind: ListKind,
        id: Option<&String>,
        level: u32,
        start: Option<u64>,
    ) -> Result<()> {
        self.json.open_object()?;
        self.write_id(id)?;
        let type_name = match kind {
            ListKind::Ordered => "numberedListItem",
            ListKind::Unordered => "bulletListItem",
        };
        self.json.key("type").value(type_name)?;
        self.json.key("props").object(|j| {
            j.key("backgroundColor").value("default")?;
            j.key("textColor").value("default")?;
            j.key("textAlignment").value("left")?;
            if kind == ListKind::Ordered {
                if let Some(start_value) = start {
                    let start_prop = u32::try_from(start_value).map_err(|err| Error::Other {
                        message: format!(
                            "ordered list start value out of range: {start_value}: {err}"
                        ),
                    })?;
                    j.key("start").value(start_prop)?;
                }
            }
            Ok(())
        })?;
        self.json.key("content").open_array()?;
        self.list_stack.push(ListStackEntry {
            children_array_open: false,
            content_array_open: true,
            first_paragraph_consumed: false,
            kind,
            level,
        });
        Ok(())
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
            blockquote_depth: Depth::default(),
            blockquote_force_closed_count: Depth::default(),
            context: BlockContext::default(),
            drop_inside_list_depth: Depth::default(),
            dropped_list_depth: Depth::default(),
            in_link: false,
            json: JsonEmitter::new(StrusonBackend::new(writer)),
            link_emitted_styled_text: false,
            list_stack: Vec::new(),
            table_depth: Depth::default(),
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
            Event::EndDocument => {
                while !self.list_stack.is_empty() {
                    self.close_current_list_item_object()?;
                }
                self.json.close_array()
            }
            Event::StartHeading { level, id, .. } => {
                return_if_table_cell!(self);
                drop_block_in_list_start!(self);
                self.close_for_block_sibling()?;
                self.handle_heading(level, id.as_ref())
            }
            Event::EndHeading => {
                drop_block_in_list_end!(self);
                if !self.context.in_text_block {
                    return Ok(());
                }
                close_text_block!(self)
            }
            Event::EndPreformatted => {
                drop_block_in_list_end!(self);
                return_if_table_cell!(self);
                if !self.context.in_text_block {
                    return Ok(());
                }
                close_text_block!(self)
            }
            Event::StartParagraph { id, .. } => self.handle_paragraph(id.as_ref()),
            Event::EndParagraph => self.handle_end_paragraph(),
            Event::StartBlockQuote { id, .. } => {
                return_if_table_cell!(self);
                drop_block_in_list_start!(self);
                self.close_for_block_sibling()?;
                self.handle_blockquote(id.as_ref())
            }
            Event::EndBlockQuote => {
                drop_block_in_list_end!(self);
                return_if_table_cell!(self);
                if self.blockquote_force_closed_count.is_positive() {
                    self.blockquote_force_closed_count.dec();
                    return Ok(());
                }
                self.close_open_link_if_any()?;
                self.close_content_block()?;
                self.blockquote_depth.dec();
                self.context.in_text_block = self.blockquote_depth.is_positive();
                Ok(())
            }
            Event::StartPreformatted { id, syntax, .. } => {
                return_if_table_cell!(self);
                drop_block_in_list_start!(self);
                self.close_for_block_sibling()?;
                self.handle_preformatted(id.as_ref(), syntax.as_ref())
            }
            Event::ThematicBreak { id, .. } => {
                return_if_table_cell!(self);
                if self.in_any_list_item() || self.drop_inside_list_depth.is_positive() {
                    return Ok(());
                }
                self.close_for_block_sibling()?;
                self.handle_divider(id.as_ref())
            }
            Event::Text { content, style, .. } => self.handle_text_event(&content, &style),
            Event::Image {
                source, alt, id, ..
            } => self.handle_image(source, alt, id.as_ref()),
            Event::LineBreak | Event::SoftBreak => self.handle_line_break(),
            Event::StartOrderedListItem {
                id, level, start, ..
            } => self.handle_start_list_item(ListKind::Ordered, id.as_ref(), level, start),
            Event::StartUnorderedListItem { id, level, .. } => {
                self.handle_start_list_item(ListKind::Unordered, id.as_ref(), level, None)
            }
            Event::EndOrderedListItem | Event::EndUnorderedListItem => self.handle_end_list_item(),
            Event::StartTable { id, .. } => self.handle_start_table(id.as_ref()),
            Event::EndTable => self.handle_end_table(),
            Event::StartTableRow { id, .. } => self.handle_start_table_row(id.as_ref()),
            Event::EndTableRow => self.handle_end_table_row(),
            Event::StartTableCell { id, .. } | Event::StartTableHeader { id, .. } => {
                self.handle_table_cell(id.as_ref())
            }
            Event::EndTableCell | Event::EndTableHeader => self.handle_end_table_cell(),
            Event::StartLink { href, .. } => self.handle_start_link(&href),
            Event::EndLink => self.handle_end_link(),
            Event::EndCaption
            | Event::EndDefinitionDetail
            | Event::EndDefinitionList
            | Event::EndDefinitionTerm
            | Event::EndFootnote
            | Event::FootnoteRef { .. }
            | Event::StartCaption { .. }
            | Event::StartDefinitionDetail { .. }
            | Event::StartDefinitionList { .. }
            | Event::StartDefinitionTerm { .. }
            | Event::StartFootnote { .. }
            | _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_stack_empty_after_new() {
        let mut buf = Vec::new();
        let writer = BlockNoteWriter::new(&mut buf);
        assert!(writer.list_stack.is_empty());
    }

    #[test]
    fn close_for_block_sibling_with_nonempty_list_stack_closes_all_items() {
        // Drives close_open_list_items call inside close_for_block_sibling (line 264).
        // After opening a list item, list_stack is non-empty; calling the private
        // method directly exercises the !list_stack.is_empty() branch.
        let mut buf = Vec::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        assert!(writer
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            })
            .is_ok());
        assert!(
            !writer.list_stack.is_empty(),
            "list_stack must be non-empty before calling close_for_block_sibling"
        );
        assert!(writer.close_for_block_sibling().is_ok());
        assert!(
            writer.list_stack.is_empty(),
            "close_for_block_sibling must drain list_stack via close_open_list_items"
        );
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        assert!(writer.finish().is_ok());
    }
}
