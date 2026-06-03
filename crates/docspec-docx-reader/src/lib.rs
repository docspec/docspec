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

use alloc::collections::VecDeque;
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

impl DocxReader {
    /// Creates a `DocxReader` from a file path.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if the file cannot be opened. See [`from_reader`](Self::from_reader)
    /// for additional error conditions.
    #[inline]
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self> {
        let _ = path;
        Err(Error::Other {
            message: "docx reader not yet implemented".to_string(),
        })
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
    pub fn from_reader<R: Read + Seek + 'static>(reader: &R) -> Result<Self> {
        let _ = reader;
        Err(Error::Other {
            message: "docx reader not yet implemented".to_string(),
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
