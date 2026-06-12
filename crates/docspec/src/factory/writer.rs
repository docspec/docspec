//! Writer factory for creating writers from output formats.

use std::io::Write;

use docspec_core::{Event, EventSink, Result, StackTrackingSink};

#[cfg(feature = "blocknote-writer")]
use docspec_blocknote_writer::BlockNoteWriter;
#[cfg(feature = "html-writer")]
use docspec_html_writer::HtmlWriter;
#[cfg(feature = "markdown-writer")]
use docspec_markdown_writer::MarkdownWriter;
#[cfg(feature = "oxa-writer")]
use docspec_oxa_writer::OxaWriter;
#[cfg(feature = "pandoc-native-writer")]
use docspec_pandoc_native_writer::PandocNativeWriter;

use crate::format::OutputFormat;

/// Enum-dispatch writer for any registered output format.
///
/// Internally wraps the chosen writer in [`StackTrackingSink`] so callers
/// never need to compose normalization manually. Constructed via
/// [`AnyWriter::new`].
pub struct AnyWriter<W: Write> {
    inner: StackTrackingSink<AnyWriterInner<W>>,
}

enum AnyWriterInner<W: Write> {
    #[cfg(feature = "blocknote-writer")]
    BlockNote(BlockNoteWriter<W>),
    #[cfg(feature = "html-writer")]
    Html(HtmlWriter<W>),
    #[cfg(feature = "oxa-writer")]
    Oxa(OxaWriter<W>),
    #[cfg(feature = "pandoc-native-writer")]
    PandocNative(PandocNativeWriter<W>),
    #[cfg(feature = "markdown-writer")]
    Markdown(MarkdownWriter<W>),
}

impl<W: Write> AnyWriter<W> {
    /// Construct a writer for the given format.
    #[inline]
    #[must_use]
    pub fn new(format: OutputFormat, writer: W) -> Self {
        #[cfg(not(any(
            feature = "blocknote-writer",
            feature = "oxa-writer",
            feature = "html-writer",
            feature = "pandoc-native-writer",
            feature = "markdown-writer"
        )))]
        {
            drop(writer);
            match format {}
        }
        #[cfg(any(
            feature = "blocknote-writer",
            feature = "oxa-writer",
            feature = "html-writer",
            feature = "pandoc-native-writer",
            feature = "markdown-writer"
        ))]
        {
            let inner = match format {
                #[cfg(feature = "blocknote-writer")]
                OutputFormat::Blocknote => AnyWriterInner::BlockNote(BlockNoteWriter::new(writer)),
                #[cfg(feature = "html-writer")]
                OutputFormat::Html => AnyWriterInner::Html(HtmlWriter::new(writer)),
                #[cfg(feature = "oxa-writer")]
                OutputFormat::Oxa => AnyWriterInner::Oxa(OxaWriter::new(writer)),
                #[cfg(feature = "pandoc-native-writer")]
                OutputFormat::PandocNative => {
                    AnyWriterInner::PandocNative(PandocNativeWriter::new(writer))
                }
                #[cfg(feature = "markdown-writer")]
                OutputFormat::Markdown => AnyWriterInner::Markdown(MarkdownWriter::new(writer)),
            };
            Self {
                inner: StackTrackingSink::new(inner),
            }
        }
    }
}

impl<W: Write> EventSink for AnyWriterInner<W> {
    fn finish(self) -> Result<()> {
        match self {
            #[cfg(feature = "blocknote-writer")]
            Self::BlockNote(w) => w.finish(),
            #[cfg(feature = "html-writer")]
            Self::Html(w) => w.finish(),
            #[cfg(feature = "oxa-writer")]
            Self::Oxa(w) => w.finish(),
            #[cfg(feature = "pandoc-native-writer")]
            Self::PandocNative(w) => w.finish(),
            #[cfg(feature = "markdown-writer")]
            Self::Markdown(w) => w.finish(),
            #[cfg(not(feature = "blocknote-writer"))]
            Self::_Phantom(_) => Ok(()),
        }
    }

    fn handle_event(&mut self, event: Event) -> Result<()> {
        match self {
            #[cfg(feature = "blocknote-writer")]
            Self::BlockNote(w) => w.handle_event(event),
            #[cfg(feature = "html-writer")]
            Self::Html(w) => w.handle_event(event),
            #[cfg(feature = "oxa-writer")]
            Self::Oxa(w) => w.handle_event(event),
            #[cfg(feature = "pandoc-native-writer")]
            Self::PandocNative(w) => w.handle_event(event),
            #[cfg(feature = "markdown-writer")]
            Self::Markdown(w) => w.handle_event(event),
            #[cfg(not(feature = "blocknote-writer"))]
            Self::_Phantom(_) => {
                let _ = event;
                Ok(())
            }
        }
    }
}

impl<W: Write> EventSink for AnyWriter<W> {
    #[inline]
    fn finish(self) -> Result<()> {
        self.inner.finish()
    }

    #[inline]
    fn handle_event(&mut self, event: Event) -> Result<()> {
        self.inner.handle_event(event)
    }
}
