//! Writer factory for creating writers from output formats.

use std::io::Write;

use docspec_core::{AssetProvider, Event, EventSink, Result, StackTrackingSink};

#[cfg(feature = "blocknote")]
use docspec_blocknote_writer::BlockNoteWriter;

use crate::format::OutputFormat;

/// Enum-dispatch writer for any registered output format.
///
/// Internally wraps the chosen writer in [`StackTrackingSink`] so callers
/// never need to compose normalization manually. Constructed via
/// [`AnyWriter::new`] or [`AnyWriter::with_assets`].
pub struct AnyWriter<'a, W: Write> {
    inner: StackTrackingSink<AnyWriterInner<'a, W>>,
}

enum AnyWriterInner<'a, W: Write> {
    #[cfg(feature = "blocknote")]
    BlockNote(BlockNoteWriter<'a, W>),
    #[cfg(not(feature = "blocknote"))]
    _Phantom(std::marker::PhantomData<&'a W>),
}

impl<'a, W: Write> AnyWriter<'a, W> {
    /// Construct a writer for the given format.
    #[inline]
    #[must_use]
    pub fn new(format: OutputFormat, writer: W) -> Self {
        let inner = match format {
            #[cfg(feature = "blocknote")]
            OutputFormat::Blocknote => AnyWriterInner::BlockNote(BlockNoteWriter::new(writer)),
        };
        Self {
            inner: StackTrackingSink::new(inner),
        }
    }

    /// Construct a writer with an asset provider, where supported.
    #[inline]
    #[must_use]
    pub fn with_assets(format: OutputFormat, writer: W, assets: &'a dyn AssetProvider) -> Self {
        let inner = match format {
            #[cfg(feature = "blocknote")]
            OutputFormat::Blocknote => {
                AnyWriterInner::BlockNote(BlockNoteWriter::with_assets(writer, assets))
            }
        };
        Self {
            inner: StackTrackingSink::new(inner),
        }
    }
}

impl<W: Write> EventSink for AnyWriterInner<'_, W> {
    fn finish(self) -> Result<()> {
        match self {
            #[cfg(feature = "blocknote")]
            Self::BlockNote(w) => w.finish(),
            #[cfg(not(feature = "blocknote"))]
            Self::_Phantom(_) => Ok(()),
        }
    }

    fn handle_event(&mut self, event: Event) -> Result<()> {
        match self {
            #[cfg(feature = "blocknote")]
            Self::BlockNote(w) => w.handle_event(event),
            #[cfg(not(feature = "blocknote"))]
            Self::_Phantom(_) => Ok(()),
        }
    }
}

impl<W: Write> EventSink for AnyWriter<'_, W> {
    #[inline]
    fn finish(self) -> Result<()> {
        self.inner.finish()
    }

    #[inline]
    fn handle_event(&mut self, event: Event) -> Result<()> {
        self.inner.handle_event(event)
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;
    use std::io::Write;

    use docspec_core::{AssetProvider, Event, EventSink as _};

    use super::{AnyWriter, OutputFormat};

    struct NullAssets;

    impl AssetProvider for NullAssets {
        fn content_type(&self, _asset_id: &str) -> Option<Cow<'_, str>> {
            None
        }

        fn stream_to(
            &self,
            _asset_id: &str,
            _writer: &mut dyn Write,
        ) -> Option<std::io::Result<u64>> {
            None
        }
    }

    #[cfg(feature = "blocknote")]
    #[test]
    fn with_assets_constructs_writer_for_blocknote() {
        let assets = NullAssets;
        assert!(assets.content_type("any-id").is_none());
        assert!(assets.stream_to("any-id", &mut std::io::sink()).is_none());
        let mut buf = Vec::new();
        let mut writer = AnyWriter::with_assets(OutputFormat::Blocknote, &mut buf, &assets);
        assert!(writer
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        assert!(writer.finish().is_ok());
        assert!(!buf.is_empty());
    }
}
