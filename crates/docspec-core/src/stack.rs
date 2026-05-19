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
    document_finished: bool,
    sink: S,
    stack: Vec<BlockKind>,
}

impl<S: EventSink> StackTrackingSink<S> {
    /// Returns `true` if the stack contains any content-bearing block.
    ///
    /// Content-bearing blocks are: [`BlockKind::Heading`], [`BlockKind::Paragraph`],
    /// [`BlockKind::Preformatted`], [`BlockKind::Link`], [`BlockKind::DefinitionTerm`].
    ///
    /// Note: [`BlockKind::Blockquote`] is NOT content-bearing because block quotes contain
    /// block elements (paragraphs, headings, etc.), not inline text directly. Text inside
    /// a block quote without an explicit paragraph triggers auto-paragraph insertion.
    ///
    /// Note: [`BlockKind::ListItem`], [`BlockKind::TableCell`], [`BlockKind::TableHeader`],
    /// and [`BlockKind::DefinitionDetail`] are NOT content-bearing despite being able to
    /// contain inline content per `EVENTS.md`. This is because downstream writers like
    /// `BlockNoteWriter` rely on auto-paragraph insertion for these container types.
    #[inline]
    pub fn has_open_content(&self) -> bool {
        self.stack.iter().any(|kind| {
            matches!(
                kind,
                BlockKind::Heading
                    | BlockKind::Paragraph
                    | BlockKind::Preformatted
                    | BlockKind::Link
                    | BlockKind::DefinitionTerm
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
            document_finished: false,
            sink,
            stack: Vec::new(),
        }
    }

    /// Returns a reference to the inner sink.
    #[cfg(test)]
    fn sink(&self) -> &S {
        &self.sink
    }

    /// Returns a slice of the current nesting stack.
    ///
    /// The first element is the outermost container (typically [`BlockKind::Document`]),
    /// and the last element is the innermost currently open container.
    #[inline]
    pub fn stack(&self) -> &[BlockKind] {
        &self.stack
    }

    /// Returns a mutable reference to the stack.
    #[cfg(test)]
    fn stack_mut(&mut self) -> &mut Vec<BlockKind> {
        &mut self.stack
    }
}

impl<S: EventSink> EventSink for StackTrackingSink<S> {
    #[inline]
    fn finish(self) -> Result<()> {
        self.sink.finish()
    }

    #[inline]
    fn handle_event(&mut self, event: Event) -> Result<()> {
        // Reject all events after document has finished (except we already handle StartDocument
        // specially below for a clearer error message)
        if self.document_finished && !matches!(event, Event::StartDocument { .. }) {
            return Err(Error::InvalidSequence {
                expected: "end of stream".to_string(),
                found: format!("{event:?}"),
                message: "event received after document already finished".to_string(),
            });
        }

        if let Some(kind) = block_kind_for_start(&event) {
            if kind == BlockKind::Document {
                if self.stack.contains(&BlockKind::Document) {
                    return Err(Error::InvalidSequence {
                        expected: "single Document".to_string(),
                        found: "StartDocument".to_string(),
                        message: "StartDocument received while Document already open".to_string(),
                    });
                }
                if self.document_finished {
                    return Err(Error::InvalidSequence {
                        expected: "end of stream".to_string(),
                        found: "StartDocument".to_string(),
                        message: "StartDocument received after document already finished"
                            .to_string(),
                    });
                }
            }
            // Links cannot nest per EVENTS.md
            if kind == BlockKind::Link && self.stack.contains(&BlockKind::Link) {
                return Err(Error::InvalidSequence {
                    expected: "no nested links".to_string(),
                    found: "StartLink".to_string(),
                    message: "StartLink received while another link is already open".to_string(),
                });
            }
            if kind != BlockKind::Link && self.stack.last() == Some(&BlockKind::Paragraph) {
                self.stack.pop();
                self.sink.handle_event(Event::EndParagraph)?;
            }
            self.stack.push(kind);
            return self.sink.handle_event(event);
        }

        if matches!(event, Event::EndDocument) {
            if !self.stack.contains(&BlockKind::Document) {
                return Err(Error::InvalidSequence {
                    expected: "open Document".to_string(),
                    found: "EndDocument".to_string(),
                    message: "EndDocument received without StartDocument".to_string(),
                });
            }
            while let Some(kind) = self.stack.pop() {
                if kind != BlockKind::Document {
                    self.sink.handle_event(end_event_for(kind))?;
                }
            }
            self.document_finished = true;
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
    use alloc::vec::Vec;

    use super::*;
    use crate::TextStyle;

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
        assert!(result.is_ok());
    }

    #[test]
    fn has_open_content_with_blockquote_returns_false() {
        let mock = MockSink::new();
        let mut sink = StackTrackingSink::new(mock);
        sink.stack_mut().push(BlockKind::Document);
        sink.stack_mut().push(BlockKind::Blockquote);
        // Blockquote is NOT content-bearing - it contains block elements, not inline text
        assert!(!sink.has_open_content());
    }

    #[test]
    fn has_open_content_with_heading() {
        let mock = MockSink::new();
        let mut sink = StackTrackingSink::new(mock);
        sink.stack_mut().push(BlockKind::Document);
        sink.stack_mut().push(BlockKind::Heading);
        assert!(sink.has_open_content());
    }

    #[test]
    fn has_open_content_with_paragraph() {
        let mock = MockSink::new();
        let mut sink = StackTrackingSink::new(mock);
        sink.stack_mut().push(BlockKind::Document);
        sink.stack_mut().push(BlockKind::Paragraph);
        assert!(sink.has_open_content());
    }

    #[test]
    fn has_open_content_with_preformatted() {
        let mock = MockSink::new();
        let mut sink = StackTrackingSink::new(mock);
        sink.stack_mut().push(BlockKind::Document);
        sink.stack_mut().push(BlockKind::Preformatted);
        assert!(sink.has_open_content());
    }

    #[test]
    fn has_open_content_without_content_blocks() {
        let mock = MockSink::new();
        let mut sink = StackTrackingSink::new(mock);
        sink.stack_mut().push(BlockKind::Document);
        sink.stack_mut().push(BlockKind::Table);
        sink.stack_mut().push(BlockKind::TableRow);
        assert!(!sink.has_open_content());
    }

    #[test]
    fn is_inside_finds_nested_kind() {
        let mock = MockSink::new();
        let mut sink = StackTrackingSink::new(mock);
        sink.stack_mut().push(BlockKind::Document);
        sink.stack_mut().push(BlockKind::Blockquote);
        sink.stack_mut().push(BlockKind::Paragraph);
        assert!(sink.is_inside(BlockKind::Blockquote));
    }

    #[test]
    fn is_inside_returns_false_for_missing_kind() {
        let mock = MockSink::new();
        let mut sink = StackTrackingSink::new(mock);
        sink.stack_mut().push(BlockKind::Document);
        sink.stack_mut().push(BlockKind::Paragraph);
        assert!(!sink.is_inside(BlockKind::Blockquote));
    }

    #[test]
    fn stack_returns_current_stack() {
        let mock = MockSink::new();
        let mut sink = StackTrackingSink::new(mock);
        sink.stack_mut().push(BlockKind::Document);
        sink.stack_mut().push(BlockKind::Paragraph);
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
                style: TextStyle::default(),
            },
        );
        send(&mut sink, Event::EndParagraph);
        send(&mut sink, Event::EndDocument);

        assert_eq!(sink.sink().events.len(), 5);
        assert!(matches!(
            sink.sink().events.first(),
            Some(Event::StartDocument { .. })
        ));
        assert!(matches!(
            sink.sink().events.get(1),
            Some(Event::StartParagraph { .. })
        ));
        assert!(matches!(
            sink.sink().events.get(2),
            Some(Event::Text { .. })
        ));
        assert!(matches!(
            sink.sink().events.get(3),
            Some(Event::EndParagraph)
        ));
        assert!(matches!(
            sink.sink().events.get(4),
            Some(Event::EndDocument)
        ));
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
                style: TextStyle::default(),
            },
        );
        send(&mut sink, Event::EndDocument);

        assert_eq!(sink.sink().events.len(), 5);
        assert!(matches!(
            sink.sink().events.first(),
            Some(Event::StartDocument { .. })
        ));
        assert_eq!(
            sink.sink().events.get(1),
            Some(&Event::StartParagraph {
                alignment: None,
                id: None
            })
        );
        assert!(matches!(
            sink.sink().events.get(2),
            Some(Event::Text { .. })
        ));
        assert_eq!(sink.sink().events.get(3), Some(&Event::EndParagraph));
        assert!(matches!(
            sink.sink().events.get(4),
            Some(Event::EndDocument)
        ));
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
                style: TextStyle::default(),
            },
        );
        send(&mut sink, Event::EndTableCell);
        send(&mut sink, Event::EndTableRow);
        send(&mut sink, Event::EndTable);
        send(&mut sink, Event::EndDocument);

        assert_eq!(sink.sink().events.len(), 11);
        assert_eq!(
            sink.sink().events.get(4),
            Some(&Event::StartParagraph {
                alignment: None,
                id: None
            })
        );
        assert_eq!(sink.sink().events.get(6), Some(&Event::EndParagraph));
    }

    #[test]
    fn orphan_text_inside_blockquote_gets_paragraph() {
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
            Event::Text {
                content: "quoted".to_string(),
                style: TextStyle::default(),
            },
        );
        send(&mut sink, Event::EndBlockQuote);
        send(&mut sink, Event::EndDocument);

        // Should auto-insert paragraph around orphan text inside blockquote
        assert_eq!(sink.sink().events.len(), 7);
        assert!(matches!(
            sink.sink().events.first(),
            Some(Event::StartDocument { .. })
        ));
        assert!(matches!(
            sink.sink().events.get(1),
            Some(Event::StartBlockQuote { .. })
        ));
        assert_eq!(
            sink.sink().events.get(2),
            Some(&Event::StartParagraph {
                alignment: None,
                id: None
            })
        );
        assert!(matches!(
            sink.sink().events.get(3),
            Some(Event::Text { .. })
        ));
        assert_eq!(sink.sink().events.get(4), Some(&Event::EndParagraph));
        assert!(matches!(
            sink.sink().events.get(5),
            Some(Event::EndBlockQuote)
        ));
        assert!(matches!(
            sink.sink().events.get(6),
            Some(Event::EndDocument)
        ));
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
                style: TextStyle::default(),
            },
        );
        send(
            &mut sink,
            Event::Text {
                content: "world".to_string(),
                style: TextStyle::default(),
            },
        );
        send(&mut sink, Event::EndParagraph);
        send(&mut sink, Event::EndDocument);

        assert_eq!(sink.sink().events.len(), 6);
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
                style: TextStyle::default(),
            },
        );
        send(&mut sink, Event::EndTable);

        assert_eq!(sink.sink().events.len(), 10);
        assert_eq!(sink.sink().events.get(6), Some(&Event::EndParagraph));
        assert_eq!(sink.sink().events.get(7), Some(&Event::EndTableCell));
        assert_eq!(sink.sink().events.get(8), Some(&Event::EndTableRow));
        assert_eq!(sink.sink().events.get(9), Some(&Event::EndTable));
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
                style: TextStyle::default(),
            },
        );
        send(&mut sink, Event::EndBlockQuote);

        assert_eq!(sink.sink().events.len(), 6);
        assert_eq!(sink.sink().events.get(4), Some(&Event::EndParagraph));
        assert_eq!(sink.sink().events.get(5), Some(&Event::EndBlockQuote));
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

        assert_eq!(sink.sink().events.len(), 6);
        assert_eq!(sink.sink().events.get(3), Some(&Event::EndParagraph));
        assert_eq!(sink.sink().events.get(4), Some(&Event::EndBlockQuote));
        assert_eq!(sink.sink().events.get(5), Some(&Event::EndDocument));
        assert!(sink.stack().is_empty());
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
