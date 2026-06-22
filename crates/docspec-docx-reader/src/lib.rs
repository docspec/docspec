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
//! `<w:tc>`), lists (`<w:p>` with `<w:numPr>` — ordered and unordered),
//! hyperlinks (`<w:hyperlink>` — resolved via `word/_rels/document.xml.rels`
//! and emitted as `StartLink`/`EndLink` events around inline content),
//! structured document tags (`<w:sdt>` — content emitted normally;
//! `<w:sdtPr>`/`<w:sdtEndPr>` dropped), tracked insertions and moves
//! (`<w:ins>`, `<w:moveTo>` — accept-changes semantics), `DrawingML` images
//! (`<w:drawing>` — emitted as `Image` events; see the crate README for
//! `ImageSource` variants and `AssetHandle` usage), and VML images
//! (`<w:pict>` containing `<v:imagedata>`) — emitted as `Event::Image`.
//! Alt text precedence: `o:title` attribute on `<v:imagedata>` (non-empty) falls back to
//! `alt` attribute on the parent `<v:shape>`. Multiple `<v:imagedata>` elements in one
//! `<w:pict>` emit multiple `Image` events in document order. `<v:group>` wrappers are
//! transparent.
//! Emits: `StartDocument`, `StartParagraph`, `StartTextStyle`, `Text`,
//! `EndTextStyle`, `LineBreak`, `EndParagraph`, `StartTable`, `StartTableRow`,
//! `StartTableCell`, `StartTableHeader`, `EndTableHeader`, `EndTableCell`,
//! `EndTableRow`, `EndTable`, `StartLink`, `EndLink`, `StartOrderedListItem`,
//! `EndOrderedListItem`, `StartUnorderedListItem`, `EndUnorderedListItem`,
//! `Image`, `EndDocument`.
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
//! - `<mc:Fallback>` (in `<mc:AlternateContent>`) — subtree silently dropped to prevent
//!   duplicate `Image` events when both a modern `<w:drawing>` (in `<mc:Choice>`) and a
//!   legacy `<w:pict>` (in `<mc:Fallback>`) reference the same image.
//! - VML positioning attributes (`style="position:..."`, `w10:wrap`, `w10:anchorlock`,
//!   `<v:textbox>` text content) — silently dropped (consistent with `<w:drawing>`
//!   positioning loss for `<wp:anchor>`).
//! - VML image-bytes via `<w:binData>` (referenced from `<v:imagedata src="wordml://..."/>`)
//!   — not parsed; the `<v:imagedata>` degrades to "no rId found" and emits no event.
//! - Comments, footnotes, headers, footers
//! - Document metadata
//! - Tracked deletions (`<w:del>`, `<w:moveFrom>`) — accept-changes semantics
//! - Structured document tag properties (`<w:sdtPr>`, `<w:sdtEndPr>`)
//! - Field-code hyperlinks (`<w:fldChar>` + `<w:instrText>HYPERLINK ...`):
//!   legacy form not currently supported; only the modern `<w:hyperlink>`
//!   element is recognized.
//!
//! # Lists
//!
//! See the crate README for V1 list semantics and limitations.
//!
//! # Streaming Guarantee
//!
//! Memory use depends on which constructor you call:
//!
//! - **`from_path`**: streams `word/document.xml` in constant memory — O(1)
//!   regardless of document size. This is the recommended path for large files.
//! - **`from_reader`**: buffers `word/document.xml` into memory — O(N) in
//!   document size. Use `from_path` when constant memory is required.
//!
//! In both cases, `_rels/.rels` and `word/_rels/document.xml.rels` are fully
//! read into memory at package-open time (typical combined size < 10 KB even
//! for large documents). The internal event queue remains bounded regardless
//! of document size or hyperlink count.
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

mod asset_provider;
mod content_types;
mod document;
mod numbering;
mod package;
mod properties;
mod rels;
mod streaming_archive;
mod styles;
mod symbol_fonts;

use std::io::{BufReader, Read, Seek};
use std::path::Path;

pub use docspec_core::EventSource;

use docspec_core::Result;

const _: for<'a> fn(&'a content_types::ContentTypes, &str) -> Option<&'a str> =
    content_types::ContentTypes::lookup;
fn _image_rel_fields(r: &rels::ImageRel) -> (&str, bool) {
    (&r.target, r.is_external)
}
const _: for<'a> fn(&'a rels::ImageRel) -> (&'a str, bool) = _image_rel_fields;

/// A streaming DOCX reader that implements [`EventSource`].
///
/// `DocxReader` parses a DOCX archive and emits `DocSpec` events one at a time.
/// `<w:p>` paragraphs, `<w:t>` text, `<w:br>` line breaks, `<w:tab>` tabs,
/// table elements (`<w:tbl>`, `<w:tr>`, `<w:tc>`), and `DrawingML` images
/// (`<w:drawing>`) are recognized; all other elements are silently ignored.
///
/// # Streaming
///
/// Memory use depends on which constructor you call. `from_path` streams
/// `word/document.xml` in constant memory (O(1)). `from_reader` buffers
/// `word/document.xml` into memory (O(N) in document size). In both cases,
/// `_rels/.rels` and `word/_rels/document.xml.rels` are fully read into memory
/// at package-open time (typical combined size < 10 KB). The internal event
/// queue remains bounded regardless of document size or hyperlink count.
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
    /// Streams `word/document.xml` in constant memory (O(1) in document size).
    /// Prefer this over [`from_reader`](Self::from_reader) when memory is a concern.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if the file cannot be opened. See [`from_reader`](Self::from_reader)
    /// for additional error conditions.
    #[inline]
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self> {
        let (style_list, numbering, hyperlink_map, image_map, content_types, archive, stream) =
            package::open_package_from_path(path.as_ref())?;
        let xml = quick_xml::Reader::from_reader(BufReader::new(stream));
        let data = document::DocxData {
            style_list,
            hyperlink_map,
            numbering,
            image_map,
        };
        Ok(Self {
            inner: document::DocumentReader::from_xml_reader_and_archive(
                xml,
                data,
                archive,
                content_types,
            ),
        })
    }

    /// Creates a `DocxReader` from any `Read + Seek` source.
    ///
    /// The reader must be positioned at the start of a valid DOCX (ZIP) archive.
    ///
    /// **Memory note**: this constructor buffers `word/document.xml` into memory
    /// (O(N) in document size). Use [`from_path`](Self::from_path) for O(1)
    /// constant-memory streaming.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Parse`] if the input is not a valid ZIP archive, if
    /// `_rels/.rels` is missing or malformed, or if the document target entry
    /// cannot be opened. Returns [`Error::Io`] for I/O failures.
    #[inline]
    pub fn from_reader<R: Read + Seek + Send + 'static>(reader: R) -> Result<Self> {
        let (style_list, numbering, hyperlink_map, image_map, content_types, archive, stream) =
            package::open_package(reader)?;
        let xml = quick_xml::Reader::from_reader(BufReader::new(stream));
        let data = document::DocxData {
            style_list,
            hyperlink_map,
            numbering,
            image_map,
        };
        Ok(Self {
            inner: document::DocumentReader::from_xml_reader_and_archive(
                xml,
                data,
                archive,
                content_types,
            ),
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
