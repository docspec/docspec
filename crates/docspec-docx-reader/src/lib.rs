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
            | _ => Error::Parse {
                message: format!("not a valid ZIP archive: {err}"),
                position: None,
            },
        })?;

        let document_path = rels::find_document_path(&mut archive)?;

        let (data_start, compressed_size, method) = {
            let entry = archive
                .by_name(&document_path)
                .map_err(|_err| Error::Parse {
                    message: format!("document target not found: {document_path}"),
                    position: None,
                })?;
            let data_start = entry.data_start().ok_or_else(|| Error::Parse {
                message: "document.xml has no data offset".to_string(),
                position: None,
            })?;
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
            phase: Phase::NotStarted,
            queue: VecDeque::new(),
            xml,
        })
    }
}

impl EventSource for DocxReader {
    #[inline]
    fn next_event(&mut self) -> Result<Option<Event>> {
        Err(Error::Other {
            message: "docx reader not yet implemented".to_string(),
        })
    }
}
