//! Reader factory for creating readers from input formats.

use docspec_core::{Event, EventSource, Result};

#[cfg(feature = "html")]
use docspec_html_reader::HtmlReader;
#[cfg(feature = "markdown")]
use docspec_markdown_reader::MarkdownReader;

use crate::format::InputFormat;

/// Enum-dispatch reader for any registered input format.
///
/// Constructed via [`AnyReader::new`]. Implements [`EventSource`] by delegating
/// `next_event` to the inner concrete reader. Zero heap allocation, zero
/// virtual-dispatch overhead.
pub enum AnyReader<'a> {
    /// HTML reader from [`docspec_html_reader`] (paragraph-only; see crate docs).
    #[cfg(feature = "html")]
    Html(HtmlReader<'a>),
    /// Markdown reader from [`docspec_markdown_reader`].
    #[cfg(feature = "markdown")]
    Markdown(MarkdownReader<'a>),
    /// Phantom variant when no reader features are active.
    #[cfg(not(any(feature = "markdown", feature = "html")))]
    _Phantom(std::marker::PhantomData<&'a ()>),
}

impl<'a> AnyReader<'a> {
    /// Construct a reader for the given format from an in-memory string.
    #[inline]
    #[must_use]
    pub fn new(format: InputFormat, input: &'a str) -> Self {
        #[cfg(not(any(feature = "markdown", feature = "html", feature = "docx")))]
        {
            let _ = input;
            match format {}
        }
        #[cfg(any(feature = "markdown", feature = "html", feature = "docx"))]
        match format {
            #[cfg(feature = "html")]
            InputFormat::Html => Self::Html(HtmlReader::new(input)),
            #[cfg(feature = "markdown")]
            InputFormat::Markdown => Self::Markdown(MarkdownReader::new(input)),
            #[cfg(feature = "docx")]
            InputFormat::Docx => {
                // Docx reader requires a file path or binary reader, not a string.
                // This arm is unreachable in correct usage; T7 will remove AnyReader::new entirely.
                // Return a dummy value that will never be used in practice.
                #[cfg(feature = "html")]
                {
                    Self::Html(HtmlReader::new(""))
                }
                #[cfg(all(not(feature = "html"), feature = "markdown"))]
                {
                    Self::Markdown(MarkdownReader::new(""))
                }
                #[cfg(all(not(feature = "html"), not(feature = "markdown")))]
                {
                    Self::_Phantom(std::marker::PhantomData)
                }
            }
        }
    }
}

impl EventSource for AnyReader<'_> {
    #[inline]
    fn next_event(&mut self) -> Result<Option<Event>> {
        match self {
            #[cfg(feature = "html")]
            Self::Html(r) => r.next_event(),
            #[cfg(feature = "markdown")]
            Self::Markdown(r) => r.next_event(),
            #[cfg(not(any(feature = "markdown", feature = "html")))]
            Self::_Phantom(_) => Ok(None),
        }
    }
}
