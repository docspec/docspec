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
//! content is the single character `"\t"`), and tables (`<w:tbl>`, `<w:tr>`,
//! `<w:tc>` — emitted as structural events only; cell merging, header rows, and
//! table styles are not represented).
//! Emits exactly: `StartDocument`, `StartParagraph`, `Text`, `LineBreak`,
//! `EndParagraph`, `StartTable`, `StartTableRow`, `StartTableCell`,
//! `EndTableCell`, `EndTableRow`, `EndTable`, `EndDocument`.
//!
//! **Out of scope (silently dropped)**:
//! - Run styling (`<w:rPr>`, bold, italic, etc.)
//! - Headings (any `<w:pStyle>` value — every paragraph is `StartParagraph`)
//! - Cell merging (`<w:gridSpan>`, `<w:vMerge>`) — every cell emits with
//!   `colspan: None` and `rowspan: None`
//! - Header rows (`<w:tblHeader>`) — every cell emits as `StartTableCell`,
//!   never `StartTableHeader`
//! - Table, row, and cell properties (`<w:tblPr>`, `<w:trPr>`, `<w:tcPr>`,
//!   `<w:tblGrid>`)
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

mod properties;
mod rels;

use alloc::collections::VecDeque;
use core::fmt;
use std::io::{BufReader, Read, Seek};
use std::path::Path;

pub use docspec_core::EventSource;
use docspec_core::{Error, Event, Result, TextAlignment, TextStyle};
use quick_xml::events::{BytesCData, BytesRef, BytesStart, BytesText};

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
#[expect(
    clippy::struct_excessive_bools,
    reason = "DocxReader tracks six independent boolean parser states; grouping them would obscure the streaming state machine"
)]
pub struct DocxReader {
    /// Reusable buffer for quick-xml event reading.
    buf: Vec<u8>,
    /// Depth counter for ignored subtrees (tracked changes, hyperlinks,
    /// drawings, table/row/cell property containers, etc.).
    /// Incremented on Start of an ignored container, decremented on End.
    in_ignored_subtree: u32,
    /// Whether the reader is currently inside a `<w:p>` element.
    in_paragraph: bool,
    /// Whether the reader is currently inside a `<w:t>` element.
    in_text: bool,
    /// Whether currently inside a `<w:pPr>` element that is still legal (first child of paragraph).
    in_ppr: bool,
    /// Paragraph alignment captured from `<w:jc>` while inside `<w:pPr>`.
    pending_paragraph_alignment: Option<TextAlignment>,
    /// True once `StartParagraph` has been queued for the current paragraph.
    paragraph_started_emitted: bool,
    /// Whether currently inside a `<w:rPr>` element that is still legal (first child of run).
    in_rpr: bool,
    /// Run style accumulated while inside `<w:rPr>`.
    pending_run_style: TextStyle,
    /// Text collected for the current `<w:t>` element.
    pending_text: String,
    /// Run style frozen at `</w:rPr>`, applied to subsequent text emissions in the same run.
    current_run_style: TextStyle,
    /// Document processing phase.
    phase: Phase,
    /// Queue of `DocSpec` events to emit.
    queue: VecDeque<Event>,
    /// True once the first content event of the current run has been queued.
    run_content_emitted: bool,
    /// The quick-xml reader streaming from the document entry.
    xml: quick_xml::Reader<BufReader<Box<dyn Read + Send>>>,
}

impl fmt::Debug for DocxReader {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DocxReader")
            .field("buf", &self.buf)
            .field("in_ignored_subtree", &self.in_ignored_subtree)
            .field("in_paragraph", &self.in_paragraph)
            .field("in_text", &self.in_text)
            .field("in_ppr", &self.in_ppr)
            .field(
                "pending_paragraph_alignment",
                &self.pending_paragraph_alignment,
            )
            .field("paragraph_started_emitted", &self.paragraph_started_emitted)
            .field("in_rpr", &self.in_rpr)
            .field("pending_run_style", &self.pending_run_style)
            .field("pending_text", &self.pending_text)
            .field("current_run_style", &self.current_run_style)
            .field("phase", &"<phase>")
            .field("queue", &self.queue)
            .field("run_content_emitted", &self.run_content_emitted)
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
    pub fn from_reader<R: Read + Seek + Send + 'static>(mut reader: R) -> Result<Self> {
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

        let stream: Box<dyn Read + Send> = if method == zip::CompressionMethod::Stored {
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
            in_ppr: false,
            pending_paragraph_alignment: None,
            paragraph_started_emitted: false,
            in_rpr: false,
            pending_run_style: TextStyle::default(),
            pending_text: String::new(),
            current_run_style: TextStyle::default(),
            phase: Phase::NotStarted,
            queue: VecDeque::new(),
            run_content_emitted: false,
            xml,
        })
    }
}

impl DocxReader {
    fn can_collect_text(&self) -> bool {
        self.in_ignored_subtree == 0 && self.in_paragraph && self.in_text
    }

    fn emit_line_break(&mut self) {
        self.ensure_paragraph_started();
        self.flush_pending_text();
        self.run_content_emitted = true;
        self.queue.push_back(Event::LineBreak);
    }

    fn emit_tab(&mut self) {
        self.ensure_paragraph_started();
        self.flush_pending_text();
        self.run_content_emitted = true;
        self.queue.push_back(Event::Text {
            content: "\t".to_string(),
            style: TextStyle::default(),
        });
    }

    fn end_paragraph(&mut self) {
        self.ensure_paragraph_started();
        self.queue.push_back(Event::EndParagraph);
        self.in_paragraph = false;
        self.in_text = false;
        self.pending_text.clear();
        self.in_ppr = false;
        self.pending_paragraph_alignment = None;
        self.paragraph_started_emitted = false;
    }

    fn flush_pending_text(&mut self) {
        if !self.pending_text.is_empty() {
            self.queue.push_back(Event::Text {
                content: core::mem::take(&mut self.pending_text),
                style: self.current_run_style.clone(),
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

    fn handle_empty(&mut self, tag: &BytesStart<'_>) {
        let local_name = tag.local_name();
        let local = local_name.as_ref();
        match local {
            value if self.in_ignored_subtree > 0 || is_ignored_container(value) => {}
            b"pPr" if self.in_paragraph && !self.paragraph_started_emitted => {
                self.ensure_paragraph_started();
            }
            b"jc" if self.in_ppr => {
                let val = read_val_attribute(tag);
                self.pending_paragraph_alignment =
                    val.as_deref().and_then(properties::parse_alignment);
            }
            b"rPr" if self.in_ppr => {}
            b"rPr" if self.in_paragraph && !self.in_ppr && !self.in_rpr => {}
            b"b" if self.in_rpr => {
                self.pending_run_style.bold = parse_on_off_attribute(tag);
            }
            b"i" if self.in_rpr => {
                self.pending_run_style.italic = parse_on_off_attribute(tag);
            }
            b"strike" | b"dstrike" if self.in_rpr => {
                self.pending_run_style.strikethrough = parse_on_off_attribute(tag);
            }
            b"u" if self.in_rpr => {
                let val = read_val_attribute(tag);
                self.pending_run_style.underline = properties::parse_underline_on(val.as_deref());
            }
            b"vertAlign" if self.in_rpr => {
                let val = read_val_attribute(tag);
                match properties::parse_vert_align(val.as_deref()) {
                    properties::VertAlign::Subscript => {
                        self.pending_run_style.subscript = true;
                        self.pending_run_style.superscript = false;
                    }
                    properties::VertAlign::Superscript => {
                        self.pending_run_style.superscript = true;
                        self.pending_run_style.subscript = false;
                    }
                    properties::VertAlign::None => {
                        self.pending_run_style.subscript = false;
                        self.pending_run_style.superscript = false;
                    }
                }
            }
            b"p" if !self.in_paragraph => {
                self.queue.push_back(Event::StartParagraph {
                    alignment: None,
                    id: None,
                });
                self.queue.push_back(Event::EndParagraph);
            }
            b"br" if self.in_paragraph => self.emit_line_break(),
            b"tab" if self.in_paragraph => self.emit_tab(),
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
            b"pPr" if self.in_ppr => {
                self.ensure_paragraph_started();
                self.in_ppr = false;
            }
            b"rPr" if self.in_rpr => {
                self.current_run_style = self.pending_run_style.clone();
                self.in_rpr = false;
            }
            b"r" => {
                self.current_run_style = TextStyle::default();
                self.pending_run_style = TextStyle::default();
                self.run_content_emitted = false;
                self.in_rpr = false;
            }
            b"t" if self.in_text => {
                self.flush_pending_text();
                self.in_text = false;
            }
            b"tbl" => self.queue.push_back(Event::EndTable),
            b"tr" => self.queue.push_back(Event::EndTableRow),
            b"tc" => self.queue.push_back(Event::EndTableCell),
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

    fn handle_start(&mut self, tag: &BytesStart<'_>) {
        let local_name = tag.local_name();
        let local = local_name.as_ref();
        if self.in_ignored_subtree > 0 {
            self.in_ignored_subtree = self.in_ignored_subtree.saturating_add(1);
            return;
        }

        match local {
            value if is_ignored_container(value) => self.in_ignored_subtree = 1,
            b"pPr" if self.in_paragraph => {
                if self.paragraph_started_emitted {
                    // Out-of-order pPr: StartParagraph already emitted; silently consume
                    self.in_ignored_subtree = 1;
                } else {
                    self.in_ppr = true;
                    self.pending_paragraph_alignment = None;
                }
            }
            b"jc" if self.in_ppr => {
                let val = read_val_attribute(tag);
                self.pending_paragraph_alignment =
                    val.as_deref().and_then(properties::parse_alignment);
            }
            b"rPr" if self.in_ppr => {
                self.in_ignored_subtree = 1;
            }
            b"rPr" if self.in_paragraph && !self.in_ppr && !self.in_rpr => {
                if self.run_content_emitted {
                    // Out-of-order rPr: content already emitted in this run; silently consume
                    self.in_ignored_subtree = 1;
                } else {
                    self.in_rpr = true;
                    self.pending_run_style = TextStyle::default();
                }
            }
            b"b" if self.in_rpr => {
                self.pending_run_style.bold = parse_on_off_attribute(tag);
            }
            b"i" if self.in_rpr => {
                self.pending_run_style.italic = parse_on_off_attribute(tag);
            }
            b"strike" | b"dstrike" if self.in_rpr => {
                self.pending_run_style.strikethrough = parse_on_off_attribute(tag);
            }
            b"u" if self.in_rpr => {
                let val = read_val_attribute(tag);
                self.pending_run_style.underline = properties::parse_underline_on(val.as_deref());
            }
            b"vertAlign" if self.in_rpr => {
                let val = read_val_attribute(tag);
                match properties::parse_vert_align(val.as_deref()) {
                    properties::VertAlign::Subscript => {
                        self.pending_run_style.subscript = true;
                        self.pending_run_style.superscript = false;
                    }
                    properties::VertAlign::Superscript => {
                        self.pending_run_style.superscript = true;
                        self.pending_run_style.subscript = false;
                    }
                    properties::VertAlign::None => {
                        self.pending_run_style.subscript = false;
                        self.pending_run_style.superscript = false;
                    }
                }
            }
            b"p" if !self.in_paragraph => self.start_paragraph(),
            b"r" if self.in_paragraph => {
                self.ensure_paragraph_started();
            }
            b"t" if self.in_paragraph => {
                self.ensure_paragraph_started();
                self.in_text = true;
                self.pending_text.clear();
                self.run_content_emitted = true;
            }
            b"br" if self.in_paragraph => self.emit_line_break(),
            b"tab" if self.in_paragraph => self.emit_tab(),
            b"tbl" => self.queue.push_back(Event::StartTable { id: None }),
            b"tr" => self.queue.push_back(Event::StartTableRow { id: None }),
            b"tc" => self.queue.push_back(Event::StartTableCell {
                colspan: None,
                id: None,
                rowspan: None,
            }),
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
            quick_xml::events::Event::Start(tag) => self.handle_start(&tag),
            quick_xml::events::Event::End(tag) => self.handle_end(tag.local_name().as_ref()),
            quick_xml::events::Event::Empty(tag) => self.handle_empty(&tag),
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
        self.in_paragraph = true;
        self.in_text = false;
        self.pending_text.clear();
        self.paragraph_started_emitted = false;
        self.pending_paragraph_alignment = None;
    }

    /// Queues `StartParagraph` once paragraph properties have been parsed.
    fn ensure_paragraph_started(&mut self) {
        if self.in_paragraph && !self.paragraph_started_emitted {
            self.queue.push_back(Event::StartParagraph {
                alignment: self.pending_paragraph_alignment.clone(),
                id: None,
            });
            self.paragraph_started_emitted = true;
        }
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
        b"sdt"
            | b"hyperlink"
            | b"drawing"
            | b"pict"
            | b"object"
            | b"ins"
            | b"del"
            | b"moveFrom"
            | b"moveTo"
            | b"tblPr"
            | b"trPr"
            | b"tcPr"
            | b"tblGrid"
    )
}

fn read_val_attribute(tag: &BytesStart<'_>) -> Option<String> {
    let a = tag.try_get_attribute(b"w:val").ok().flatten()?;
    core::str::from_utf8(a.value.as_ref())
        .ok()
        .map(str::to_owned)
}

fn parse_on_off_attribute(tag: &BytesStart<'_>) -> bool {
    let val = read_val_attribute(tag);
    properties::parse_on_off(val.as_deref())
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

    #[test]
    fn docx_reader_is_send_static() {
        fn assert_send_static<T: Send + 'static>() {}
        assert_send_static::<DocxReader>();
    }

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
