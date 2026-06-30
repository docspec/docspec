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
//! constant regardless of document size. Local buffering is used only for bounded conversion
//! details such as asset-based image data URI encoding and lifting nested table event substreams
//! after their enclosing table closes.
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
//! - `StartTextStyle` / `EndTextStyle` — inline style spans
//! - `Text` — inline text content styled by currently open style spans
//! - `Image` — image blocks
//! - `LineBreak` / `SoftBreak` — line breaks within content blocks
//! - `ThematicBreak` — divider blocks
//! - `StartOrderedListItem` / `EndOrderedListItem` — `numberedListItem` blocks with optional `start` prop
//! - `StartUnorderedListItem` / `EndUnorderedListItem` — `bulletListItem` blocks
//!
//! # Table Cell Content Semantics
//!
//! `BlockNote`'s `tableCell.content` is `InlineContent[]` — it cannot hold block-level types.
//! The [`docspec_core::event`] well-formedness rules declare that `DocSpec` cells may contain any
//! block element, so this writer flattens block-level events that appear inside a cell:
//!
//! - **Preserved**: [`StartTextStyle`](docspec_core::Event::StartTextStyle) / [`EndTextStyle`](docspec_core::Event::EndTextStyle), [`Text`](docspec_core::Event::Text) (with currently open inline styles), [`LineBreak`](docspec_core::Event::LineBreak), [`SoftBreak`](docspec_core::Event::SoftBreak)
//! - **Absorbed silently**: [`StartParagraph`](docspec_core::Event::StartParagraph) / [`EndParagraph`](docspec_core::Event::EndParagraph) (paragraph boundaries are dropped — adjacent paragraphs concatenate without separator)
//! - **Dropped**: [`StartBlockQuote`](docspec_core::Event::StartBlockQuote), [`StartPreformatted`](docspec_core::Event::StartPreformatted), [`StartHeading`](docspec_core::Event::StartHeading), [`ThematicBreak`](docspec_core::Event::ThematicBreak) — silently discarded
//! - **Lifted**: nested [`StartTable`](docspec_core::Event::StartTable) and its children, and [`Image`](docspec_core::Event::Image) events that appear inside a cell — buffered and replayed as top-level sibling blocks after the enclosing outermost table closes
//!
//! Nested tables (a `StartTable` inside a cell) are buffered between the first nested
//! `StartTable` and its matching `EndTable`, then replayed through the writer after the
//! enclosing outermost table closes. Each lifted nested table emits as a top-level sibling
//! block in document order. The buffer empties on every outer `EndTable`, so nesting any
//! number of levels deep collapses to a flat top-level sequence: `A` containing `B`
//! containing `C` emits as `A, B, C`. Inline text adjacent to a nested table in the same
//! outer cell stays in that outer cell.
//!
//! Images encountered inside a cell are likewise buffered and replayed as top-level sibling
//! blocks after the enclosing outermost table closes. Their order in the output mirrors the
//! order they appeared inside the table (interleaved with any lifted nested tables in
//! document order). Inline text adjacent to a lifted image in the same cell stays in that
//! cell — only the image moves out. Images inside a nested cell lift through the recursive
//! drain: they emit as siblings of the (already-lifted) nested table.
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
//! The `id` field on `Start*ListItem` events is dropped — list items never emit an `id` key,
//! matching how `paragraph` blocks behave when their source has no id. Upstream readers (notably
//! the DOCX reader) use this field to carry the OOXML `numId`, which is shared across every item
//! in the same list rather than uniquely identifying one item, so propagating it as `BlockNote`'s
//! per-block `id` would be misleading.
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
//! use docspec_core::{Event, EventSink, ListStyleType, StackTrackingSink};
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

pub mod palette;

use std::io::Write;

use docspec_core::{
    BlockKind, Depth, Error, Event, EventSink, ImageSource, Result, TextAlignment, TextStyleKind,
};
use docspec_json::{JsonEmitter, Null, StrusonBackend};

macro_rules! close_text_block {
    ($writer:expr) => {{
        $writer.close_open_link_if_any()?;
        $writer.close_content_block()?;
        $writer.context.in_text_block = false;
        Ok(())
    }};
}

/// Represents the kind of list (ordered or unordered).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ListKind {
    /// Ordered list (numbered).
    Ordered,
    /// Unordered list (bulleted).
    Unordered,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ListContentState {
    Pending,
    Open,
    Closed,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
enum BlockquoteContentState {
    /// Quote is open but no content has been emitted yet.
    /// The writer has NOT yet decided whether to open `"content": [` or `"children": [`.
    #[default]
    Pending,
    /// `"content": [` is open and we're emitting inline content into it.
    InContent,
    /// `"content": []` has been written and closed; `"children": [` is open
    /// and we're emitting block children into it.
    InChildren,
}

#[derive(Debug)]
enum DrainDestination {
    DocumentRoot,
    Ancestor {
        ancestor_index: usize,
        kind: BlockKind,
    },
}

/// Block-level events that are leaves (no Start/End counterpart) in the `DocSpec` stream.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum BlockLeafKind {
    Image,
    Divider,
}

/// Represents a potential child block kind for the `blocknote_accepts_child` predicate.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum ChildKind {
    Container(BlockKind),
    Leaf(BlockLeafKind),
}

/// Returns true iff `BlockNote`'s default schema allows `child` to appear inside `parent`.
/// Captures the schema constraint, NOT writer policy (drop/lift is the policy layer).
fn blocknote_accepts_child(parent: Option<BlockKind>, child: ChildKind) -> bool {
    match parent {
        None => true,
        Some(
            BlockKind::Blockquote
            | BlockKind::Heading
            | BlockKind::OrderedListItem
            | BlockKind::UnorderedListItem,
        ) => !matches!(
            child,
            ChildKind::Container(
                BlockKind::TableRow | BlockKind::TableCell | BlockKind::TableHeader
            )
        ),
        Some(_) => false,
    }
}

const _: BlockLeafKind = BlockLeafKind::Image;
const _: BlockLeafKind = BlockLeafKind::Divider;
const _: ChildKind = ChildKind::Container(BlockKind::Paragraph);
const _: ChildKind = ChildKind::Leaf(BlockLeafKind::Image);
const _: fn(Option<BlockKind>, ChildKind) -> bool = blocknote_accepts_child;

/// Represents a single entry in the list stack, tracking list nesting state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ListStackEntry {
    /// Whether the children array for this list has been opened.
    children_array_open: bool,
    /// Current state of this list item's content array.
    content_state: ListContentState,
    /// Whether the first paragraph in this list item has been consumed.
    first_paragraph_consumed: bool,
    /// The kind of list (ordered or unordered).
    kind: ListKind,
    /// The nesting level of this list (0-based).
    level: u32,
    /// Starting number for ordered list items.
    start: Option<u32>,
}

#[derive(Default)]
struct BlockContext {
    blockquote_has_content: bool,
    in_table_cell: bool,
    in_text_block: bool,
}

fn non_default_alignment_value(alignment: Option<&TextAlignment>) -> Option<&'static str> {
    match alignment {
        Some(TextAlignment::Center) => Some("center"),
        Some(TextAlignment::Right) => Some("right"),
        Some(TextAlignment::Justify) => Some("justify"),
        _ => None,
    }
}

/// A streaming `BlockNote` JSON writer.
///
/// Writes JSON tokens directly to the underlying `Write` as events arrive using `docspec-json`.
/// Implements [`EventSink`] for integration with the `DocSpec` pipeline.
///
/// # Type Parameters
///
/// * `W` - Any type implementing [`Write`]
pub struct BlockNoteWriter<W: Write> {
    blockquote_depth: Depth,
    /// Per-blockquote content state stack. One entry is pushed on each `StartBlockQuote` and
    /// popped on the matching `EndBlockQuote`. Tracks whether the current blockquote has opened
    /// its `"content": [` array (inline content), its `"children": [` array (block children),
    /// or neither yet (pending).
    blockquote_content_states: Vec<BlockquoteContentState>,
    context: BlockContext,
    /// Whether the writer is currently inside an open link inline container.
    in_link: bool,
    json: JsonEmitter<StrusonBackend<W>>,
    /// Whether at least one `StyledText` has been emitted into the current link's content array.
    link_emitted_styled_text: bool,
    /// Events deferred for replay at the outermost `EndTable` as top-level siblings:
    /// nested-table substreams, in-cell `Image` events, and block-level events started
    /// inside a table cell. All are illegal inside `BlockNote`'s `tableCell.content`;
    /// co-buffering preserves their document order.
    lifted_nested_events: Vec<Event>,
    /// Tracks nesting depth of non-table block-level subtrees currently being buffered
    /// for lift (e.g. a heading or blockquote that started inside a table cell). Increments
    /// on each non-table block-level start event while buffering; decrements on the
    /// matching end. Zero means we are not currently inside such a subtree.
    lifted_subtree_depth: Depth,
    list_stack: Vec<ListStackEntry>,
    /// Ordered stack of open block containers in emission order.
    /// Pushed when an outer container is opened in the EMITTED output (not while buffering).
    /// Used to compute the nearest valid drain destination for lifted events.
    /// Contains: `Blockquote`, `OrderedListItem`, `UnorderedListItem`, `Table` (tables listed for
    /// completeness but never serve as drain destinations per predicate).
    open_block_ancestors: Vec<BlockKind>,
    open_styles: Vec<TextStyleKind>,
    table_depth: Depth,
}

impl<W: Write> BlockNoteWriter<W> {
    fn close_content_block(&mut self) -> Result<()> {
        self.json.close_array()?;
        self.json.key("children").array(|_| Ok(()))?;
        self.json.close_object()
    }

    fn close_current_list_item_object(&mut self) -> Result<()> {
        if self
            .list_stack
            .last()
            .is_some_and(|entry| entry.content_state == ListContentState::Pending)
        {
            self.initialize_current_list_item_content(None)?;
        }
        let popped_entry = self.list_stack.pop();
        if let Some(list_entry) = popped_entry {
            let ancestor_kind = match list_entry.kind {
                ListKind::Ordered => BlockKind::OrderedListItem,
                ListKind::Unordered => BlockKind::UnorderedListItem,
            };
            if list_entry.content_state == ListContentState::Open {
                self.close_open_link_if_any()?;
                self.json.close_array()?;
            }
            if list_entry.children_array_open {
                self.json.close_array()?;
            } else {
                self.json.key("children").array(|_| Ok(()))?;
            }
            self.json.close_object()?;
            self.pop_ancestor(ancestor_kind);
        }
        Ok(())
    }

    fn close_for_block_sibling(&mut self) -> Result<()> {
        if !self.list_stack.is_empty() {
            self.close_open_list_items()?;
        }
        if self.blockquote_depth.is_positive() {
            return self.prepare_blockquote_children();
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

    fn handle_blockquote(&mut self, id: Option<&str>) -> Result<()> {
        self.json.open_object()?;
        self.json.key("type").value("quote")?;
        self.write_id(id)?;
        self.blockquote_depth.inc();
        self.blockquote_content_states
            .push(BlockquoteContentState::Pending);
        self.open_block_ancestors.push(BlockKind::Blockquote);
        self.context.blockquote_has_content = false;
        Ok(())
    }

    fn prepare_blockquote_inline_content(&mut self) -> Result<()> {
        if self.blockquote_content_states.last() == Some(&BlockquoteContentState::Pending) {
            self.json.key("content").open_array()?;
            if let Some(state) = self.blockquote_content_states.last_mut() {
                *state = BlockquoteContentState::InContent;
            }
            self.context.in_text_block = true;
        }
        Ok(())
    }

    fn prepare_blockquote_children(&mut self) -> Result<()> {
        match self.blockquote_content_states.last().copied() {
            Some(BlockquoteContentState::Pending) => {
                self.json.key("content").open_array()?;
                self.json.close_array()?;
                self.json.key("children").open_array()?;
                if let Some(state) = self.blockquote_content_states.last_mut() {
                    *state = BlockquoteContentState::InChildren;
                }
            }
            Some(BlockquoteContentState::InContent) => {
                self.close_open_link_if_any()?;
                self.json.close_array()?;
                self.context.in_text_block = false;
                self.json.key("children").open_array()?;
                if let Some(state) = self.blockquote_content_states.last_mut() {
                    *state = BlockquoteContentState::InChildren;
                }
            }
            Some(BlockquoteContentState::InChildren) | None => {}
        }
        Ok(())
    }

    fn handle_divider(&mut self, id: Option<&str>) -> Result<()> {
        self.json.object(|j| {
            j.key("type").value("divider")?;
            if let Some(id_val) = id {
                j.key("id").value(id_val)?;
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
        if self.list_stack.is_empty() {
            return Ok(());
        }
        self.close_current_list_item_object()
    }

    fn handle_end_paragraph(&mut self) -> Result<()> {
        let in_current_blockquote_list_item =
            self.blockquote_depth.is_positive() && self.current_blockquote_contains_list_item();
        if !self.list_stack.is_empty()
            && (self.blockquote_depth.is_zero() || in_current_blockquote_list_item)
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
        if self.blockquote_depth.is_positive()
            && self.context.in_text_block
            && self
                .blockquote_content_states
                .last()
                .is_some_and(|state| *state == BlockquoteContentState::InChildren)
        {
            return close_text_block!(self);
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
        if self.table_depth.is_zero() {
            return Ok(());
        }
        self.json.close_array()?;
        self.json.close_object()?;
        self.json.key("children").array(|_| Ok(()))?;
        self.json.close_object()?;
        self.pop_ancestor(BlockKind::Table);
        self.table_depth.dec();
        Ok(())
    }

    fn handle_end_table_cell(&mut self) -> Result<()> {
        self.close_open_link_if_any()?;
        self.json.close_array()?;
        self.json.close_object()?;
        self.context.in_table_cell = false;
        Ok(())
    }

    fn handle_end_table_row(&mut self) -> Result<()> {
        self.json.close_array()?;
        self.json.close_object()
    }

    fn handle_end_blockquote(&mut self) -> Result<()> {
        if self.blockquote_depth.is_zero() {
            return Ok(());
        }
        let state = self.blockquote_content_states.pop().unwrap_or_default();
        match state {
            BlockquoteContentState::Pending => {
                self.json.key("content").open_array()?;
                self.json.close_array()?;
                self.json.key("children").open_array()?;
                self.json.close_array()?;
            }
            BlockquoteContentState::InContent => {
                self.close_open_link_if_any()?;
                self.json.close_array()?;
                self.context.in_text_block = false;
                self.json.key("children").open_array()?;
                self.json.close_array()?;
            }
            BlockquoteContentState::InChildren => {
                self.json.close_array()?;
            }
        }
        self.pop_ancestor(BlockKind::Blockquote);
        self.blockquote_depth.dec();
        self.context.in_text_block = self
            .blockquote_content_states
            .last()
            .is_some_and(|s| *s == BlockquoteContentState::InContent);
        self.json.close_object()?;
        Ok(())
    }

    fn handle_heading(&mut self, level: u8, id: Option<&str>) -> Result<()> {
        self.json.open_object()?;
        self.json.key("type").value("heading")?;
        self.write_id(id)?;
        self.json
            .key("props")
            .object(|j| j.key("level").value(level))?;
        self.json.key("content").open_array()?;
        self.context.in_text_block = true;
        Ok(())
    }

    fn handle_image(
        &mut self,
        source: ImageSource,
        alt: Option<String>,
        id: Option<&str>,
    ) -> Result<()> {
        self.prepare_for_child_block()?;
        let caption = alt.unwrap_or_default();

        match source {
            ImageSource::Uri { uri } => self.json.object(|j| {
                if let Some(id_val) = id {
                    j.key("id").value(id_val)?;
                }
                j.key("type").value("image")?;
                j.key("props").object(|p| {
                    p.key("url").value(uri.as_str())?;
                    p.key("caption").value(caption.as_str())
                })?;
                j.key("content").value(Null)?;
                j.key("children").array(|_| Ok(()))
            }),
            ImageSource::Asset(handle) => {
                let content_type = handle
                    .content_type()
                    .ok_or_else(|| Error::Other {
                        message: format!("asset not found: {}", handle.asset_id()),
                    })?
                    .into_owned();

                self.json.object(|j| {
                    if let Some(id_val) = id {
                        j.key("id").value(id_val)?;
                    }
                    j.key("type").value("image")?;
                    j.key("props").object(|p| {
                        p.key("url").string_value_streaming(|w| {
                            write!(w, "data:{content_type};base64,")?;
                            let mut enc = base64::write::EncoderWriter::new(
                                w,
                                &base64::engine::general_purpose::STANDARD,
                            );
                            handle.stream_to(&mut enc)?;
                            enc.finish()?;
                            Ok(())
                        })?;
                        p.key("caption").value(caption.as_str())
                    })?;
                    j.key("content").value(Null)?;
                    j.key("children").array(|_| Ok(()))
                })
            }
            _ => Ok(()),
        }
    }

    fn handle_line_break(&mut self) -> Result<()> {
        if self.context.in_text_block || self.context.in_table_cell || self.in_list_item_content() {
            self.handle_text("\n")
        } else {
            Ok(())
        }
    }

    fn handle_paragraph(
        &mut self,
        id: Option<&str>,
        alignment: Option<&TextAlignment>,
    ) -> Result<()> {
        // Inside a table cell, BlockNote's content type is InlineContent[] — block-level events are dropped.
        if self.context.in_table_cell {
            return Ok(());
        }
        let in_current_blockquote_list_item =
            self.blockquote_depth.is_positive() && self.current_blockquote_contains_list_item();
        // Second and subsequent paragraphs inside a list item dispatch as child paragraph blocks
        // in the item's children[] array (T11). Must be checked before in_list_item_content()
        // because content may still be open when first_paragraph_consumed is set.
        if !self.list_stack.is_empty()
            && (self.blockquote_depth.is_zero() || in_current_blockquote_list_item)
            && self
                .list_stack
                .last()
                .is_some_and(|e| e.first_paragraph_consumed)
        {
            if self
                .list_stack
                .last()
                .is_some_and(|e| e.content_state == ListContentState::Open)
            {
                self.json.close_array()?;
                if let Some(e) = self.list_stack.last_mut() {
                    e.content_state = ListContentState::Closed;
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
            self.write_paragraph_props(alignment)?;
            self.json.key("content").open_array()?;
            self.context.in_text_block = true;
            return Ok(());
        }
        if !self.list_stack.is_empty()
            && (self.blockquote_depth.is_zero() || in_current_blockquote_list_item)
        {
            self.initialize_current_list_item_content(alignment)?;
            return Ok(());
        }
        if self.blockquote_depth.is_positive() {
            if self
                .blockquote_content_states
                .last()
                .is_some_and(|state| *state == BlockquoteContentState::InChildren)
            {
                self.json.open_object()?;
                self.write_id(id)?;
                self.json.key("type").value("paragraph")?;
                self.write_paragraph_props(alignment)?;
                self.json.key("content").open_array()?;
                self.context.in_text_block = true;
                return Ok(());
            }
            if self.context.blockquote_has_content {
                self.handle_text("\n\n")?;
            }
            self.prepare_blockquote_inline_content()?;
            return Ok(());
        }
        self.json.open_object()?;
        self.write_id(id)?;
        self.json.key("type").value("paragraph")?;
        self.write_paragraph_props(alignment)?;
        self.json.key("content").open_array()?;
        self.context.in_text_block = true;
        Ok(())
    }

    fn write_paragraph_props(&mut self, alignment: Option<&TextAlignment>) -> Result<()> {
        if let Some(value) = non_default_alignment_value(alignment) {
            self.json
                .key("props")
                .object(|j| j.key("textAlignment").value(value))?;
        }
        Ok(())
    }

    fn handle_preformatted(&mut self, id: Option<&str>, syntax: Option<&str>) -> Result<()> {
        self.json.open_object()?;
        self.json.key("type").value("codeBlock")?;
        self.write_id(id)?;
        if let Some(lang) = syntax {
            self.json
                .key("props")
                .object(|j| j.key("language").value(lang))?;
        }
        self.json.key("content").open_array()?;
        self.context.in_text_block = true;
        Ok(())
    }

    /// Opens a `BlockNote` inline link object and its `content` array.
    ///
    /// Drops `title` and `id` — `BlockNote`'s inline link schema has no slot for these.
    fn handle_start_link(&mut self, href: &str) -> Result<()> {
        if self.list_stack.last().is_some_and(|entry| {
            entry.content_state == ListContentState::Pending && !entry.first_paragraph_consumed
        }) {
            self.initialize_current_list_item_content(None)?;
        }
        if !self.context.in_text_block
            && !self.context.in_table_cell
            && !self.in_list_item_content()
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

    fn handle_start_text_style(&mut self, kind: TextStyleKind) -> Result<()> {
        self.open_styles.push(kind);
        Ok(())
    }

    fn handle_end_text_style(&mut self) -> Result<()> {
        if self.open_styles.pop().is_none() {
            return Err(Error::InvalidSequence {
                expected: "StartTextStyle".to_string(),
                found: "EndTextStyle".to_string(),
                message: "cannot close text style because no text style is open".to_string(),
            });
        }
        Ok(())
    }

    fn current_blockquote_contains_list_item(&self) -> bool {
        let Some(blockquote_index) = self
            .open_block_ancestors
            .iter()
            .rposition(|kind| *kind == BlockKind::Blockquote)
        else {
            return false;
        };
        self.open_block_ancestors
            .iter()
            .skip(blockquote_index.saturating_add(1))
            .any(|kind| {
                matches!(
                    kind,
                    BlockKind::OrderedListItem | BlockKind::UnorderedListItem
                )
            })
    }

    fn handle_start_list_item(
        &mut self,
        kind: ListKind,
        level: u32,
        start: Option<u64>,
    ) -> Result<()> {
        if self.blockquote_depth.is_positive() {
            self.prepare_blockquote_children()?;
            if !self.current_blockquote_contains_list_item() {
                self.open_list_item_object(kind, level, start)?;
                return Ok(());
            }
        }
        if self.list_stack.is_empty() {
            self.close_for_block_sibling()?;
            self.open_list_item_object(kind, level, start)?;
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
            self.open_list_item_object(kind, effective_level, start)?;
            return Ok(());
        }

        if effective_level == stack_top_level {
            self.close_current_list_item_object()?;
            if self.list_stack.is_empty() {
                self.close_for_block_sibling()?;
            }
            self.open_list_item_object(kind, effective_level, start)?;
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
        self.open_list_item_object(kind, effective_level, start)?;
        Ok(())
    }

    fn handle_start_table(&mut self, id: Option<&str>) -> Result<()> {
        self.prepare_for_child_block()?;
        self.json.open_object()?;
        self.json.key("type").value("table")?;
        self.write_id(id)?;
        self.json.key("content").open_object()?;
        self.json.key("type").value("tableContent")?;
        self.json.key("columnWidths").array(|_| Ok(()))?;
        self.json.key("rows").open_array()?;
        self.table_depth.inc();
        self.open_block_ancestors.push(BlockKind::Table);
        self.context.in_text_block = false;
        Ok(())
    }

    fn handle_start_table_row(&mut self, id: Option<&str>) -> Result<()> {
        self.json.open_object()?;
        self.write_id(id)?;
        self.json.key("cells").open_array()
    }

    fn handle_start_table_cell_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::StartTableCell {
                colspan,
                id,
                rowspan,
                ..
            }
            | Event::StartTableHeader {
                colspan,
                id,
                rowspan,
                ..
            } => self.handle_table_cell(id.as_deref(), colspan, rowspan),
            _ => Ok(()),
        }
    }

    fn handle_table_cell(
        &mut self,
        id: Option<&str>,
        colspan: Option<u32>,
        rowspan: Option<u32>,
    ) -> Result<()> {
        self.json.open_object()?;
        self.json.key("type").value("tableCell")?;
        self.write_id(id)?;
        if colspan.is_some() || rowspan.is_some() {
            self.json.key("props").object(|j| {
                if let Some(n) = colspan {
                    j.key("colspan").value(n)?;
                }
                if let Some(n) = rowspan {
                    j.key("rowspan").value(n)?;
                }
                Ok(())
            })?;
        }
        self.json.key("content").open_array()?;
        self.context.in_table_cell = true;
        self.context.in_text_block = false;
        Ok(())
    }

    fn handle_text(&mut self, content: &str) -> Result<()> {
        if !self.context.in_text_block
            && !self.context.in_table_cell
            && !self.in_list_item_content()
        {
            return Ok(());
        }
        if self.blockquote_depth.is_positive() {
            self.context.blockquote_has_content = true;
        }
        let mut bold = false;
        let mut italic = false;
        let mut code = false;
        let mut strike = false;
        let mut underline = false;
        let mut text_color: Option<docspec_core::Color> = None;
        let mut background_color: Option<docspec_core::Color> = None;

        for kind in &self.open_styles {
            match kind {
                TextStyleKind::Bold => bold = true,
                TextStyleKind::Italic => italic = true,
                TextStyleKind::Code => code = true,
                TextStyleKind::Strikethrough => strike = true,
                TextStyleKind::Underline => underline = true,
                TextStyleKind::Subscript => {
                    // Intentionally not rendered: BlockNote's default schema has no subscript representation.
                    Self::omit_unsupported_text_style("subscript");
                }
                TextStyleKind::Superscript => {
                    // Intentionally not rendered: BlockNote's default schema has no superscript representation.
                    Self::omit_unsupported_text_style("superscript");
                }
                TextStyleKind::TextColor(color) => text_color = Some(*color),
                TextStyleKind::Mark(color) => background_color = Some(*color),
                future_kind => {
                    // Future text styles are accepted and omitted until BlockNote has a mapped representation.
                    Self::omit_future_text_style(future_kind);
                }
            }
        }

        self.json.object(|j| {
            j.key("type").value("text")?;
            j.key("text").value(content)?;
            j.key("styles").object(|s| {
                for (key, enabled) in [
                    ("bold", bold),
                    ("italic", italic),
                    ("code", code),
                    ("strike", strike),
                    ("underline", underline),
                ] {
                    if enabled {
                        s.key(key).value(true)?;
                    }
                }
                if let Some(c) = text_color {
                    if let Some(name) = palette::nearest_text_color(&c) {
                        s.key("textColor").value(name)?;
                    }
                }
                if let Some(c) = background_color {
                    if let Some(name) = palette::nearest_background_color(&c) {
                        s.key("backgroundColor").value(name)?;
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

    fn handle_text_event(&mut self, content: &str) -> Result<()> {
        // Auto-open paragraph for orphan text (e.g., text after image closed paragraph)
        if self.list_stack.last().is_some_and(|entry| {
            entry.content_state == ListContentState::Pending && !entry.first_paragraph_consumed
        }) {
            self.initialize_current_list_item_content(None)?;
        }
        if self.blockquote_depth.is_positive() && !self.context.in_text_block {
            self.prepare_blockquote_inline_content()?;
        }
        if !self.context.in_text_block
            && self.blockquote_depth.is_zero()
            && !self.context.in_table_cell
            && !self.in_list_item_content()
        {
            self.handle_paragraph(None, None)?;
        }
        self.handle_text(content)
    }

    fn in_list_item_content(&self) -> bool {
        self.list_stack
            .last()
            .is_some_and(|entry| entry.content_state == ListContentState::Open)
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
            blockquote_depth: Depth::default(),
            blockquote_content_states: Vec::new(),
            context: BlockContext::default(),
            in_link: false,
            json: JsonEmitter::new(StrusonBackend::new(writer)),
            lifted_nested_events: Vec::new(),
            lifted_subtree_depth: Depth::default(),
            link_emitted_styled_text: false,
            list_stack: Vec::new(),
            open_block_ancestors: Vec::new(),
            open_styles: Vec::new(),
            table_depth: Depth::default(),
        }
    }

    fn current_drain_destination(&self) -> DrainDestination {
        for (idx, kind) in self.open_block_ancestors.iter().enumerate().rev() {
            if blocknote_accepts_child(Some(*kind), ChildKind::Container(BlockKind::Heading)) {
                return DrainDestination::Ancestor {
                    ancestor_index: idx,
                    kind: *kind,
                };
            }
        }
        DrainDestination::DocumentRoot
    }

    fn compute_rebase_offset(&self, buffered: &[Event]) -> u32 {
        let min_buffered_level = buffered
            .iter()
            .filter_map(|event| match event {
                Event::StartOrderedListItem { level, .. }
                | Event::StartUnorderedListItem { level, .. } => Some(*level),
                _ => None,
            })
            .min();

        let Some(min_level) = min_buffered_level else {
            return 0;
        };

        let required_min_level = self
            .list_stack
            .last()
            .map_or(0, |entry| entry.level.saturating_add(1));
        required_min_level.saturating_sub(min_level)
    }

    fn pop_ancestor(&mut self, kind: BlockKind) {
        if let Some(pos) = self
            .open_block_ancestors
            .iter()
            .rposition(|entry| *entry == kind)
        {
            self.open_block_ancestors.remove(pos);
        }
    }

    fn initialize_current_list_item_content(
        &mut self,
        alignment: Option<&TextAlignment>,
    ) -> Result<()> {
        let Some(current_entry) = self.list_stack.last() else {
            return Ok(());
        };
        if current_entry.content_state != ListContentState::Pending {
            return Ok(());
        }
        let kind = current_entry.kind;
        let start = current_entry.start;
        let alignment_value = non_default_alignment_value(alignment);
        if alignment_value.is_some() || (kind == ListKind::Ordered && start.is_some()) {
            self.json.key("props").object(|j| {
                if let Some(value) = alignment_value {
                    j.key("textAlignment").value(value)?;
                }
                if kind == ListKind::Ordered {
                    if let Some(start_prop) = start {
                        j.key("start").value(start_prop)?;
                    }
                }
                Ok(())
            })?;
        }
        self.json.key("content").open_array()?;
        if let Some(entry) = self.list_stack.last_mut() {
            entry.content_state = ListContentState::Open;
        }
        Ok(())
    }

    fn open_current_list_item_children(&mut self) -> Result<()> {
        if self
            .list_stack
            .last()
            .is_some_and(|entry| entry.content_state == ListContentState::Pending)
        {
            self.initialize_current_list_item_content(None)?;
        }
        let content_array_open = self
            .list_stack
            .last()
            .is_some_and(|entry| entry.content_state == ListContentState::Open);
        if content_array_open {
            self.json.close_array()?;
            if let Some(entry) = self.list_stack.last_mut() {
                entry.content_state = ListContentState::Closed;
                entry.first_paragraph_consumed = true;
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

    fn prepare_list_item_children(&mut self) -> Result<()> {
        self.open_current_list_item_children()
    }

    fn prepare_for_child_block(&mut self) -> Result<()> {
        match self.current_drain_destination() {
            DrainDestination::Ancestor {
                kind: BlockKind::Blockquote,
                ..
            } => self.prepare_blockquote_children(),
            DrainDestination::Ancestor {
                kind: BlockKind::OrderedListItem | BlockKind::UnorderedListItem,
                ..
            } => self.prepare_list_item_children(),
            DrainDestination::Ancestor { .. } | DrainDestination::DocumentRoot => {
                self.close_for_block_sibling()
            }
        }
    }

    fn open_list_item_object(
        &mut self,
        kind: ListKind,
        level: u32,
        start: Option<u64>,
    ) -> Result<()> {
        self.json.open_object()?;
        let type_name = match kind {
            ListKind::Ordered => "numberedListItem",
            ListKind::Unordered => "bulletListItem",
        };
        self.json.key("type").value(type_name)?;
        let checked_start = start
            .map(|start_value| {
                u32::try_from(start_value).map_err(|err| Error::Other {
                    message: format!("ordered list start value out of range: {start_value}: {err}"),
                })
            })
            .transpose()?;
        self.list_stack.push(ListStackEntry {
            children_array_open: false,
            content_state: ListContentState::Pending,
            first_paragraph_consumed: false,
            kind,
            level,
            start: checked_start,
        });
        let ancestor_kind = match kind {
            ListKind::Ordered => BlockKind::OrderedListItem,
            ListKind::Unordered => BlockKind::UnorderedListItem,
        };
        self.open_block_ancestors.push(ancestor_kind);
        Ok(())
    }

    fn handle_end_document(&mut self) -> Result<()> {
        while !self.list_stack.is_empty() {
            self.close_current_list_item_object()?;
        }
        self.json.close_array()
    }

    fn write_id(&mut self, id: Option<&str>) -> Result<()> {
        if let Some(id_val) = id {
            self.json.key("id").value(id_val)?;
        }
        Ok(())
    }

    fn omit_unsupported_text_style(_style_name: &str) {}

    fn omit_future_text_style(_style: &TextStyleKind) {}

    fn should_buffer_for_lift(&self, event: &Event) -> bool {
        if self.table_depth.get() >= 2 || self.lifted_subtree_depth.is_positive() {
            return true;
        }
        if self.context.in_table_cell && Self::is_block_level_start(event) {
            return true;
        }
        if matches!(event, Event::Image { .. }) && self.context.in_table_cell {
            return true;
        }
        if matches!(event, Event::StartTable { .. }) && self.table_depth.is_positive() {
            return true;
        }
        false
    }

    fn update_lift_depth(&mut self, event: &Event) {
        match event {
            Event::StartTable { .. } => self.table_depth.inc(),
            Event::EndTable => self.table_depth.dec(),
            _ => {}
        }
        let non_leaf_block_start =
            Self::is_block_level_start(event) && !matches!(event, Event::ThematicBreak { .. });
        let non_leaf_block_end =
            Self::is_block_level_end(event) && !matches!(event, Event::ThematicBreak { .. });
        if non_leaf_block_start
            && ((self.context.in_table_cell && self.table_depth.get() == 1)
                || self.lifted_subtree_depth.is_positive())
        {
            self.lifted_subtree_depth.inc();
        }
        if non_leaf_block_end && self.lifted_subtree_depth.is_positive() {
            self.lifted_subtree_depth.dec();
        }
    }

    fn is_outermost_table_close(&self, event: &Event) -> bool {
        matches!(event, Event::EndTable) && self.table_depth.get() == 1
    }

    fn drain_lifted_nested_events(&mut self) -> Result<()> {
        let buffered = core::mem::take(&mut self.lifted_nested_events);
        if buffered.is_empty() {
            return Ok(());
        }
        match self.current_drain_destination() {
            DrainDestination::Ancestor {
                ancestor_index,
                kind: BlockKind::Blockquote,
            } => {
                let _ = ancestor_index;
                self.prepare_blockquote_children()?;
            }
            DrainDestination::Ancestor {
                ancestor_index,
                kind: BlockKind::OrderedListItem | BlockKind::UnorderedListItem,
            } => {
                let _ = ancestor_index;
                self.prepare_list_item_children()?;
            }
            DrainDestination::Ancestor { ancestor_index, .. } => {
                let _ = ancestor_index;
                self.close_for_block_sibling()?;
            }
            DrainDestination::DocumentRoot => {
                self.close_for_block_sibling()?;
            }
        }
        let rebase_offset = match self.current_drain_destination() {
            DrainDestination::Ancestor {
                kind: BlockKind::OrderedListItem | BlockKind::UnorderedListItem,
                ..
            } => self.compute_rebase_offset(&buffered),
            DrainDestination::Ancestor { .. } | DrainDestination::DocumentRoot => 0,
        };
        for ev in buffered {
            let rebased_event = if rebase_offset > 0 {
                match ev {
                    Event::StartOrderedListItem {
                        id,
                        level,
                        start,
                        style_type,
                    } => Event::StartOrderedListItem {
                        id,
                        level: level.saturating_add(rebase_offset),
                        start,
                        style_type,
                    },
                    Event::StartUnorderedListItem {
                        id,
                        level,
                        style_type,
                    } => Event::StartUnorderedListItem {
                        id,
                        level: level.saturating_add(rebase_offset),
                        style_type,
                    },
                    other => other,
                }
            } else {
                ev
            };
            self.handle_event(rebased_event)?;
        }
        Ok(())
    }

    fn is_block_level_start(event: &Event) -> bool {
        matches!(
            event,
            Event::StartHeading { .. }
                | Event::StartBlockQuote { .. }
                | Event::StartPreformatted { .. }
                | Event::StartOrderedListItem { .. }
                | Event::StartUnorderedListItem { .. }
                | Event::ThematicBreak { .. }
        )
    }

    fn is_block_level_end(event: &Event) -> bool {
        matches!(
            event,
            Event::EndHeading
                | Event::EndBlockQuote
                | Event::EndPreformatted
                | Event::EndOrderedListItem
                | Event::EndUnorderedListItem
                | Event::ThematicBreak { .. }
        )
    }
}

impl<W: Write> EventSink for BlockNoteWriter<W> {
    #[inline]
    fn finish(self) -> Result<()> {
        self.json.finish().map(|_| ())
    }

    #[inline]
    fn handle_event(&mut self, event: Event) -> Result<()> {
        if self.should_buffer_for_lift(&event) {
            self.update_lift_depth(&event);
            self.lifted_nested_events.push(event);
            return Ok(());
        }
        let is_outermost_table_close = self.is_outermost_table_close(&event);
        let result = match event {
            Event::StartDocument { .. } => self.json.open_array(),
            Event::EndDocument => self.handle_end_document(),
            Event::StartHeading { level, id, .. } => {
                self.prepare_for_child_block()?;
                self.handle_heading(level, id.as_deref())
            }
            Event::EndHeading | Event::EndPreformatted => {
                if !self.context.in_text_block {
                    return Ok(());
                }
                close_text_block!(self)
            }
            Event::StartParagraph { alignment, id } => {
                self.handle_paragraph(id.as_deref(), alignment.as_ref())
            }
            Event::EndParagraph => self.handle_end_paragraph(),
            Event::StartBlockQuote { id, .. } => {
                self.prepare_for_child_block()?;
                self.handle_blockquote(id.as_deref())
            }
            Event::EndBlockQuote => self.handle_end_blockquote(),
            Event::StartPreformatted { id, syntax, .. } => {
                self.prepare_for_child_block()?;
                self.handle_preformatted(id.as_deref(), syntax.as_deref())
            }
            Event::ThematicBreak { id, .. } => {
                self.prepare_for_child_block()?;
                self.handle_divider(id.as_deref())
            }
            Event::Text { content } => self.handle_text_event(&content),
            Event::StartTextStyle { kind, .. } => self.handle_start_text_style(kind),
            Event::EndTextStyle => self.handle_end_text_style(),
            Event::Image {
                source, alt, id, ..
            } => self.handle_image(source, alt, id.as_deref()),
            Event::LineBreak | Event::SoftBreak => self.handle_line_break(),
            Event::StartOrderedListItem { level, start, .. } => {
                self.handle_start_list_item(ListKind::Ordered, level, start)
            }
            Event::StartUnorderedListItem { level, .. } => {
                self.handle_start_list_item(ListKind::Unordered, level, None)
            }
            Event::EndOrderedListItem | Event::EndUnorderedListItem => self.handle_end_list_item(),
            Event::StartTable { id, .. } => self.handle_start_table(id.as_deref()),
            Event::EndTable => self.handle_end_table(),
            Event::StartTableRow { id, .. } => self.handle_start_table_row(id.as_deref()),
            Event::EndTableRow => self.handle_end_table_row(),
            event @ (Event::StartTableCell { .. } | Event::StartTableHeader { .. }) => {
                self.handle_start_table_cell_event(event)
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
        };
        if is_outermost_table_close {
            result?;
            return self.drain_lifted_nested_events();
        }
        result
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
    fn blockquote_state_stack_empty_after_new() {
        let mut buf = Vec::new();
        let writer = BlockNoteWriter::new(&mut buf);
        assert!(writer.blockquote_content_states.is_empty());
    }

    #[test]
    fn current_drain_destination_unit_walks_stack_top_to_bottom() {
        let mut buf = Vec::new();
        let mut writer = BlockNoteWriter::new(&mut buf);

        writer.open_block_ancestors.push(BlockKind::OrderedListItem);
        writer.open_block_ancestors.push(BlockKind::Table);
        writer.open_block_ancestors.push(BlockKind::Blockquote);

        assert!(matches!(
            writer.current_drain_destination(),
            DrainDestination::Ancestor {
                ancestor_index: 2,
                kind: BlockKind::Blockquote,
            }
        ));
    }

    #[test]
    fn current_drain_destination_skips_table_to_document_root() {
        let mut buf = Vec::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        writer.open_block_ancestors.push(BlockKind::Table);

        assert!(matches!(
            writer.current_drain_destination(),
            DrainDestination::DocumentRoot
        ));
    }

    #[test]
    fn compute_rebase_offset_without_list_items_is_zero() {
        let mut buf = Vec::new();
        let writer = BlockNoteWriter::new(&mut buf);

        assert_eq!(
            writer.compute_rebase_offset(&[Event::StartParagraph {
                alignment: None,
                id: None,
            }]),
            0
        );
    }

    #[test]
    fn compute_rebase_offset_without_destination_list_is_zero_for_level_zero() {
        let mut buf = Vec::new();
        let writer = BlockNoteWriter::new(&mut buf);

        assert_eq!(
            writer.compute_rebase_offset(&[Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            }]),
            0
        );
    }

    #[test]
    fn compute_rebase_offset_uses_current_list_stack_level() {
        let mut buf = Vec::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        writer.list_stack.push(ListStackEntry {
            children_array_open: true,
            content_state: ListContentState::Closed,
            first_paragraph_consumed: true,
            kind: ListKind::Unordered,
            level: 2,
            start: None,
        });

        assert_eq!(
            writer.compute_rebase_offset(&[
                Event::StartOrderedListItem {
                    id: None,
                    level: 4,
                    start: Some(1),
                    style_type: docspec_core::ListStyleType::Decimal,
                },
                Event::StartUnorderedListItem {
                    id: None,
                    level: 0,
                    style_type: docspec_core::ListStyleType::Disc,
                },
            ]),
            3
        );
    }

    #[test]
    fn compute_rebase_offset_does_not_lower_already_nested_items() {
        let mut buf = Vec::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        writer.list_stack.push(ListStackEntry {
            children_array_open: true,
            content_state: ListContentState::Closed,
            first_paragraph_consumed: true,
            kind: ListKind::Unordered,
            level: 1,
            start: None,
        });

        assert_eq!(
            writer.compute_rebase_offset(&[Event::StartUnorderedListItem {
                id: None,
                level: 4,
                style_type: docspec_core::ListStyleType::Disc,
            }]),
            0
        );
    }

    #[test]
    fn close_for_block_sibling_with_open_blockquote_prepares_children() {
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
            .handle_event(Event::StartBlockQuote { id: None })
            .is_ok());

        assert!(writer.close_for_block_sibling().is_ok());
        assert_eq!(
            writer.blockquote_content_states.last(),
            Some(&BlockquoteContentState::InChildren)
        );
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

#[cfg(test)]
mod accepts_child_tests {
    #![allow(clippy::bool_assert_comparison)]
    use super::*;
    use docspec_core::BlockKind;

    #[test]
    fn document_root_accepts_all_block_kinds() {
        assert_eq!(
            blocknote_accepts_child(None, ChildKind::Container(BlockKind::Blockquote)),
            true
        );
        assert_eq!(
            blocknote_accepts_child(None, ChildKind::Container(BlockKind::Caption)),
            true
        );
        assert_eq!(
            blocknote_accepts_child(None, ChildKind::Container(BlockKind::DefinitionDetail)),
            true
        );
        assert_eq!(
            blocknote_accepts_child(None, ChildKind::Container(BlockKind::DefinitionList)),
            true
        );
        assert_eq!(
            blocknote_accepts_child(None, ChildKind::Container(BlockKind::DefinitionTerm)),
            true
        );
        assert_eq!(
            blocknote_accepts_child(None, ChildKind::Container(BlockKind::Document)),
            true
        );
        assert_eq!(
            blocknote_accepts_child(None, ChildKind::Container(BlockKind::Footnote)),
            true
        );
        assert_eq!(
            blocknote_accepts_child(None, ChildKind::Container(BlockKind::Heading)),
            true
        );
        assert_eq!(
            blocknote_accepts_child(None, ChildKind::Container(BlockKind::Link)),
            true
        );
        assert_eq!(
            blocknote_accepts_child(None, ChildKind::Container(BlockKind::OrderedListItem)),
            true
        );
        assert_eq!(
            blocknote_accepts_child(None, ChildKind::Container(BlockKind::Paragraph)),
            true
        );
        assert_eq!(
            blocknote_accepts_child(None, ChildKind::Container(BlockKind::Preformatted)),
            true
        );
        assert_eq!(
            blocknote_accepts_child(None, ChildKind::Container(BlockKind::Table)),
            true
        );
        assert_eq!(
            blocknote_accepts_child(None, ChildKind::Container(BlockKind::TableCell)),
            true
        );
        assert_eq!(
            blocknote_accepts_child(None, ChildKind::Container(BlockKind::TableHeader)),
            true
        );
        assert_eq!(
            blocknote_accepts_child(None, ChildKind::Container(BlockKind::TableRow)),
            true
        );
        assert_eq!(
            blocknote_accepts_child(None, ChildKind::Container(BlockKind::UnorderedListItem)),
            true
        );
        assert_eq!(
            blocknote_accepts_child(None, ChildKind::Leaf(BlockLeafKind::Image)),
            true
        );
        assert_eq!(
            blocknote_accepts_child(None, ChildKind::Leaf(BlockLeafKind::Divider)),
            true
        );
    }

    #[test]
    fn heading_accepts_paragraph_child() {
        assert_eq!(
            blocknote_accepts_child(
                Some(BlockKind::Heading),
                ChildKind::Container(BlockKind::Paragraph),
            ),
            true
        );
    }

    #[test]
    fn list_item_accepts_heading_child() {
        assert_eq!(
            blocknote_accepts_child(
                Some(BlockKind::OrderedListItem),
                ChildKind::Container(BlockKind::Heading),
            ),
            true
        );
    }

    #[test]
    fn blockquote_accepts_list_item_child() {
        assert_eq!(
            blocknote_accepts_child(
                Some(BlockKind::Blockquote),
                ChildKind::Container(BlockKind::UnorderedListItem),
            ),
            true
        );
    }

    #[test]
    fn table_cell_rejects_heading() {
        assert_eq!(
            blocknote_accepts_child(
                Some(BlockKind::TableCell),
                ChildKind::Container(BlockKind::Heading),
            ),
            false
        );
    }

    #[test]
    fn table_cell_rejects_list_item() {
        assert_eq!(
            blocknote_accepts_child(
                Some(BlockKind::TableCell),
                ChildKind::Container(BlockKind::OrderedListItem),
            ),
            false
        );
    }

    #[test]
    fn code_block_rejects_heading() {
        assert_eq!(
            blocknote_accepts_child(
                Some(BlockKind::Preformatted),
                ChildKind::Container(BlockKind::Heading),
            ),
            false
        );
    }

    #[test]
    fn paragraph_rejects_blocks() {
        assert_eq!(
            blocknote_accepts_child(
                Some(BlockKind::Paragraph),
                ChildKind::Container(BlockKind::Blockquote),
            ),
            false
        );
    }

    #[test]
    fn heading_rejects_table_row_child() {
        assert_eq!(
            blocknote_accepts_child(
                Some(BlockKind::Heading),
                ChildKind::Container(BlockKind::TableRow),
            ),
            false
        );
    }

    #[test]
    fn heading_rejects_table_cell_child() {
        assert_eq!(
            blocknote_accepts_child(
                Some(BlockKind::Heading),
                ChildKind::Container(BlockKind::TableCell),
            ),
            false
        );
    }

    #[test]
    fn heading_rejects_table_header_child() {
        assert_eq!(
            blocknote_accepts_child(
                Some(BlockKind::Heading),
                ChildKind::Container(BlockKind::TableHeader),
            ),
            false
        );
    }

    #[test]
    fn heading_accepts_image_leaf() {
        assert_eq!(
            blocknote_accepts_child(
                Some(BlockKind::Heading),
                ChildKind::Leaf(BlockLeafKind::Image)
            ),
            true
        );
    }

    #[test]
    fn heading_accepts_divider_leaf() {
        assert_eq!(
            blocknote_accepts_child(
                Some(BlockKind::Heading),
                ChildKind::Leaf(BlockLeafKind::Divider),
            ),
            true
        );
    }

    #[test]
    fn blockquote_accepts_heading_child() {
        assert_eq!(
            blocknote_accepts_child(
                Some(BlockKind::Blockquote),
                ChildKind::Container(BlockKind::Heading),
            ),
            true
        );
    }

    #[test]
    fn blockquote_accepts_table_child() {
        assert_eq!(
            blocknote_accepts_child(
                Some(BlockKind::Blockquote),
                ChildKind::Container(BlockKind::Table),
            ),
            true
        );
    }

    #[test]
    fn blockquote_rejects_table_row_child() {
        assert_eq!(
            blocknote_accepts_child(
                Some(BlockKind::Blockquote),
                ChildKind::Container(BlockKind::TableRow),
            ),
            false
        );
    }

    #[test]
    fn blockquote_rejects_table_cell_child() {
        assert_eq!(
            blocknote_accepts_child(
                Some(BlockKind::Blockquote),
                ChildKind::Container(BlockKind::TableCell),
            ),
            false
        );
    }

    #[test]
    fn blockquote_rejects_table_header_child() {
        assert_eq!(
            blocknote_accepts_child(
                Some(BlockKind::Blockquote),
                ChildKind::Container(BlockKind::TableHeader),
            ),
            false
        );
    }

    #[test]
    fn ordered_list_item_accepts_image_leaf() {
        assert_eq!(
            blocknote_accepts_child(
                Some(BlockKind::OrderedListItem),
                ChildKind::Leaf(BlockLeafKind::Image),
            ),
            true
        );
    }

    #[test]
    fn ordered_list_item_rejects_table_row_child() {
        assert_eq!(
            blocknote_accepts_child(
                Some(BlockKind::OrderedListItem),
                ChildKind::Container(BlockKind::TableRow),
            ),
            false
        );
    }

    #[test]
    fn ordered_list_item_rejects_table_cell_child() {
        assert_eq!(
            blocknote_accepts_child(
                Some(BlockKind::OrderedListItem),
                ChildKind::Container(BlockKind::TableCell),
            ),
            false
        );
    }

    #[test]
    fn ordered_list_item_rejects_table_header_child() {
        assert_eq!(
            blocknote_accepts_child(
                Some(BlockKind::OrderedListItem),
                ChildKind::Container(BlockKind::TableHeader),
            ),
            false
        );
    }

    #[test]
    fn unordered_list_item_accepts_preformatted_child() {
        assert_eq!(
            blocknote_accepts_child(
                Some(BlockKind::UnorderedListItem),
                ChildKind::Container(BlockKind::Preformatted),
            ),
            true
        );
    }

    #[test]
    fn unordered_list_item_accepts_divider_leaf() {
        assert_eq!(
            blocknote_accepts_child(
                Some(BlockKind::UnorderedListItem),
                ChildKind::Leaf(BlockLeafKind::Divider),
            ),
            true
        );
    }

    #[test]
    fn unordered_list_item_rejects_table_row_child() {
        assert_eq!(
            blocknote_accepts_child(
                Some(BlockKind::UnorderedListItem),
                ChildKind::Container(BlockKind::TableRow),
            ),
            false
        );
    }

    #[test]
    fn unordered_list_item_rejects_table_cell_child() {
        assert_eq!(
            blocknote_accepts_child(
                Some(BlockKind::UnorderedListItem),
                ChildKind::Container(BlockKind::TableCell),
            ),
            false
        );
    }

    #[test]
    fn unordered_list_item_rejects_table_header_child() {
        assert_eq!(
            blocknote_accepts_child(
                Some(BlockKind::UnorderedListItem),
                ChildKind::Container(BlockKind::TableHeader),
            ),
            false
        );
    }

    #[test]
    fn caption_rejects_paragraph_child() {
        assert_eq!(
            blocknote_accepts_child(
                Some(BlockKind::Caption),
                ChildKind::Container(BlockKind::Paragraph),
            ),
            false
        );
    }

    #[test]
    fn definition_list_rejects_definition_term_child() {
        assert_eq!(
            blocknote_accepts_child(
                Some(BlockKind::DefinitionList),
                ChildKind::Container(BlockKind::DefinitionTerm),
            ),
            false
        );
    }

    #[test]
    fn definition_term_rejects_paragraph_child() {
        assert_eq!(
            blocknote_accepts_child(
                Some(BlockKind::DefinitionTerm),
                ChildKind::Container(BlockKind::Paragraph),
            ),
            false
        );
    }

    #[test]
    fn definition_detail_rejects_paragraph_child() {
        assert_eq!(
            blocknote_accepts_child(
                Some(BlockKind::DefinitionDetail),
                ChildKind::Container(BlockKind::Paragraph),
            ),
            false
        );
    }

    #[test]
    fn footnote_rejects_paragraph_child() {
        assert_eq!(
            blocknote_accepts_child(
                Some(BlockKind::Footnote),
                ChildKind::Container(BlockKind::Paragraph),
            ),
            false
        );
    }

    #[test]
    fn link_rejects_paragraph_child() {
        assert_eq!(
            blocknote_accepts_child(
                Some(BlockKind::Link),
                ChildKind::Container(BlockKind::Paragraph),
            ),
            false
        );
    }

    #[test]
    fn document_container_rejects_paragraph_child() {
        assert_eq!(
            blocknote_accepts_child(
                Some(BlockKind::Document),
                ChildKind::Container(BlockKind::Paragraph),
            ),
            false
        );
    }

    #[test]
    fn preformatted_rejects_image_leaf() {
        assert_eq!(
            blocknote_accepts_child(
                Some(BlockKind::Preformatted),
                ChildKind::Leaf(BlockLeafKind::Image),
            ),
            false
        );
    }

    #[test]
    fn paragraph_rejects_divider_leaf() {
        assert_eq!(
            blocknote_accepts_child(
                Some(BlockKind::Paragraph),
                ChildKind::Leaf(BlockLeafKind::Divider),
            ),
            false
        );
    }

    #[test]
    fn table_rejects_table_row_child() {
        assert_eq!(
            blocknote_accepts_child(
                Some(BlockKind::Table),
                ChildKind::Container(BlockKind::TableRow),
            ),
            false
        );
    }

    #[test]
    fn table_rejects_image_leaf() {
        assert_eq!(
            blocknote_accepts_child(
                Some(BlockKind::Table),
                ChildKind::Leaf(BlockLeafKind::Image)
            ),
            false
        );
    }

    #[test]
    fn table_row_rejects_table_cell_child() {
        assert_eq!(
            blocknote_accepts_child(
                Some(BlockKind::TableRow),
                ChildKind::Container(BlockKind::TableCell),
            ),
            false
        );
    }

    #[test]
    fn table_row_rejects_heading_child() {
        assert_eq!(
            blocknote_accepts_child(
                Some(BlockKind::TableRow),
                ChildKind::Container(BlockKind::Heading),
            ),
            false
        );
    }

    #[test]
    fn table_header_rejects_heading_child() {
        assert_eq!(
            blocknote_accepts_child(
                Some(BlockKind::TableHeader),
                ChildKind::Container(BlockKind::Heading),
            ),
            false
        );
    }

    #[test]
    fn table_header_rejects_divider_leaf() {
        assert_eq!(
            blocknote_accepts_child(
                Some(BlockKind::TableHeader),
                ChildKind::Leaf(BlockLeafKind::Divider),
            ),
            false
        );
    }
}
