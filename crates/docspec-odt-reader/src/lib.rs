#![forbid(unsafe_code)]
//! ODT to `DocSpec` event stream reader.
//!
//! [`OdtReader`] implements [`EventSource`] for ODT files. It
//! reads `content.xml` from the ODT (ZIP) archive with `quick-xml` and emits
//! one event per XML token, so nothing is queued or buffered.
//!
//! # Scope
//!
//! Only paragraphs and headings are read for now.
//!
//! **In scope**: paragraphs (`<text:p>`), headings (`<text:h>`, with the level
//! from `text:outline-level`, 1 when absent), their text, spans and links
//! (`<text:span>`, `<text:a>` — transparent, their text is emitted unstyled
//! and unlinked), repeated spaces (`<text:s>`), tabs (`<text:tab>`, emitted as
//! a `Text` event whose content is `"\t"`), and line breaks
//! (`<text:line-break>`).
//! Emits: `StartDocument`, `StartParagraph`, `StartHeading`, `Text`,
//! `LineBreak`, `EndParagraph`, `EndHeading`, `EndDocument`.
//!
//! Paragraphs and headings inside lists, table cells, and sections come out
//! flat, as if they were at the top level.
//!
//! **Out of scope (silently dropped)**:
//! - Every other element inside a paragraph or heading, with its subtree:
//!   notes, frames, images, annotations, and fields
//! - Paragraphs styled to look like headings (a `<text:p>` with a heading
//!   style) — emitted as paragraphs
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
use core::num::NonZeroU8;
use core::str::FromStr;
use std::io::{BufReader, Cursor, Read, Seek};
use std::path::Path;

pub use docspec_core::EventSource;
use docspec_core::{Error, Event, Result};
use docspec_zip::{open_archive, read_optional_entry, StreamingArchive};
use quick_xml::events::{BytesStart, Event as XmlEvent};

/// The archive entry holding the document body.
const CONTENT_ENTRY: &str = "content.xml";

/// Starts every error about malformed `content.xml` content.
const CONTEXT: &str = "malformed content.xml";

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

/// The block whose inline content is being read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Block {
    /// Inside `<text:p>`.
    Paragraph,
    /// Inside `<text:h>`.
    Heading,
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
    /// The open paragraph or heading, if any. Blocks never nest.
    block: Option<Block>,
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
            block: None,
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
                .map_err(|err| docspec_xml::xml_error(CONTEXT, err))?;
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
                XmlEvent::Start(start) => match (self.block.is_some(), start.local_name().as_ref())
                {
                    (false, b"p") => {
                        self.block = Some(Block::Paragraph);
                        Some(Event::StartParagraph {
                            alignment: None,
                            id: None,
                        })
                    }
                    (false, b"h") => {
                        // Absent means level 1; zero is not a level.
                        let level = number::<NonZeroU8>(&self.xml, &start, "outline-level")?
                            .map_or(1, NonZeroU8::get);
                        self.block = Some(Block::Heading);
                        Some(Event::StartHeading { id: None, level })
                    }
                    (false, b"tracked-changes") => {
                        self.skip_depth = 1;
                        None
                    }
                    (false, _) | (true, b"span" | b"a") => None,
                    (true, b"s") => {
                        // `u16` bounds the allocation a hostile count could force.
                        let count = number::<u16>(&self.xml, &start, "c")?.map_or(1, usize::from);
                        text(" ".repeat(count))
                    }
                    (true, b"tab") => text("\t".to_string()),
                    (true, b"line-break") => Some(Event::LineBreak),
                    // Notes and frames hold paragraphs of their own, which must not nest.
                    (true, _) => {
                        self.skip_depth = 1;
                        None
                    }
                },
                XmlEvent::End(end) => match (self.block, end.local_name().as_ref()) {
                    (Some(Block::Paragraph), b"p") => {
                        self.block = None;
                        Some(Event::EndParagraph)
                    }
                    (Some(Block::Heading), b"h") => {
                        self.block = None;
                        Some(Event::EndHeading)
                    }
                    _ => None,
                },
                // shortcut: ODF whitespace collapsing is not applied, upgrade when a real
                // file shows doubled spaces.
                XmlEvent::Text(raw) if self.block.is_some() => {
                    text(raw.decode().map_err(malformed)?.into_owned())
                }
                XmlEvent::CData(raw) if self.block.is_some() => {
                    text(raw.decode().map_err(malformed)?.into_owned())
                }
                XmlEvent::GeneralRef(reference) if self.block.is_some() => {
                    text(docspec_xml::resolve_reference(CONTEXT, &reference)?)
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

/// The numeric value of the text-namespace attribute `name`, or `None` when it
/// is absent. Matched by local name, like elements, so any prefix works.
fn number<T>(
    xml: &quick_xml::Reader<impl Read>,
    start: &BytesStart<'_>,
    name: &str,
) -> Result<Option<T>>
where
    T: FromStr,
{
    docspec_xml::attr_string(xml, start, name.as_bytes(), CONTEXT)?
        .map(|value| {
            value
                .parse()
                .ok()
                .ok_or_else(|| malformed(format!("invalid text:{name}")))
        })
        .transpose()
}

/// Builds a position-less parse error for `content.xml`.
fn malformed<E>(err: E) -> Error
where
    E: Display,
{
    docspec_xml::malformed(CONTEXT, err)
}
