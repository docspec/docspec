//! Stack tracking for block-level containers in the event stream.
//!
//! This module provides [`StackTrackingSink`], a wrapper around any [`EventSink`] that
//! maintains a stack of open block-level containers. This enables normalization logic
//! and well-formedness validation.

use alloc::vec::Vec;

use crate::{Error, Event, EventSink, Result};

/// Identifies the kind of block-level container in a document event stream.
///
/// Each variant corresponds to a Start/End event pair. The stack tracker uses this
/// to maintain the nesting structure as events flow through.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BlockKind {
    /// A block quote container.
    Blockquote,
    /// A table caption container.
    Caption,
    /// A definition detail (description) container.
    DefinitionDetail,
    /// A definition list container.
    DefinitionList,
    /// A definition term container.
    DefinitionTerm,
    /// The document root container.
    Document,
    /// A footnote definition container.
    Footnote,
    /// A heading container.
    Heading,
    /// A hyperlink container.
    Link,
    /// A list item container.
    ListItem,
    /// A paragraph container.
    Paragraph,
    /// A preformatted (code) block container.
    Preformatted,
    /// A table container.
    Table,
    /// A table data cell container.
    TableCell,
    /// A table header cell container.
    TableHeader,
    /// A table row container.
    TableRow,
}

/// A wrapper around any [`EventSink`] that tracks the nesting stack of open block-level containers
/// and performs event-stream normalization.
///
/// This sink maintains a stack of [`BlockKind`] values representing currently open containers.
/// Use the query methods to inspect the current nesting state.
///
/// # Normalization Behavior
///
/// - **Auto-insert**: If a [`Text`](Event::Text) event arrives outside any content-bearing block,
///   a [`StartParagraph`](Event::StartParagraph) is automatically inserted before it.
/// - **Auto-close**: On [`EndDocument`](Event::EndDocument), all remaining open blocks are closed
///   in reverse order. When an End event targets a block deeper in the stack, intervening blocks
///   are auto-closed first. Auto-inserted paragraphs are closed before new block-level Start events.
/// - **Validation**: An End event that does not match any open block on the stack returns
///   [`Error::InvalidSequence`](crate::Error::InvalidSequence).
pub struct StackTrackingSink<S: EventSink> {
    sink: S,
    stack: Vec<BlockKind>,
}

impl<S: EventSink> StackTrackingSink<S> {
    /// Returns `true` if the stack contains any content-bearing block.
    ///
    /// Content-bearing blocks are: [`BlockKind::Blockquote`], [`BlockKind::Heading`],
    /// [`BlockKind::Paragraph`], [`BlockKind::Preformatted`].
    #[inline]
    pub fn has_open_content(&self) -> bool {
        self.stack.iter().any(|kind| {
            matches!(
                kind,
                BlockKind::Blockquote
                    | BlockKind::Heading
                    | BlockKind::Paragraph
                    | BlockKind::Preformatted
            )
        })
    }

    /// Returns `true` if the given block kind is anywhere in the current nesting stack.
    #[inline]
    pub fn is_inside(&self, kind: BlockKind) -> bool {
        self.stack.contains(&kind)
    }

    /// Creates a new stack-tracking wrapper around the given sink.
    #[inline]
    pub fn new(sink: S) -> Self {
        Self {
            sink,
            stack: Vec::new(),
        }
    }

    /// Returns a slice of the current nesting stack.
    ///
    /// The first element is the outermost container (typically [`BlockKind::Document`]),
    /// and the last element is the innermost currently open container.
    #[inline]
    pub fn stack(&self) -> &[BlockKind] {
        &self.stack
    }
}

impl<S: EventSink> EventSink for StackTrackingSink<S> {
    #[inline]
    fn finish(self) -> Result<()> {
        self.sink.finish()
    }

    #[inline]
    fn handle_event(&mut self, event: Event) -> Result<()> {
        if let Some(kind) = block_kind_for_start(&event) {
            if kind != BlockKind::Link && self.stack.last() == Some(&BlockKind::Paragraph) {
                self.stack.pop();
                self.sink.handle_event(Event::EndParagraph)?;
            }
            self.stack.push(kind);
            return self.sink.handle_event(event);
        }

        if matches!(event, Event::EndDocument) {
            while let Some(kind) = self.stack.pop() {
                if kind != BlockKind::Document {
                    self.sink.handle_event(end_event_for(kind))?;
                }
            }
            return self.sink.handle_event(event);
        }

        if let Some(target_kind) = block_kind_for_end(&event) {
            if self.stack.is_empty() {
                return Err(Error::InvalidSequence {
                    expected: "open block".to_string(),
                    found: format!("{target_kind:?}"),
                    message: "received End event with empty stack".to_string(),
                });
            }

            if self.stack.contains(&target_kind) {
                while self.stack.last() != Some(&target_kind) {
                    if let Some(popped_kind) = self.stack.pop() {
                        self.sink.handle_event(end_event_for(popped_kind))?;
                    }
                }
                self.stack.pop();
                return self.sink.handle_event(event);
            }

            return Err(Error::InvalidSequence {
                expected: self
                    .stack
                    .last()
                    .map_or("empty".to_string(), |k| format!("{k:?}")),
                found: format!("{target_kind:?}"),
                message: format!("End event for {target_kind:?} does not match any open block"),
            });
        }

        if matches!(event, Event::ThematicBreak { .. })
            && self.stack.last() == Some(&BlockKind::Paragraph)
        {
            self.stack.pop();
            self.sink.handle_event(Event::EndParagraph)?;
        }

        if matches!(event, Event::Text { .. }) && !self.has_open_content() {
            let para = Event::StartParagraph {
                alignment: None,
                id: None,
            };
            self.stack.push(BlockKind::Paragraph);
            self.sink.handle_event(para)?;
        }

        self.sink.handle_event(event)
    }
}

/// Maps a Start event to its corresponding [`BlockKind`].
///
/// Returns `Some(kind)` for Start event variants, `None` for all other events.
#[inline]
#[must_use]
pub fn block_kind_for_start(event: &Event) -> Option<BlockKind> {
    if let Event::StartBlockQuote { .. } = event {
        Some(BlockKind::Blockquote)
    } else if let Event::StartCaption { .. } = event {
        Some(BlockKind::Caption)
    } else if let Event::StartDefinitionDetail { .. } = event {
        Some(BlockKind::DefinitionDetail)
    } else if let Event::StartDefinitionList { .. } = event {
        Some(BlockKind::DefinitionList)
    } else if let Event::StartDefinitionTerm { .. } = event {
        Some(BlockKind::DefinitionTerm)
    } else if let Event::StartDocument { .. } = event {
        Some(BlockKind::Document)
    } else if let Event::StartFootnote { .. } = event {
        Some(BlockKind::Footnote)
    } else if let Event::StartHeading { .. } = event {
        Some(BlockKind::Heading)
    } else if let Event::StartLink { .. } = event {
        Some(BlockKind::Link)
    } else if let Event::StartListItem { .. } = event {
        Some(BlockKind::ListItem)
    } else if let Event::StartParagraph { .. } = event {
        Some(BlockKind::Paragraph)
    } else if let Event::StartPreformatted { .. } = event {
        Some(BlockKind::Preformatted)
    } else if let Event::StartTable { .. } = event {
        Some(BlockKind::Table)
    } else if let Event::StartTableCell { .. } = event {
        Some(BlockKind::TableCell)
    } else if let Event::StartTableHeader { .. } = event {
        Some(BlockKind::TableHeader)
    } else if let Event::StartTableRow { .. } = event {
        Some(BlockKind::TableRow)
    } else {
        None
    }
}

/// Maps an End event to its corresponding [`BlockKind`].
///
/// Returns `Some(kind)` for End event variants, `None` for all other events.
#[inline]
#[must_use]
pub fn block_kind_for_end(event: &Event) -> Option<BlockKind> {
    if let Event::EndBlockQuote = event {
        Some(BlockKind::Blockquote)
    } else if let Event::EndCaption = event {
        Some(BlockKind::Caption)
    } else if let Event::EndDefinitionDetail = event {
        Some(BlockKind::DefinitionDetail)
    } else if let Event::EndDefinitionList = event {
        Some(BlockKind::DefinitionList)
    } else if let Event::EndDefinitionTerm = event {
        Some(BlockKind::DefinitionTerm)
    } else if let Event::EndDocument = event {
        Some(BlockKind::Document)
    } else if let Event::EndFootnote = event {
        Some(BlockKind::Footnote)
    } else if let Event::EndHeading = event {
        Some(BlockKind::Heading)
    } else if let Event::EndLink = event {
        Some(BlockKind::Link)
    } else if let Event::EndListItem = event {
        Some(BlockKind::ListItem)
    } else if let Event::EndParagraph = event {
        Some(BlockKind::Paragraph)
    } else if let Event::EndPreformatted = event {
        Some(BlockKind::Preformatted)
    } else if let Event::EndTable = event {
        Some(BlockKind::Table)
    } else if let Event::EndTableCell = event {
        Some(BlockKind::TableCell)
    } else if let Event::EndTableHeader = event {
        Some(BlockKind::TableHeader)
    } else if let Event::EndTableRow = event {
        Some(BlockKind::TableRow)
    } else {
        None
    }
}

/// Maps a [`BlockKind`] back to its corresponding End event.
///
/// This is the inverse of [`block_kind_for_end`]. Used when auto-closing blocks
/// that were not explicitly closed by the event stream.
#[inline]
#[must_use]
pub fn end_event_for(kind: BlockKind) -> Event {
    match kind {
        BlockKind::Blockquote => Event::EndBlockQuote,
        BlockKind::Caption => Event::EndCaption,
        BlockKind::DefinitionDetail => Event::EndDefinitionDetail,
        BlockKind::DefinitionList => Event::EndDefinitionList,
        BlockKind::DefinitionTerm => Event::EndDefinitionTerm,
        BlockKind::Document => Event::EndDocument,
        BlockKind::Footnote => Event::EndFootnote,
        BlockKind::Heading => Event::EndHeading,
        BlockKind::Link => Event::EndLink,
        BlockKind::ListItem => Event::EndListItem,
        BlockKind::Paragraph => Event::EndParagraph,
        BlockKind::Preformatted => Event::EndPreformatted,
        BlockKind::Table => Event::EndTable,
        BlockKind::TableCell => Event::EndTableCell,
        BlockKind::TableHeader => Event::EndTableHeader,
        BlockKind::TableRow => Event::EndTableRow,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockSink {
        events: Vec<Event>,
    }

    impl MockSink {
        fn new() -> Self {
            Self { events: Vec::new() }
        }
    }

    impl EventSink for MockSink {
        fn finish(self) -> Result<()> {
            Ok(())
        }

        fn handle_event(&mut self, event: Event) -> Result<()> {
            self.events.push(event);
            Ok(())
        }
    }

    fn send(sink: &mut StackTrackingSink<MockSink>, event: Event) {
        let result = sink.handle_event(event);
        assert!(matches!(result, Ok(())));
    }

    #[test]
    fn block_kind_clone() {
        let kind = BlockKind::Paragraph;
        let cloned = kind;
        assert_eq!(kind, cloned);
    }

    #[test]
    fn block_kind_copy() {
        let kind = BlockKind::Heading;
        let copied: BlockKind = kind;
        assert_eq!(kind, copied);
    }

    #[test]
    fn block_kind_debug() {
        let kind = BlockKind::Document;
        let debug_str = format!("{kind:?}");
        assert_eq!(debug_str, "Document");
    }

    #[test]
    fn block_kind_eq() {
        assert_eq!(BlockKind::Paragraph, BlockKind::Paragraph);
        assert_ne!(BlockKind::Paragraph, BlockKind::Heading);
    }

    #[test]
    fn block_kind_for_end_blockquote() {
        let event = Event::EndBlockQuote;
        assert_eq!(block_kind_for_end(&event), Some(BlockKind::Blockquote));
    }

    #[test]
    fn block_kind_for_end_caption() {
        let event = Event::EndCaption;
        assert_eq!(block_kind_for_end(&event), Some(BlockKind::Caption));
    }

    #[test]
    fn block_kind_for_end_definition_detail() {
        let event = Event::EndDefinitionDetail;
        assert_eq!(
            block_kind_for_end(&event),
            Some(BlockKind::DefinitionDetail)
        );
    }

    #[test]
    fn block_kind_for_end_definition_list() {
        let event = Event::EndDefinitionList;
        assert_eq!(block_kind_for_end(&event), Some(BlockKind::DefinitionList));
    }

    #[test]
    fn block_kind_for_end_definition_term() {
        let event = Event::EndDefinitionTerm;
        assert_eq!(block_kind_for_end(&event), Some(BlockKind::DefinitionTerm));
    }

    #[test]
    fn block_kind_for_end_document() {
        let event = Event::EndDocument;
        assert_eq!(block_kind_for_end(&event), Some(BlockKind::Document));
    }

    #[test]
    fn block_kind_for_end_footnote() {
        let event = Event::EndFootnote;
        assert_eq!(block_kind_for_end(&event), Some(BlockKind::Footnote));
    }

    #[test]
    fn block_kind_for_end_heading() {
        let event = Event::EndHeading;
        assert_eq!(block_kind_for_end(&event), Some(BlockKind::Heading));
    }

    #[test]
    fn block_kind_for_end_link() {
        let event = Event::EndLink;
        assert_eq!(block_kind_for_end(&event), Some(BlockKind::Link));
    }

    #[test]
    fn block_kind_for_end_list_item() {
        let event = Event::EndListItem;
        assert_eq!(block_kind_for_end(&event), Some(BlockKind::ListItem));
    }

    #[test]
    fn block_kind_for_end_paragraph() {
        let event = Event::EndParagraph;
        assert_eq!(block_kind_for_end(&event), Some(BlockKind::Paragraph));
    }

    #[test]
    fn block_kind_for_end_preformatted() {
        let event = Event::EndPreformatted;
        assert_eq!(block_kind_for_end(&event), Some(BlockKind::Preformatted));
    }

    #[test]
    fn block_kind_for_end_table() {
        let event = Event::EndTable;
        assert_eq!(block_kind_for_end(&event), Some(BlockKind::Table));
    }

    #[test]
    fn block_kind_for_end_table_cell() {
        let event = Event::EndTableCell;
        assert_eq!(block_kind_for_end(&event), Some(BlockKind::TableCell));
    }

    #[test]
    fn block_kind_for_end_table_header() {
        let event = Event::EndTableHeader;
        assert_eq!(block_kind_for_end(&event), Some(BlockKind::TableHeader));
    }

    #[test]
    fn block_kind_for_end_table_row() {
        let event = Event::EndTableRow;
        assert_eq!(block_kind_for_end(&event), Some(BlockKind::TableRow));
    }

    #[test]
    fn block_kind_for_end_text_returns_none() {
        let event = Event::Text {
            content: "hello".to_string(),
            bold: false,
            italic: false,
            code: false,
            strikethrough: false,
            underline: false,
            subscript: false,
            superscript: false,
            mark: None,
        };
        assert_eq!(block_kind_for_end(&event), None);
    }

    #[test]
    fn block_kind_for_start_blockquote() {
        let event = Event::StartBlockQuote { id: None };
        assert_eq!(block_kind_for_start(&event), Some(BlockKind::Blockquote));
    }

    #[test]
    fn block_kind_for_start_caption() {
        let event = Event::StartCaption { id: None };
        assert_eq!(block_kind_for_start(&event), Some(BlockKind::Caption));
    }

    #[test]
    fn block_kind_for_start_definition_detail() {
        let event = Event::StartDefinitionDetail { id: None };
        assert_eq!(
            block_kind_for_start(&event),
            Some(BlockKind::DefinitionDetail)
        );
    }

    #[test]
    fn block_kind_for_start_definition_list() {
        let event = Event::StartDefinitionList { id: None };
        assert_eq!(
            block_kind_for_start(&event),
            Some(BlockKind::DefinitionList)
        );
    }

    #[test]
    fn block_kind_for_start_definition_term() {
        let event = Event::StartDefinitionTerm { id: None };
        assert_eq!(
            block_kind_for_start(&event),
            Some(BlockKind::DefinitionTerm)
        );
    }

    #[test]
    fn block_kind_for_start_document() {
        let event = Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        };
        assert_eq!(block_kind_for_start(&event), Some(BlockKind::Document));
    }

    #[test]
    fn block_kind_for_start_footnote() {
        let event = Event::StartFootnote { id: 1 };
        assert_eq!(block_kind_for_start(&event), Some(BlockKind::Footnote));
    }

    #[test]
    fn block_kind_for_start_heading() {
        let event = Event::StartHeading { id: None, level: 1 };
        assert_eq!(block_kind_for_start(&event), Some(BlockKind::Heading));
    }

    #[test]
    fn block_kind_for_start_link() {
        let event = Event::StartLink {
            href: "https://example.com".to_string(),
            id: None,
            title: None,
        };
        assert_eq!(block_kind_for_start(&event), Some(BlockKind::Link));
    }

    #[test]
    fn block_kind_for_start_list_item() {
        let event = Event::StartListItem {
            id: None,
            level: 1,
            list_type: crate::ListType::Unordered,
            start: None,
            style_type: None,
        };
        assert_eq!(block_kind_for_start(&event), Some(BlockKind::ListItem));
    }

    #[test]
    fn block_kind_for_start_paragraph() {
        let event = Event::StartParagraph {
            alignment: None,
            id: None,
        };
        assert_eq!(block_kind_for_start(&event), Some(BlockKind::Paragraph));
    }

    #[test]
    fn block_kind_for_start_preformatted() {
        let event = Event::StartPreformatted {
            id: None,
            syntax: None,
        };
        assert_eq!(block_kind_for_start(&event), Some(BlockKind::Preformatted));
    }

    #[test]
    fn block_kind_for_start_table() {
        let event = Event::StartTable { id: None };
        assert_eq!(block_kind_for_start(&event), Some(BlockKind::Table));
    }

    #[test]
    fn block_kind_for_start_table_cell() {
        let event = Event::StartTableCell {
            colspan: None,
            id: None,
            rowspan: None,
        };
        assert_eq!(block_kind_for_start(&event), Some(BlockKind::TableCell));
    }

    #[test]
    fn block_kind_for_start_table_header() {
        let event = Event::StartTableHeader {
            abbr: None,
            colspan: None,
            id: None,
            rowspan: None,
            scope: None,
        };
        assert_eq!(block_kind_for_start(&event), Some(BlockKind::TableHeader));
    }

    #[test]
    fn block_kind_for_start_table_row() {
        let event = Event::StartTableRow { id: None };
        assert_eq!(block_kind_for_start(&event), Some(BlockKind::TableRow));
    }

    #[test]
    fn block_kind_for_start_text_returns_none() {
        let event = Event::Text {
            content: "hello".to_string(),
            bold: false,
            italic: false,
            code: false,
            strikethrough: false,
            underline: false,
            subscript: false,
            superscript: false,
            mark: None,
        };
        assert_eq!(block_kind_for_start(&event), None);
    }

    #[test]
    fn has_open_content_with_blockquote() {
        let mock = MockSink::new();
        let mut sink = StackTrackingSink::new(mock);
        sink.stack.push(BlockKind::Document);
        sink.stack.push(BlockKind::Blockquote);
        assert!(sink.has_open_content());
    }

    #[test]
    fn has_open_content_with_heading() {
        let mock = MockSink::new();
        let mut sink = StackTrackingSink::new(mock);
        sink.stack.push(BlockKind::Document);
        sink.stack.push(BlockKind::Heading);
        assert!(sink.has_open_content());
    }

    #[test]
    fn has_open_content_with_paragraph() {
        let mock = MockSink::new();
        let mut sink = StackTrackingSink::new(mock);
        sink.stack.push(BlockKind::Document);
        sink.stack.push(BlockKind::Paragraph);
        assert!(sink.has_open_content());
    }

    #[test]
    fn has_open_content_with_preformatted() {
        let mock = MockSink::new();
        let mut sink = StackTrackingSink::new(mock);
        sink.stack.push(BlockKind::Document);
        sink.stack.push(BlockKind::Preformatted);
        assert!(sink.has_open_content());
    }

    #[test]
    fn has_open_content_without_content_blocks() {
        let mock = MockSink::new();
        let mut sink = StackTrackingSink::new(mock);
        sink.stack.push(BlockKind::Document);
        sink.stack.push(BlockKind::Table);
        sink.stack.push(BlockKind::TableRow);
        assert!(!sink.has_open_content());
    }

    #[test]
    fn is_inside_finds_nested_kind() {
        let mock = MockSink::new();
        let mut sink = StackTrackingSink::new(mock);
        sink.stack.push(BlockKind::Document);
        sink.stack.push(BlockKind::Blockquote);
        sink.stack.push(BlockKind::Paragraph);
        assert!(sink.is_inside(BlockKind::Blockquote));
    }

    #[test]
    fn is_inside_returns_false_for_missing_kind() {
        let mock = MockSink::new();
        let mut sink = StackTrackingSink::new(mock);
        sink.stack.push(BlockKind::Document);
        sink.stack.push(BlockKind::Paragraph);
        assert!(!sink.is_inside(BlockKind::Blockquote));
    }

    #[test]
    fn new_creates_empty_stack() {
        let mock = MockSink::new();
        let sink = StackTrackingSink::new(mock);
        assert!(sink.stack().is_empty());
    }

    #[test]
    fn sink_finish_forwards_to_inner() {
        let mock = MockSink::new();
        let sink = StackTrackingSink::new(mock);
        let result = sink.finish();
        assert!(matches!(result, Ok(())));
    }

    #[test]
    fn sink_handle_event_forwards_to_inner() {
        let mock = MockSink::new();
        let mut sink = StackTrackingSink::new(mock);
        let event = Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        };
        let result = sink.handle_event(event);
        assert!(matches!(result, Ok(())));
    }

    #[test]
    fn stack_returns_current_stack() {
        let mock = MockSink::new();
        let mut sink = StackTrackingSink::new(mock);
        sink.stack.push(BlockKind::Document);
        sink.stack.push(BlockKind::Paragraph);
        let stack = sink.stack();
        assert_eq!(stack.len(), 2);
        assert_eq!(stack.first(), Some(&BlockKind::Document));
        assert_eq!(stack.get(1), Some(&BlockKind::Paragraph));
    }

    #[test]
    fn passthrough_forwards_all_events() {
        let mock = MockSink::new();
        let mut sink = StackTrackingSink::new(mock);

        send(
            &mut sink,
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
        );
        send(
            &mut sink,
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
        );
        send(
            &mut sink,
            Event::Text {
                content: "hello".to_string(),
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
            },
        );
        send(&mut sink, Event::EndParagraph);
        send(&mut sink, Event::EndDocument);

        assert_eq!(sink.sink.events.len(), 5);
        assert!(matches!(
            sink.sink.events.first(),
            Some(Event::StartDocument { .. })
        ));
        assert!(matches!(
            sink.sink.events.get(1),
            Some(Event::StartParagraph { .. })
        ));
        assert!(matches!(sink.sink.events.get(2), Some(Event::Text { .. })));
        assert!(matches!(sink.sink.events.get(3), Some(Event::EndParagraph)));
        assert!(matches!(sink.sink.events.get(4), Some(Event::EndDocument)));
        assert!(sink.stack().is_empty());
    }

    #[test]
    fn orphan_text_gets_paragraph() {
        let mock = MockSink::new();
        let mut sink = StackTrackingSink::new(mock);

        send(
            &mut sink,
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
        );
        send(
            &mut sink,
            Event::Text {
                content: "hello".to_string(),
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
            },
        );
        send(&mut sink, Event::EndDocument);

        assert_eq!(sink.sink.events.len(), 5);
        assert!(matches!(
            sink.sink.events.first(),
            Some(Event::StartDocument { .. })
        ));
        assert_eq!(
            sink.sink.events.get(1),
            Some(&Event::StartParagraph {
                alignment: None,
                id: None
            })
        );
        assert!(matches!(sink.sink.events.get(2), Some(Event::Text { .. })));
        assert_eq!(sink.sink.events.get(3), Some(&Event::EndParagraph));
        assert!(matches!(sink.sink.events.get(4), Some(Event::EndDocument)));
    }

    #[test]
    fn orphan_text_inside_table_cell_gets_paragraph() {
        let mock = MockSink::new();
        let mut sink = StackTrackingSink::new(mock);

        send(
            &mut sink,
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
        );
        send(&mut sink, Event::StartTable { id: None });
        send(&mut sink, Event::StartTableRow { id: None });
        send(
            &mut sink,
            Event::StartTableCell {
                colspan: None,
                id: None,
                rowspan: None,
            },
        );
        send(
            &mut sink,
            Event::Text {
                content: "cell".to_string(),
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
            },
        );
        send(&mut sink, Event::EndTableCell);
        send(&mut sink, Event::EndTableRow);
        send(&mut sink, Event::EndTable);
        send(&mut sink, Event::EndDocument);

        assert_eq!(sink.sink.events.len(), 11);
        assert_eq!(
            sink.sink.events.get(4),
            Some(&Event::StartParagraph {
                alignment: None,
                id: None
            })
        );
        assert_eq!(sink.sink.events.get(6), Some(&Event::EndParagraph));
    }

    #[test]
    fn text_inside_paragraph_no_extra_insert() {
        let mock = MockSink::new();
        let mut sink = StackTrackingSink::new(mock);

        send(
            &mut sink,
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
        );
        send(
            &mut sink,
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
        );
        send(
            &mut sink,
            Event::Text {
                content: "hello".to_string(),
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
            },
        );
        send(
            &mut sink,
            Event::Text {
                content: "world".to_string(),
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
            },
        );
        send(&mut sink, Event::EndParagraph);
        send(&mut sink, Event::EndDocument);

        assert_eq!(sink.sink.events.len(), 6);
    }

    #[test]
    fn stack_tracks_nesting() {
        let mock = MockSink::new();
        let mut sink = StackTrackingSink::new(mock);

        send(
            &mut sink,
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
        );
        send(&mut sink, Event::StartTable { id: None });
        send(&mut sink, Event::StartTableRow { id: None });
        send(
            &mut sink,
            Event::StartTableCell {
                colspan: None,
                id: None,
                rowspan: None,
            },
        );

        assert!(sink.is_inside(BlockKind::Document));
        assert!(sink.is_inside(BlockKind::Table));
        assert!(sink.is_inside(BlockKind::TableRow));
        assert!(sink.is_inside(BlockKind::TableCell));
        assert!(!sink.is_inside(BlockKind::Paragraph));
        assert!(!sink.has_open_content());
    }

    #[test]
    fn auto_close_paragraph_on_end_table() {
        let mock = MockSink::new();
        let mut sink = StackTrackingSink::new(mock);

        send(
            &mut sink,
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
        );
        send(&mut sink, Event::StartTable { id: None });
        send(&mut sink, Event::StartTableRow { id: None });
        send(
            &mut sink,
            Event::StartTableCell {
                colspan: None,
                id: None,
                rowspan: None,
            },
        );
        send(
            &mut sink,
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
        );
        send(
            &mut sink,
            Event::Text {
                content: "cell".to_string(),
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
            },
        );
        send(&mut sink, Event::EndTable);

        assert_eq!(sink.sink.events.len(), 10);
        assert_eq!(sink.sink.events.get(6), Some(&Event::EndParagraph));
        assert_eq!(sink.sink.events.get(7), Some(&Event::EndTableCell));
        assert_eq!(sink.sink.events.get(8), Some(&Event::EndTableRow));
        assert_eq!(sink.sink.events.get(9), Some(&Event::EndTable));
    }

    #[test]
    fn auto_close_on_end_blockquote() {
        let mock = MockSink::new();
        let mut sink = StackTrackingSink::new(mock);

        send(
            &mut sink,
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
        );
        send(&mut sink, Event::StartBlockQuote { id: None });
        send(
            &mut sink,
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
        );
        send(
            &mut sink,
            Event::Text {
                content: "quote".to_string(),
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
            },
        );
        send(&mut sink, Event::EndBlockQuote);

        assert_eq!(sink.sink.events.len(), 6);
        assert_eq!(sink.sink.events.get(4), Some(&Event::EndParagraph));
        assert_eq!(sink.sink.events.get(5), Some(&Event::EndBlockQuote));
    }

    #[test]
    fn end_document_closes_all() {
        let mock = MockSink::new();
        let mut sink = StackTrackingSink::new(mock);

        send(
            &mut sink,
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
        );
        send(&mut sink, Event::StartBlockQuote { id: None });
        send(
            &mut sink,
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
        );
        send(&mut sink, Event::EndDocument);

        assert_eq!(sink.sink.events.len(), 6);
        assert_eq!(sink.sink.events.get(3), Some(&Event::EndParagraph));
        assert_eq!(sink.sink.events.get(4), Some(&Event::EndBlockQuote));
        assert_eq!(sink.sink.events.get(5), Some(&Event::EndDocument));
        assert!(sink.stack().is_empty());
    }

    #[test]
    fn mismatched_end_returns_error() {
        let mock = MockSink::new();
        let mut sink = StackTrackingSink::new(mock);

        send(
            &mut sink,
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
        );
        send(&mut sink, Event::StartTable { id: None });

        let result = sink.handle_event(Event::EndBlockQuote);
        assert!(result.is_err());
        let err_str = format!("{result:?}");
        assert!(err_str.contains("Blockquote"));
    }

    #[test]
    fn end_without_start_returns_error() {
        let mock = MockSink::new();
        let mut sink = StackTrackingSink::new(mock);

        let result = sink.handle_event(Event::EndParagraph);
        assert!(result.is_err());
        let err_str = format!("{result:?}");
        assert!(err_str.contains("empty stack"));
    }

    #[test]
    fn end_event_for_all_kinds() {
        assert_eq!(end_event_for(BlockKind::Blockquote), Event::EndBlockQuote);
        assert_eq!(end_event_for(BlockKind::Caption), Event::EndCaption);
        assert_eq!(
            end_event_for(BlockKind::DefinitionDetail),
            Event::EndDefinitionDetail
        );
        assert_eq!(
            end_event_for(BlockKind::DefinitionList),
            Event::EndDefinitionList
        );
        assert_eq!(
            end_event_for(BlockKind::DefinitionTerm),
            Event::EndDefinitionTerm
        );
        assert_eq!(end_event_for(BlockKind::Document), Event::EndDocument);
        assert_eq!(end_event_for(BlockKind::Footnote), Event::EndFootnote);
        assert_eq!(end_event_for(BlockKind::Heading), Event::EndHeading);
        assert_eq!(end_event_for(BlockKind::Link), Event::EndLink);
        assert_eq!(end_event_for(BlockKind::ListItem), Event::EndListItem);
        assert_eq!(end_event_for(BlockKind::Paragraph), Event::EndParagraph);
        assert_eq!(
            end_event_for(BlockKind::Preformatted),
            Event::EndPreformatted
        );
        assert_eq!(end_event_for(BlockKind::Table), Event::EndTable);
        assert_eq!(end_event_for(BlockKind::TableCell), Event::EndTableCell);
        assert_eq!(end_event_for(BlockKind::TableHeader), Event::EndTableHeader);
        assert_eq!(end_event_for(BlockKind::TableRow), Event::EndTableRow);
    }
}
