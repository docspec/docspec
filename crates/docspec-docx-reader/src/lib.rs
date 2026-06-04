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
//! **In scope**: Paragraphs (`<w:p>`) and direct text (`<w:t>` inside `<w:r>`).
//! Emits exactly: `StartDocument`, `StartParagraph`, `Text`, `EndParagraph`, `EndDocument`.
//!
//! **Out of scope (silently dropped)**:
//! - Run styling (`<w:rPr>`, bold, italic, etc.)
//! - Line and page breaks (`<w:br>`)
//! - Tabs (`<w:tab>`)
//! - Headings (any `<w:pStyle>` value — every paragraph is `StartParagraph`)
//! - Tables (`<w:tbl>`, `<w:tr>`, `<w:tc>`)
//! - Lists
//! - Hyperlinks (`<w:hyperlink>`)
//! - Drawings and images (`<w:drawing>`, `<w:pict>`)
//! - Structured document tags (`<w:sdt>`)
//! - Comments, footnotes, headers, footers
//! - Document metadata
//! - Tracked changes (`<w:ins>`, `<w:del>`, `<w:moveFrom>`, `<w:moveTo>`)
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

mod rels;

use alloc::collections::VecDeque;
use core::fmt;
use std::io::{BufReader, Read, Seek};
use std::path::Path;

pub use docspec_core::EventSource;
use docspec_core::{Error, Event, Result};
use quick_xml::events::{BytesCData, BytesRef, BytesText};

/// Document processing phase.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// `EndDocument` has been emitted.
    Finished,
    /// `StartDocument` not yet emitted.
    NotStarted,
    /// Processing events between `StartDocument` and `EndDocument`.
    Running,
}

/// A streaming DOCX reader that implements [`EventSource`].
///
/// `DocxReader` parses a DOCX archive and emits `DocSpec` events one at a time.
/// Only `<w:p>` paragraph elements and `<w:t>` text elements are recognized;
/// all other elements are silently ignored.
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
pub struct DocxReader {
    /// Reusable buffer for quick-xml event reading.
    buf: Vec<u8>,
    /// Depth counter for ignored subtrees (tables, tracked changes, etc.).
    /// Incremented on Start of an ignored container, decremented on End.
    in_ignored_subtree: u32,
    /// Whether the reader is currently inside a `<w:p>` element.
    in_paragraph: bool,
    /// Whether the reader is currently inside a `<w:t>` element.
    in_text: bool,
    /// Text collected for the current `<w:t>` element.
    pending_text: String,
    /// Document processing phase.
    phase: Phase,
    /// Queue of `DocSpec` events to emit.
    queue: VecDeque<Event>,
    /// The quick-xml reader streaming from the document entry.
    xml: quick_xml::Reader<BufReader<Box<dyn Read>>>,
}

impl fmt::Debug for DocxReader {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DocxReader")
            .field("buf", &self.buf)
            .field("in_ignored_subtree", &self.in_ignored_subtree)
            .field("in_paragraph", &self.in_paragraph)
            .field("in_text", &self.in_text)
            .field("pending_text", &self.pending_text)
            .field("phase", &"<phase>")
            .field("queue", &self.queue)
            .field("xml", &"<quick_xml::Reader>")
            .finish()
    }
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
    pub fn from_reader<R: Read + Seek + 'static>(mut reader: R) -> Result<Self> {
        let mut archive = zip::ZipArchive::new(&mut reader).map_err(|err| match err {
            zip::result::ZipError::InvalidArchive(_)
            | zip::result::ZipError::UnsupportedArchive(_) => Error::Parse {
                message: "not a valid ZIP archive".to_string(),
                position: None,
            },
            zip::result::ZipError::Io(source) => Error::Io { source },
            zip::result::ZipError::FileNotFound
            | zip::result::ZipError::InvalidPassword
            | zip::result::ZipError::CompressionMethodNotSupported(_)
            | _ => parse_error(format!("not a valid ZIP archive: {err}")),
        })?;

        let document_path = rels::find_document_path(&mut archive)?;

        let (data_start, compressed_size, method) = {
            let entry = archive
                .by_name(&document_path)
                .map_err(|_err| Error::Parse {
                    message: format!("document target not found: {document_path}"),
                    position: None,
                })?;
            let data_start = entry
                .data_start()
                .ok_or_else(|| parse_error("document.xml has no data offset".to_string()))?;
            (data_start, entry.compressed_size(), entry.compression())
        };
        drop(archive);

        reader
            .seek(std::io::SeekFrom::Start(data_start))
            .map_err(Error::from)?;

        let limited = reader.take(compressed_size);

        let stream: Box<dyn Read> = if method == zip::CompressionMethod::Stored {
            Box::new(limited)
        } else if method == zip::CompressionMethod::Deflated {
            Box::new(flate2::read::DeflateDecoder::new(limited))
        } else {
            return Err(Error::Parse {
                message: format!("unsupported compression: {method:?}"),
                position: None,
            });
        };

        let xml = quick_xml::Reader::from_reader(BufReader::new(stream));

        Ok(Self {
            buf: Vec::with_capacity(4096),
            in_ignored_subtree: 0,
            in_paragraph: false,
            in_text: false,
            pending_text: String::new(),
            phase: Phase::NotStarted,
            queue: VecDeque::new(),
            xml,
        })
    }
}

impl DocxReader {
    fn can_collect_text(&self) -> bool {
        self.in_ignored_subtree == 0 && self.in_paragraph && self.in_text
    }

    fn end_paragraph(&mut self) {
        self.queue.push_back(Event::EndParagraph);
        self.in_paragraph = false;
        self.in_text = false;
        self.pending_text.clear();
    }

    fn flush_pending_text(&mut self) {
        if !self.pending_text.is_empty() {
            self.queue.push_back(Event::Text {
                content: core::mem::take(&mut self.pending_text),
            });
        }
    }

    fn handle_cdata(&mut self, cdata: BytesCData<'_>) -> Result<()> {
        if self.can_collect_text() {
            let bytes = cdata.into_inner();
            let content = core::str::from_utf8(&bytes)
                .map_err(|err| parse_error(format!("malformed document.xml: {err}")))?;
            self.pending_text.push_str(content);
        }
        Ok(())
    }

    fn handle_empty(&mut self, local: &[u8]) {
        match local {
            value if self.in_ignored_subtree > 0 || is_ignored_container(value) => {}
            b"p" if !self.in_paragraph => {
                self.queue.push_back(Event::StartParagraph {
                    alignment: None,
                    id: None,
                });
                self.queue.push_back(Event::EndParagraph);
            }
            _ => {}
        }
    }

    fn handle_end(&mut self, local: &[u8]) {
        if self.in_ignored_subtree > 0 {
            self.in_ignored_subtree = self.in_ignored_subtree.saturating_sub(1);
            return;
        }

        match local {
            b"p" if self.in_paragraph => self.end_paragraph(),
            b"t" if self.in_text => {
                self.flush_pending_text();
                self.in_text = false;
            }
            _ => {}
        }
    }

    fn handle_eof(&mut self) {
        if self.in_text {
            self.flush_pending_text();
        }
        if self.in_paragraph {
            self.end_paragraph();
        }
        self.queue.push_back(Event::EndDocument);
        self.phase = Phase::Finished;
    }

    fn handle_general_ref(&mut self, reference: &BytesRef<'_>) -> Result<()> {
        if self.can_collect_text() {
            let decoded = reference
                .decode()
                .map_err(|err| parse_error(format!("malformed document.xml: {err}")))?;
            let escaped = format!("&{decoded};");
            let unescaped = quick_xml::escape::unescape(&escaped)
                .map_err(|err| parse_error(format!("malformed document.xml: {err}")))?;
            self.pending_text.push_str(&unescaped);
        }
        Ok(())
    }

    fn handle_start(&mut self, local: &[u8]) {
        if self.in_ignored_subtree > 0 {
            self.in_ignored_subtree = self.in_ignored_subtree.saturating_add(1);
            return;
        }

        match local {
            value if is_ignored_container(value) => self.in_ignored_subtree = 1,
            b"p" if !self.in_paragraph => self.start_paragraph(),
            b"t" if self.in_paragraph => {
                self.in_text = true;
                self.pending_text.clear();
            }
            _ => {}
        }
    }

    fn handle_text(&mut self, text: &BytesText<'_>) -> Result<()> {
        if self.can_collect_text() {
            let decoded = text
                .decode()
                .map_err(|err| parse_error(format!("malformed document.xml: {err}")))?;
            let unescaped = quick_xml::escape::unescape(&decoded)
                .map_err(|err| parse_error(format!("malformed document.xml: {err}")))?;
            self.pending_text.push_str(&unescaped);
        }
        Ok(())
    }

    fn read_until_event(&mut self) -> Result<()> {
        let event = self
            .xml
            .read_event_into(&mut self.buf)
            .map_err(|err| match err {
                quick_xml::Error::Io(source) => Error::Io {
                    source: std::io::Error::new(source.kind(), source.to_string()),
                },
                other => Error::Parse {
                    message: format!("malformed document.xml: {other}"),
                    position: None,
                },
            })?
            .into_owned();

        match event {
            quick_xml::events::Event::Start(tag) => self.handle_start(tag.local_name().as_ref()),
            quick_xml::events::Event::End(tag) => self.handle_end(tag.local_name().as_ref()),
            quick_xml::events::Event::Empty(tag) => self.handle_empty(tag.local_name().as_ref()),
            quick_xml::events::Event::Text(text) => {
                self.handle_text(&text)?;
            }
            quick_xml::events::Event::GeneralRef(reference) => {
                self.handle_general_ref(&reference)?;
            }
            quick_xml::events::Event::CData(cdata) => self.handle_cdata(cdata)?,
            quick_xml::events::Event::Eof => self.handle_eof(),
            quick_xml::events::Event::Comment(_)
            | quick_xml::events::Event::Decl(_)
            | quick_xml::events::Event::PI(_)
            | quick_xml::events::Event::DocType(_) => {}
        }

        self.buf.clear();
        Ok(())
    }

    fn start_paragraph(&mut self) {
        self.queue.push_back(Event::StartParagraph {
            alignment: None,
            id: None,
        });
        self.in_paragraph = true;
        self.in_text = false;
        self.pending_text.clear();
    }
}

impl EventSource for DocxReader {
    #[inline]
    fn next_event(&mut self) -> Result<Option<Event>> {
        loop {
            if let Some(event) = self.queue.pop_front() {
                return Ok(Some(event));
            }

            match self.phase {
                Phase::NotStarted => {
                    self.phase = Phase::Running;
                    self.queue.push_back(Event::StartDocument {
                        id: None,
                        language: None,
                        metadata: None,
                    });
                }
                Phase::Finished => return Ok(None),
                Phase::Running => self.read_until_event()?,
            }
        }
    }
}

fn is_ignored_container(local: &[u8]) -> bool {
    matches!(
        local,
        b"tbl"
            | b"tr"
            | b"tc"
            | b"sdt"
            | b"hyperlink"
            | b"drawing"
            | b"pict"
            | b"object"
            | b"ins"
            | b"del"
            | b"moveFrom"
            | b"moveTo"
    )
}

fn parse_error(message: String) -> Error {
    Error::Parse {
        message,
        position: None,
    }
}

#[cfg(test)]
#[cfg(not(coverage))]
mod tests {
    use std::io::{Cursor, Write as _};

    use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

    use super::*;

    const SIMPLE_RELS: &str = r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;

    fn synth_docx_for_unit_test(
        rels_xml: &str,
        document_xml: &str,
    ) -> core::result::Result<Vec<u8>, Box<dyn core::error::Error>> {
        let buf = Cursor::new(Vec::new());
        let mut writer = ZipWriter::new(buf);
        let rels_options =
            SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        writer.start_file("_rels/.rels", rels_options)?;
        writer.write_all(rels_xml.as_bytes())?;
        let document_options =
            SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        writer.start_file("word/document.xml", document_options)?;
        writer.write_all(document_xml.as_bytes())?;
        Ok(writer.finish()?.into_inner())
    }

    fn make_reader(
        document_xml: &str,
    ) -> core::result::Result<DocxReader, Box<dyn core::error::Error>> {
        let bytes = synth_docx_for_unit_test(SIMPLE_RELS, document_xml)?;
        Ok(DocxReader::from_reader(Cursor::new(bytes))?)
    }

    #[test]
    fn queue_length_never_exceeds_three() -> core::result::Result<(), Box<dyn core::error::Error>> {
        let doc = {
            let mut content = String::from(
                r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>"#,
            );
            for _ in 0..1000 {
                content.push_str("<w:p><w:r><w:t>hello</w:t></w:r></w:p>");
            }
            content.push_str("</w:body></w:document>");
            content
        };
        let mut reader = make_reader(&doc)?;
        loop {
            if reader.queue.len() > 3 {
                return Err(Box::new(Error::Other {
                    message: format!("queue grew to {}", reader.queue.len()),
                }));
            }
            if reader.next_event()?.is_none() {
                break;
            }
        }
        Ok(())
    }

    #[test]
    fn buf_is_cleared_per_iteration() -> core::result::Result<(), Box<dyn core::error::Error>> {
        let doc = r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>hello</w:t></w:r></w:p></w:body></w:document>"#;
        let mut reader = make_reader(doc)?;
        while reader.next_event()?.is_some() {
            if !reader.buf.is_empty() {
                return Err(Box::new(Error::Other {
                    message: "buf not cleared after event".to_string(),
                }));
            }
        }
        Ok(())
    }
}
