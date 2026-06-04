//! Reader factory for creating readers from any `Read + Seek` source.

use std::io::{Read, Seek};
use std::path::Path;

use docspec_core::{Event, EventSource, Result};

#[cfg(feature = "html")]
use crate::factory::html_owned::HtmlReaderOwned;
#[cfg(feature = "markdown")]
use crate::factory::markdown_owned::MarkdownReaderOwned;
#[cfg(feature = "docx")]
use docspec_docx_reader::DocxReader;

use crate::format::InputFormat;

/// Enum-dispatch reader for any registered input format.
///
/// Constructed via [`AnyReader::from_reader`] or [`AnyReader::from_path`].
/// Implements [`EventSource`] by delegating `next_event` to the inner concrete reader.
pub enum AnyReader {
    /// DOCX reader.
    #[cfg(feature = "docx")]
    Docx(DocxReader),
    /// HTML reader (paragraph-only; see crate docs).
    #[cfg(feature = "html")]
    Html(HtmlReaderOwned),
    /// Markdown reader (paragraph-only; see crate docs).
    #[cfg(feature = "markdown")]
    Markdown(MarkdownReaderOwned),
    /// Phantom variant when no reader features are active.
    #[cfg(not(any(feature = "markdown", feature = "html", feature = "docx")))]
    _Phantom(core::convert::Infallible),
}

impl AnyReader {
    /// Convenience: open the file at `path` and call [`from_reader`](Self::from_reader).
    ///
    /// # Errors
    ///
    /// Returns `Err` if the file cannot be opened or if `from_reader` fails.
    #[inline]
    pub fn from_path<P: AsRef<Path>>(format: InputFormat, path: P) -> Result<Self> {
        let file = std::fs::File::open(path)?;
        Self::from_reader(format, file)
    }

    /// Construct a reader for the given format from any `Read + Seek` source.
    ///
    /// For text formats (Markdown, HTML), the full input is read into a `String`
    /// (UTF-8 validation is applied) and a leading UTF-8 BOM is stripped before
    /// the inner reader is constructed.
    ///
    /// For binary formats (DOCX), the reader is passed through to the underlying
    /// streaming reader; no buffering is performed by this factory beyond what the
    /// inner reader requires.
    ///
    /// # Errors
    ///
    /// Returns `Err` if reading the input fails (including non-UTF-8 bytes for text
    /// formats — `read_to_string` surfaces these as `io::ErrorKind::InvalidData`).
    /// Returns `Err` if a binary archive is malformed.
    #[inline]
    pub fn from_reader<R: Read + Seek + 'static>(format: InputFormat, reader: R) -> Result<Self> {
        #[cfg(not(any(feature = "markdown", feature = "html", feature = "docx")))]
        {
            let _ = reader;
            match format {}
        }
        #[cfg(any(feature = "markdown", feature = "html", feature = "docx"))]
        match format {
            #[cfg(feature = "docx")]
            InputFormat::Docx => {
                let docx = DocxReader::from_reader(reader)?;
                Ok(Self::Docx(docx))
            }
            #[cfg(feature = "html")]
            InputFormat::Html => {
                let mut raw_input = String::new();
                let mut reader = reader;
                reader.read_to_string(&mut raw_input)?;
                let input = crate::format::strip_bom(&raw_input).to_owned();
                Ok(Self::Html(HtmlReaderOwned::new(input, |owned| {
                    docspec_html_reader::HtmlReader::new(owned)
                })))
            }
            #[cfg(feature = "markdown")]
            InputFormat::Markdown => {
                let mut raw_input = String::new();
                let mut reader = reader;
                reader.read_to_string(&mut raw_input)?;
                let input = crate::format::strip_bom(&raw_input).to_owned();
                Ok(Self::Markdown(MarkdownReaderOwned::new(input, |owned| {
                    docspec_markdown_reader::MarkdownReader::new(owned)
                })))
            }
        }
    }
}

impl EventSource for AnyReader {
    #[inline]
    fn next_event(&mut self) -> Result<Option<Event>> {
        match self {
            #[cfg(feature = "docx")]
            Self::Docx(reader) => reader.next_event(),
            #[cfg(feature = "html")]
            Self::Html(reader) => reader.next_event(),
            #[cfg(feature = "markdown")]
            Self::Markdown(reader) => reader.next_event(),
            #[cfg(not(any(feature = "markdown", feature = "html", feature = "docx")))]
            Self::_Phantom(infallible) => match *infallible {},
        }
    }
}
