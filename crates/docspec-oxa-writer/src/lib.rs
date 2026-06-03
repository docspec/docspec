#![forbid(unsafe_code)]
//! `DocSpec` event stream to `oxa.dev` JSON writer.
//!
//! This crate provides a streaming [`OxaWriter`] that implements [`EventSink`] to convert
//! `DocSpec` event streams into `oxa.dev` JSON format.
//!
//! # Design
//!
//! The writer emits JSON tokens directly to the underlying `Write` as events arrive using
//! `docspec-json` for streaming JSON output. Memory usage is constant regardless of document size.
//!
//! # Supported Events
//!
//! - `StartDocument` / `EndDocument` — document root
//! - `StartParagraph` / `EndParagraph` — paragraph blocks
//! - `Text` — inline text content
//!
//! # Example
//!
//! ```
//! use docspec_core::{Event, EventSink, Result, TextStyle};
//! use docspec_oxa_writer::OxaWriter;
//!
//! let mut buf = Vec::<u8>::new();
//! let mut writer = OxaWriter::new(&mut buf);
//! writer.handle_event(Event::StartDocument { id: None, language: None, metadata: None })?;
//! writer.handle_event(Event::StartParagraph { alignment: None, id: None })?;
//! writer.handle_event(Event::Text {
//!     content: "Hello".to_string(),
//!     style: TextStyle::default(),
//! })?;
//! writer.handle_event(Event::EndParagraph)?;
//! writer.handle_event(Event::EndDocument)?;
//! writer.finish()?;
//! let json = String::from_utf8(buf).map_err(|err| docspec_core::Error::Other {
//!     message: err.to_string(),
//! })?;
//! assert_eq!(
//!     json,
//!     r#"{"type":"Document","children":[{"type":"Paragraph","children":[{"type":"Text","value":"Hello"}]}]}"#
//! );
//! # Ok::<(), docspec_core::Error>(())
//! ```

use std::io::Write;

use docspec_core::{Event, EventSink, Result};
use docspec_json::{JsonEmitter, StrusonBackend};

/// Streaming writer that converts a `DocSpec` event stream into `oxa.dev` JSON.
///
/// # Supported Events
///
/// - `StartDocument` / `EndDocument`
/// - `StartParagraph` / `EndParagraph`
/// - `Text` (content only, styles dropped)
///
/// All other events are silently ignored (no error, no panic).
pub struct OxaWriter<W: Write> {
    document_open: bool,
    in_paragraph: bool,
    json: JsonEmitter<StrusonBackend<W>>,
}

impl<W: Write> OxaWriter<W> {
    /// Close the currently open paragraph, if any. No-op when no paragraph is open.
    fn close_paragraph(&mut self) -> Result<()> {
        if !self.in_paragraph {
            return Ok(());
        }
        self.json.close_array()?;
        self.json.close_object()?;
        self.in_paragraph = false;
        Ok(())
    }

    /// Create a new `OxaWriter` that writes to `writer`.
    #[inline]
    #[must_use]
    pub fn new(writer: W) -> Self {
        Self {
            document_open: false,
            in_paragraph: false,
            json: JsonEmitter::new(StrusonBackend::new(writer)),
        }
    }
}

impl<W: Write> EventSink for OxaWriter<W> {
    #[inline]
    fn finish(self) -> Result<()> {
        self.json.finish().map(|_| ())
    }

    #[inline]
    fn handle_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::StartDocument { .. } => {
                if self.document_open {
                    return Ok(());
                }
                self.json.open_object()?;
                self.json.key("type").value("Document")?;
                self.json.key("children").open_array()?;
                self.document_open = true;
                Ok(())
            }
            Event::EndDocument => {
                if !self.document_open {
                    return Ok(());
                }
                self.close_paragraph()?;
                self.json.close_array()?;
                self.json.close_object()?;
                self.document_open = false;
                Ok(())
            }
            Event::StartParagraph { .. } => {
                if !self.document_open || self.in_paragraph {
                    return Ok(());
                }
                self.json.open_object()?;
                self.json.key("type").value("Paragraph")?;
                self.json.key("children").open_array()?;
                self.in_paragraph = true;
                Ok(())
            }
            Event::EndParagraph => self.close_paragraph(),
            Event::Text { content, .. } => {
                if !self.in_paragraph {
                    return Ok(());
                }
                self.json.object(|j| {
                    j.key("type").value("Text")?;
                    j.key("value").value(content.as_str())
                })
            }
            Event::EndBlockQuote
            | Event::EndCaption
            | Event::EndDefinitionDetail
            | Event::EndDefinitionList
            | Event::EndDefinitionTerm
            | Event::EndFootnote
            | Event::EndHeading
            | Event::EndLink
            | Event::EndOrderedListItem
            | Event::EndPreformatted
            | Event::EndTable
            | Event::EndTableCell
            | Event::EndTableHeader
            | Event::EndTableRow
            | Event::EndUnorderedListItem
            | Event::FootnoteRef { .. }
            | Event::Image { .. }
            | Event::LineBreak
            | Event::SoftBreak
            | Event::StartBlockQuote { .. }
            | Event::StartCaption { .. }
            | Event::StartDefinitionDetail { .. }
            | Event::StartDefinitionList { .. }
            | Event::StartDefinitionTerm { .. }
            | Event::StartFootnote { .. }
            | Event::StartHeading { .. }
            | Event::StartLink { .. }
            | Event::StartOrderedListItem { .. }
            | Event::StartPreformatted { .. }
            | Event::StartTable { .. }
            | Event::StartTableCell { .. }
            | Event::StartTableHeader { .. }
            | Event::StartTableRow { .. }
            | Event::StartUnorderedListItem { .. }
            | Event::ThematicBreak { .. }
            | _ => Ok(()),
        }
    }
}
