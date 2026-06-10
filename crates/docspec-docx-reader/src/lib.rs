#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
//! DOCX to `DocSpec` event stream reader.
//!
//! This crate provides a [`DocxReader`] that implements [`EventSource`] to convert
//! DOCX documents into the `DocSpec` event stream format. It uses `quick-xml` for
//! streaming XML parsing and `zip` for archive extraction.
//!
//! # Scope
//!
//! **In scope**: Paragraphs (`<w:p>`), direct text (`<w:t>` inside `<w:r>`),
//! line breaks (`<w:br>` — including `w:type="page"` and `w:type="column"`, all
//! emitted as `LineBreak`), tabs (`<w:tab>`, emitted as a `Text` event whose
//! content is the single character `"\t"`), tables (`<w:tbl>`, `<w:tr>`,
//! `<w:tc>`), hyperlinks (`<w:hyperlink>` — link text content is emitted as
//! plain runs), structured document tags (`<w:sdt>` — content emitted normally;
//! `<w:sdtPr>`/`<w:sdtEndPr>` dropped), and tracked insertions and moves
//! (`<w:ins>`, `<w:moveTo>` — accept-changes semantics).
//! Emits: `StartDocument`, `StartParagraph`, `StartTextStyle`, `Text`,
//! `EndTextStyle`, `LineBreak`, `EndParagraph`, `StartTable`, `StartTableRow`,
//! `StartTableCell`, `StartTableHeader`, `EndTableHeader`, `EndTableCell`,
//! `EndTableRow`, `EndTable`, `EndDocument`.
//!
//! The elements listed under "Out of scope" are the reader's denylist — their
//! entire subtree is silently dropped. Every other element (known or unknown)
//! is parsed normally; the reader continues into its children.
//!
//! **Out of scope (subtree silently dropped)**:
//! - Run styling not listed in the crate README
//! - Headings (any `<w:pStyle>` value — every paragraph is `StartParagraph`)
//! - Vertical cell merging (`<w:vMerge>`) — every cell emits with
//!   `rowspan: None`
//! - Header rows in nested tables — only the outermost table honors
//!   `<w:tblHeader>`
//! - Table-level property exceptions (`<w:tblPrEx>`) — silently ignored
//! - Table, row, and cell visual properties (`<w:tblPr>`, `<w:trPr>` visual
//!   fields, `<w:tcPr>` visual fields, `<w:tblGrid>`)
//! - Lists
//! - Drawings and images (`<w:drawing>`, `<w:pict>`)
//! - Comments, footnotes, headers, footers
//! - Document metadata
//! - Tracked deletions (`<w:del>`, `<w:moveFrom>`) — accept-changes semantics
//! - Structured document tag properties (`<w:sdtPr>`, `<w:sdtEndPr>`)
//!
//! # Streaming Guarantee
//!
//! `DocxReader` streams `document.xml` event by event using constant memory
//! regardless of document size. Only `_rels/.rels` (a few hundred bytes) is
//! fully read into memory to discover the document target path.
//!
//! # Quick Start
//!
//! ```no_run
//! use docspec_docx_reader::{DocxReader, EventSource};
//!
//! let mut reader = DocxReader::from_path("document.docx")?;
//! while let Some(event) = reader.next_event()? {
//!     println!("{event:?}");
//! }
//! # Ok::<(), docspec_core::Error>(())
//! ```

extern crate alloc;

mod document;
mod package;
mod properties;
mod rels;
mod styles;
mod symbol_fonts;

use std::io::{BufReader, Read, Seek};
use std::path::Path;

pub use docspec_core::EventSource;
use docspec_core::{Error, Result};

/// A streaming DOCX reader that implements [`EventSource`].
///
/// `DocxReader` parses a DOCX archive and emits `DocSpec` events one at a time.
/// `<w:p>` paragraphs, `<w:t>` text, `<w:br>` line breaks, `<w:tab>` tabs, and
/// table elements (`<w:tbl>`, `<w:tr>`, `<w:tc>`) are recognized; all other
/// elements are silently ignored.
///
/// # Streaming
///
/// The reader streams `document.xml` event by event using constant memory.
/// Only `_rels/.rels` (a few hundred bytes) is buffered to discover the
/// document target path.
///
/// # Errors
///
/// Returns [`Error::Io`] for I/O failures and [`Error::Parse`] for malformed
/// archives or XML.
#[derive(Debug)]
pub struct DocxReader {
    inner: document::DocumentReader,
}

impl DocxReader {
    /// Creates a `DocxReader` from a file path.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if the file cannot be opened. See [`from_reader`](Self::from_reader)
    /// for additional error conditions.
    #[inline]
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = std::fs::File::open(path.as_ref()).map_err(Error::from)?;
        Self::from_reader(file)
    }

    /// Creates a `DocxReader` from any `Read + Seek` source.
    ///
    /// The reader must be positioned at the start of a valid DOCX (ZIP) archive.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Parse`] if the input is not a valid ZIP archive, if
    /// `_rels/.rels` is missing or malformed, or if the document target entry
    /// cannot be opened. Returns [`Error::Io`] for I/O failures.
    #[inline]
    pub fn from_reader<R: Read + Seek + Send + 'static>(reader: R) -> Result<Self> {
        let (style_list, stream) = package::open_package(reader)?;
        let xml = quick_xml::Reader::from_reader(BufReader::new(stream));
        let data = document::DocxData { style_list };
        Ok(Self {
            inner: document::DocumentReader::from_xml_reader(xml, data),
        })
    }
}

impl EventSource for DocxReader {
    #[inline]
    fn next_event(&mut self) -> Result<Option<docspec_core::Event>> {
        self.inner.next_event()
    }
}

#[cfg(test)]
#[cfg(not(coverage))]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic)]
    use super::*;

    #[test]
    fn docx_reader_is_send_static() {
        fn assert_send_static<T: Send + 'static>() {}
        assert_send_static::<DocxReader>();
    }

    #[test]
    fn docx_without_styles_emits_only_paragraphs() {
        use std::io::{Cursor, Write as _};
        use zip::ZipWriter;

        let root_rels = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#;
        let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:r><w:t>hi</w:t></w:r></w:p>
  </w:body>
</w:document>"#;

        let buf = Cursor::new(Vec::new());
        let mut writer = ZipWriter::new(buf);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        writer.start_file("_rels/.rels", options).unwrap();
        writer.write_all(root_rels.as_bytes()).unwrap();
        writer.start_file("word/document.xml", options).unwrap();
        writer.write_all(document_xml.as_bytes()).unwrap();
        let zip_bytes = writer.finish().unwrap().into_inner();

        let mut reader = DocxReader::from_reader(Cursor::new(zip_bytes)).unwrap();
        let mut events = Vec::new();
        loop {
            match reader.next_event() {
                Ok(Some(event)) => events.push(event),
                Ok(None) => break,
                Err(err) => panic!("unexpected error: {err:?}"),
            }
        }

        assert_eq!(
            events,
            vec![
                docspec_core::Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "hi".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndDocument,
            ]
        );
    }
}
