#![forbid(unsafe_code)]
//! ODT to `DocSpec` event stream reader.
//!
//! [`OdtReader`] implements [`EventSource`] for ODT files. It
//! reads `content.xml` from the ODT (ZIP) archive with `quick-xml` and emits
//! one event per XML token, so nothing is queued or buffered.
//!
//! # Scope
//!
//! Only paragraphs are read for now.
//!
//! **In scope**: paragraphs (`<text:p>`), their text, spans and links
//! (`<text:span>`, `<text:a>` — transparent, their text is emitted unstyled
//! and unlinked), repeated spaces (`<text:s>`), tabs (`<text:tab>`, emitted as
//! a `Text` event whose content is `"\t"`), and line breaks
//! (`<text:line-break>`).
//! Emits: `StartDocument`, `StartParagraph`, `Text`, `LineBreak`,
//! `EndParagraph`, `EndDocument`.
//!
//! Paragraphs inside lists, table cells, and sections come out as plain,
//! flat paragraphs.
//!
//! **Out of scope (silently dropped)**:
//! - Headings (`<text:h>`)
//! - Every other element inside a paragraph, with its subtree: notes, frames,
//!   images, annotations, and fields
//! - Tracked changes (`<text:tracked-changes>`) — accept-changes semantics
//! - Styles, alignment, link targets, and document metadata
//! - Flat ODT (`.fodt`) and encrypted documents
//!
//! Whitespace is emitted as written; ODF whitespace collapsing is not applied.
//!
//! # Streaming Guarantee
//!
//! - **`from_path`**: streams `content.xml`; memory is proportional to the
//!   largest single XML node (one long text run, say), not to the file.
//! - **`from_reader`**: buffers `content.xml` into memory — O(N) in document
//!   size. Use `from_path` for large documents.
//!
//! Neither the decompressed `content.xml` size nor a single node is capped yet
//! (#582), so a crafted file can exhaust memory.
//!
//! # Quick Start
//!
//! ```no_run
//! use docspec_odt_reader::{EventSource, OdtReader};
//!
//! let mut reader = OdtReader::from_path("document.odt")?;
//! while let Some(event) = reader.next_event()? {
//!     println!("{event:?}");
//! }
//! # Ok::<(), docspec_core::Error>(())
//! ```

use core::fmt::Display;
use std::io::{BufReader, Cursor, Read, Seek};
use std::path::Path;

pub use docspec_core::EventSource;
use docspec_core::{Error, Event, Result};
use docspec_zip::{open_archive, read_optional_entry, StreamingArchive};
use quick_xml::events::{BytesRef, BytesStart, Event as XmlEvent};

/// The archive entry holding the document body.
const CONTENT_ENTRY: &str = "content.xml";

/// Which part of the stream comes next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// `StartDocument` is next.
    NotStarted,
    /// `content.xml` is being read.
    Body,
    /// The stream is exhausted.
    Finished,
}

/// A streaming ODT reader that implements [`EventSource`].
///
/// See the crate-level docs for the elements it recognizes.
///
/// # Errors
///
/// Returns [`Error::Io`] for I/O failures and [`Error::Parse`] for malformed
/// archives or XML.
pub struct OdtReader {
    xml: quick_xml::Reader<BufReader<Box<dyn Read + Send>>>,
    scratch: Vec<u8>,
    phase: Phase,
    in_paragraph: bool,
    /// Nesting depth inside a subtree that is being dropped; 0 when not skipping.
    skip_depth: usize,
    /// Elements opened and not yet closed; nonzero at EOF means a cut-off file.
    open_elements: usize,
    /// Whether a root element was seen; an empty or text-only file has none.
    saw_root: bool,
}

impl core::fmt::Debug for OdtReader {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("OdtReader")
            .field("phase", &self.phase)
            .finish_non_exhaustive()
    }
}

impl OdtReader {
    /// Creates a reader from any `Read + Seek` source holding an ODT archive.
    ///
    /// **Memory note**: this buffers `content.xml` into memory. Use
    /// [`from_path`](Self::from_path) to stream it.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Parse`] if the input is not a valid ZIP archive or has
    /// no `content.xml`, and [`Error::Io`] for I/O failures.
    #[inline]
    pub fn from_reader<R>(reader: R) -> Result<Self>
    where
        R: Read + Seek + Send + 'static,
    {
        let mut archive = open_archive(reader)?;
        let content = read_optional_entry(&mut archive, CONTENT_ENTRY)?
            .ok_or_else(|| malformed("entry is missing"))?;
        Ok(Self::new(Box::new(Cursor::new(content))))
    }

    /// Creates a reader for the ODT file at `path`, streaming `content.xml`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if the file cannot be opened or `path` is a
    /// directory, and [`Error::Parse`] if it is not a valid ZIP archive or has
    /// no `content.xml`.
    #[inline]
    pub fn from_path<P>(path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        if path.is_dir() {
            return Err(Error::Io {
                source: std::io::ErrorKind::IsADirectory.into(),
            });
        }
        Ok(Self::new(Box::new(StreamingArchive::open(
            path,
            CONTENT_ENTRY,
        )?)))
    }

    fn new(content: Box<dyn Read + Send>) -> Self {
        let mut xml = quick_xml::Reader::from_reader(BufReader::new(content));
        let config = xml.config_mut();
        // The reader tracks element scope itself and tolerates mismatched close tags.
        config.check_end_names = false;
        // `<text:p/>` then arrives as a start and an end, like `<text:p></text:p>`.
        config.expand_empty_elements = true;
        Self {
            xml,
            scratch: Vec::new(),
            phase: Phase::NotStarted,
            in_paragraph: false,
            skip_depth: 0,
            open_elements: 0,
            saw_root: false,
        }
    }
}

impl EventSource for OdtReader {
    #[inline]
    fn next_event(&mut self) -> Result<Option<Event>> {
        match self.phase {
            Phase::NotStarted => {
                self.phase = Phase::Body;
                return Ok(Some(Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                }));
            }
            Phase::Finished => return Ok(None),
            Phase::Body => {}
        }
        loop {
            self.scratch.clear();
            let token = self
                .xml
                .read_event_into(&mut self.scratch)
                .map_err(xml_error)?;
            match token {
                XmlEvent::Start(_) => {
                    self.open_elements = self.open_elements.saturating_add(1);
                    self.saw_root = true;
                }
                XmlEvent::End(_) => self.open_elements = self.open_elements.saturating_sub(1),
                _ => {}
            }
            let event = match token {
                XmlEvent::Eof => {
                    if self.open_elements > 0 || !self.saw_root {
                        return Err(malformed("unexpected end of file"));
                    }
                    self.phase = Phase::Finished;
                    Some(Event::EndDocument)
                }
                XmlEvent::Start(_) if self.skip_depth > 0 => {
                    self.skip_depth = self.skip_depth.saturating_add(1);
                    None
                }
                XmlEvent::End(_) if self.skip_depth > 0 => {
                    self.skip_depth = self.skip_depth.saturating_sub(1);
                    None
                }
                _ if self.skip_depth > 0 => None,
                XmlEvent::Start(start) => match (self.in_paragraph, start.local_name().as_ref()) {
                    (false, b"p") => {
                        self.in_paragraph = true;
                        Some(Event::StartParagraph {
                            alignment: None,
                            id: None,
                        })
                    }
                    (false, b"tracked-changes") => {
                        self.skip_depth = 1;
                        None
                    }
                    (false, _) | (true, b"span" | b"a") => None,
                    (true, b"s") => text(" ".repeat(space_count(&start)?)),
                    (true, b"tab") => text("\t".to_string()),
                    (true, b"line-break") => Some(Event::LineBreak),
                    // Notes and frames hold paragraphs of their own, which must not nest.
                    (true, _) => {
                        self.skip_depth = 1;
                        None
                    }
                },
                XmlEvent::End(end) if self.in_paragraph && end.local_name().as_ref() == b"p" => {
                    self.in_paragraph = false;
                    Some(Event::EndParagraph)
                }
                // shortcut: ODF whitespace collapsing is not applied, upgrade when a real
                // file shows doubled spaces.
                XmlEvent::Text(raw) if self.in_paragraph => {
                    text(raw.decode().map_err(malformed)?.into_owned())
                }
                XmlEvent::CData(raw) if self.in_paragraph => {
                    text(raw.decode().map_err(malformed)?.into_owned())
                }
                XmlEvent::GeneralRef(reference) if self.in_paragraph => {
                    text(resolve_reference(&reference)?)
                }
                _ => None,
            };
            if event.is_some() {
                return Ok(event);
            }
        }
    }
}

/// A `Text` event for `content`, or nothing when it is empty.
fn text(content: String) -> Option<Event> {
    (!content.is_empty()).then_some(Event::Text { content })
}

/// The number of spaces a `<text:s>` element stands for: its `text:c`, or 1.
fn space_count(start: &BytesStart<'_>) -> Result<usize> {
    let Some(attribute) = start.try_get_attribute("text:c").map_err(malformed)? else {
        return Ok(1);
    };
    // `u16` bounds the allocation a hostile count could force.
    core::str::from_utf8(&attribute.value)
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .map(usize::from)
        .ok_or_else(|| malformed("invalid text:c"))
}

/// Resolves an entity or character reference to the text it stands for.
fn resolve_reference(reference: &BytesRef<'_>) -> Result<String> {
    // quick-xml hands over the name without its delimiters; restoring them
    // reuses its entity table.
    let escaped = format!("&{};", reference.decode().map_err(malformed)?);
    Ok(quick_xml::escape::unescape(&escaped)
        .map_err(malformed)?
        .into_owned())
}

/// Builds a position-less parse error for `content.xml`.
fn malformed<E>(err: E) -> Error
where
    E: Display,
{
    Error::Parse {
        message: format!("malformed {CONTENT_ENTRY}: {err}"),
        position: None,
    }
}

/// Converts a quick-xml failure, preserving the I/O error kind.
fn xml_error(err: quick_xml::Error) -> Error {
    match err {
        quick_xml::Error::Io(source) => Error::Io {
            source: std::io::Error::new(source.kind(), source.to_string()),
        },
        other => malformed(other),
    }
}
