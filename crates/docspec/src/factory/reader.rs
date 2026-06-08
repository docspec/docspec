//! Reader factory for creating readers from input formats.

use std::io::{Read, Seek};

use docspec_core::{Event, EventSource, Result};

#[cfg(feature = "html")]
use docspec_html_reader::HtmlReader;
#[cfg(feature = "markdown")]
use docspec_markdown_reader::MarkdownReader;

use crate::format::InputFormat;

/// Enum-dispatch reader for any registered input format.
///
/// Constructed via [`AnyReader::from_str`] or [`AnyReader::from_reader`].
/// Implements [`EventSource`] by delegating `next_event` to the inner concrete
/// reader. Zero heap allocation, zero virtual-dispatch overhead.
///
/// Both `MarkdownReader` and `HtmlReader` are `Send + 'static`, so `AnyReader`
/// is also `Send + 'static` — suitable for use across `tokio::task::spawn_blocking`
/// boundaries.
#[non_exhaustive]
pub enum AnyReader {
    /// HTML reader from [`docspec_html_reader`] (paragraph-only; see crate docs).
    #[cfg(feature = "html")]
    Html(HtmlReader),
    /// Markdown reader from [`docspec_markdown_reader`].
    #[cfg(feature = "markdown")]
    Markdown(MarkdownReader),
}

impl AnyReader {
    /// Construct a reader for the given format from any `Read + Seek` source.
    ///
    /// The `Send + 'static` bounds are required so the resulting `AnyReader`
    /// can be moved across `tokio::task::spawn_blocking` boundaries.
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
        #[cfg(not(any(feature = "markdown", feature = "html")))]
        {
            let _ = reader;
            match format {}
        }
        #[cfg(any(feature = "markdown", feature = "html"))]
        match format {
            #[cfg(feature = "html")]
            InputFormat::Html => Ok(Self::Html(HtmlReader::from_reader(reader)?)),
            #[cfg(feature = "markdown")]
            InputFormat::Markdown => Ok(Self::Markdown(MarkdownReader::from_reader(reader)?)),
        }
    }

    /// Construct a reader for the given format from an in-memory string slice.
    ///
    /// The input is copied into an owned buffer internally. This is the
    /// convenience constructor; for streaming from a `Read` source, use
    /// [`AnyReader::from_reader`].
    ///
    /// # Example
    ///
    /// ```
    /// use docspec::AnyReader;
    /// use docspec::InputFormat;
    ///
    /// let reader = AnyReader::from_str(InputFormat::Markdown, "# Hello");
    /// ```
    #[inline]
    #[must_use]
    pub fn from_str(format: InputFormat, input: &str) -> Self {
        #[cfg(not(any(feature = "markdown", feature = "html")))]
        {
            let _ = input;
            match format {}
        }
        #[cfg(any(feature = "markdown", feature = "html"))]
        match format {
            #[cfg(feature = "html")]
            InputFormat::Html => Self::Html(HtmlReader::from_str(input)),
            #[cfg(feature = "markdown")]
            InputFormat::Markdown => Self::Markdown(MarkdownReader::from_str(input)),
        }
    }
}

impl EventSource for AnyReader {
    #[inline]
    fn next_event(&mut self) -> Result<Option<Event>> {
        #[cfg(not(any(feature = "markdown", feature = "html")))]
        {
            match *self {}
        }
        #[cfg(any(feature = "markdown", feature = "html"))]
        match self {
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
        #[cfg(any(feature = "markdown", feature = "html"))]
        assert_send_static::<crate::AnyReader>();
    }
}
