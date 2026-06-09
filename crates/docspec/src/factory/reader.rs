//! Reader factory for creating readers from input formats.

#[cfg(feature = "docx")]
use std::io::Cursor;
use std::io::{Read, Seek};

use docspec_core::{Event, EventSource, Result};

#[cfg(feature = "docx")]
use docspec_docx_reader::DocxReader;
#[cfg(feature = "html")]
use docspec_html_reader::HtmlReader;
#[cfg(feature = "markdown")]
use docspec_markdown_reader::MarkdownReader;

use crate::format::InputFormat;

/// Enum-dispatch reader for any registered input format.
///
/// Constructed via [`AnyReader::from_str`], [`AnyReader::from_reader`], or
/// [`AnyReader::from_path`]. Implements [`EventSource`] by delegating `next_event`
/// to the inner concrete reader. Zero heap allocation, zero virtual-dispatch overhead.
///
/// `MarkdownReader`, `HtmlReader`, and `DocxReader` are all `Send + 'static`, so
/// `AnyReader` is also `Send + 'static` — suitable for use across
/// `tokio::task::spawn_blocking` boundaries.
#[non_exhaustive]
pub enum AnyReader {
    /// DOCX reader from [`docspec_docx_reader`].
    #[cfg(feature = "docx")]
    Docx(DocxReader),
    /// HTML reader from [`docspec_html_reader`] (paragraph-only; see crate docs).
    #[cfg(feature = "html")]
    Html(HtmlReader),
    /// Markdown reader from [`docspec_markdown_reader`].
    #[cfg(feature = "markdown")]
    Markdown(MarkdownReader),
}

impl AnyReader {
    /// Construct a reader by opening a file at `path`. The file is opened with
    /// `File::open` and passed to [`from_reader`](Self::from_reader). Works for
    /// all formats including DOCX.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the file cannot be opened or if the format-specific
    /// reader construction fails.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use docspec::AnyReader;
    /// use docspec::InputFormat;
    ///
    /// # fn main() -> docspec::Result<()> {
    /// let reader = AnyReader::from_path(InputFormat::Docx, "document.docx")?;
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    pub fn from_path<P: AsRef<std::path::Path>>(format: InputFormat, path: P) -> Result<Self> {
        let file = std::fs::File::open(path.as_ref())
            .map_err(|source| docspec_core::Error::Io { source })?;
        Self::from_reader(format, file)
    }

    /// Construct a reader for the given format from any `Read + Seek` source.
    ///
    /// The `Send + 'static` bounds are required so the resulting `AnyReader`
    /// can be moved across `tokio::task::spawn_blocking` boundaries.
    /// Text-format readers (Markdown, Html) automatically strip a leading UTF-8
    /// BOM (U+FEFF). Binary formats (DOCX) pass the bytes through unchanged.
    ///
    /// # Errors
    ///
    /// Returns `Err` if reading from `reader` fails (e.g., I/O error or
    /// invalid UTF-8 for text formats).
    ///
    /// # Example
    ///
    /// ```
    /// use std::io::Cursor;
    /// use docspec::AnyReader;
    /// use docspec::InputFormat;
    ///
    /// # fn main() -> docspec::Result<()> {
    /// let reader = AnyReader::from_reader(InputFormat::Markdown, Cursor::new("# Hello"))?;
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    pub fn from_reader<R: Read + Seek + Send + 'static>(
        format: InputFormat,
        reader: R,
    ) -> Result<Self> {
        #[cfg(not(any(feature = "markdown", feature = "html", feature = "docx")))]
        {
            let _ = reader;
            match format {}
        }
        #[cfg(any(feature = "markdown", feature = "html", feature = "docx"))]
        match format {
            #[cfg(feature = "docx")]
            InputFormat::Docx => Ok(Self::Docx(DocxReader::from_reader(reader)?)),
            #[cfg(feature = "html")]
            InputFormat::Html => {
                let stripped =
                    crate::factory::bom_stripping_reader::BomStrippingReader::new(reader)?;
                Ok(Self::Html(HtmlReader::from_reader(stripped)?))
            }
            #[cfg(feature = "markdown")]
            InputFormat::Markdown => {
                let stripped =
                    crate::factory::bom_stripping_reader::BomStrippingReader::new(reader)?;
                Ok(Self::Markdown(MarkdownReader::from_reader(stripped)?))
            }
        }
    }

    /// Construct a reader for the given format from an in-memory string slice.
    ///
    /// For text formats (`Markdown`, `Html`), strips a leading UTF-8 BOM (U+FEFF)
    /// before constructing the reader. For binary formats (`Docx`), the string
    /// bytes are passed directly to [`from_reader`](Self::from_reader) via a
    /// `Cursor`.
    ///
    /// # Errors
    ///
    /// Returns `Err` if reader construction fails (e.g., I/O error or invalid
    /// format-specific content).
    ///
    /// # Example
    ///
    /// ```
    /// use docspec::AnyReader;
    /// use docspec::InputFormat;
    ///
    /// # fn main() -> docspec::Result<()> {
    /// let reader = AnyReader::from_str(InputFormat::Markdown, "# Hello")?;
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    pub fn from_str(format: InputFormat, input: &str) -> Result<Self> {
        #[cfg(not(any(feature = "markdown", feature = "html", feature = "docx")))]
        {
            let _ = input;
            match format {}
        }
        #[cfg(any(feature = "markdown", feature = "html", feature = "docx"))]
        match format {
            #[cfg(feature = "docx")]
            InputFormat::Docx => {
                // DOCX is a binary format; dispatch via from_reader with the raw bytes.
                Self::from_reader(format, Cursor::new(input.as_bytes().to_vec()))
            }
            #[cfg(feature = "html")]
            InputFormat::Html => {
                let stripped = crate::format::strip_bom(input);
                Ok(Self::Html(HtmlReader::from_str(stripped)))
            }
            #[cfg(feature = "markdown")]
            InputFormat::Markdown => {
                let stripped = crate::format::strip_bom(input);
                Ok(Self::Markdown(MarkdownReader::from_str(stripped)))
            }
        }
    }
}

impl EventSource for AnyReader {
    #[inline]
    fn next_event(&mut self) -> Result<Option<Event>> {
        #[cfg(not(any(feature = "markdown", feature = "html", feature = "docx")))]
        {
            match *self {}
        }
        #[cfg(any(feature = "markdown", feature = "html", feature = "docx"))]
        match self {
            #[cfg(feature = "docx")]
            Self::Docx(r) => r.next_event(),
            #[cfg(feature = "html")]
            Self::Html(r) => r.next_event(),
            #[cfg(feature = "markdown")]
            Self::Markdown(r) => r.next_event(),
        }
    }
}

#[cfg(test)]
mod send_static_assertions {
    fn assert_send_static<T: Send + 'static>() {}
    #[test]
    fn any_reader_is_send_static() {
        #[cfg(any(feature = "markdown", feature = "html", feature = "docx"))]
        assert_send_static::<crate::AnyReader>();
    }
    #[cfg(feature = "docx")]
    #[test]
    fn docx_variant_is_send_static() {
        assert_send_static::<crate::AnyReader>();
    }
}
