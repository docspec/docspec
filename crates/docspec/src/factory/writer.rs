//! Writer factory for creating writers from output formats.

use std::io::Write;

use docspec_core::{AssetProvider, Event, EventSink, Result, StackTrackingSink};

#[cfg(feature = "blocknote-writer")]
use docspec_blocknote_writer::BlockNoteWriter;
#[cfg(feature = "html-writer")]
use docspec_html_writer::HtmlWriter;
#[cfg(feature = "oxa-writer")]
use docspec_oxa_writer::OxaWriter;
#[cfg(feature = "pandoc-native-writer")]
use docspec_pandoc_native_writer::PandocNativeWriter;

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
    #[cfg(feature = "blocknote-writer")]
    BlockNote(BlockNoteWriter<'a, W>),
    #[cfg(feature = "html-writer")]
    Html(HtmlWriter<W>),
    #[cfg(feature = "oxa-writer")]
    Oxa(OxaWriter<W>),
    #[cfg(feature = "pandoc-native-writer")]
    PandocNative(PandocNativeWriter<W>),
    #[cfg(not(feature = "blocknote-writer"))]
    _Phantom(std::marker::PhantomData<&'a W>),
}

impl<'a, W: Write> AnyWriter<'a, W> {
    /// Construct a writer for the given format.
    #[inline]
    #[must_use]
    pub fn new(format: OutputFormat, writer: W) -> Self {
        #[cfg(not(any(
            feature = "blocknote-writer",
            feature = "oxa-writer",
            feature = "html-writer",
            feature = "pandoc-native-writer"
        )))]
        {
            drop(writer);
            match format {}
        }
        #[cfg(any(
            feature = "blocknote-writer",
            feature = "oxa-writer",
            feature = "html-writer",
            feature = "pandoc-native-writer"
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
            };
            Self {
                inner: StackTrackingSink::new(inner),
            }
        }
    }

    /// Construct a writer with an asset provider, where supported.
    ///
    /// Writers that do not consume an [`AssetProvider`] (currently `OxaWriter`,
    /// `HtmlWriter`, and `PandocNativeWriter`) silently ignore the `assets` argument;
    /// the resulting
    /// writer behaves identically to [`AnyWriter::new`].
    #[inline]
    #[must_use]
    pub fn with_assets(format: OutputFormat, writer: W, assets: &'a dyn AssetProvider) -> Self {
        #[cfg(not(any(
            feature = "blocknote-writer",
            feature = "oxa-writer",
            feature = "html-writer",
            feature = "pandoc-native-writer"
        )))]
        {
            drop(writer);
            let _ = assets;
            match format {}
        }
        #[cfg(any(
            feature = "blocknote-writer",
            feature = "oxa-writer",
            feature = "html-writer",
            feature = "pandoc-native-writer"
        ))]
        {
            let inner = match format {
                #[cfg(feature = "blocknote-writer")]
                OutputFormat::Blocknote => {
                    AnyWriterInner::BlockNote(BlockNoteWriter::with_assets(writer, assets))
                }
                #[cfg(feature = "html-writer")]
                OutputFormat::Html => {
                    let _ = assets;
                    AnyWriterInner::Html(HtmlWriter::new(writer))
                }
                #[cfg(feature = "oxa-writer")]
                OutputFormat::Oxa => {
                    let _ = assets;
                    AnyWriterInner::Oxa(OxaWriter::new(writer))
                }
                #[cfg(feature = "pandoc-native-writer")]
                OutputFormat::PandocNative => {
                    let _ = assets;
                    AnyWriterInner::PandocNative(PandocNativeWriter::new(writer))
                }
            };
            Self {
                inner: StackTrackingSink::new(inner),
            }
        }
    }
}

impl<W: Write> EventSink for AnyWriterInner<'_, W> {
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
            #[cfg(not(feature = "blocknote-writer"))]
            Self::_Phantom(_) => {
                let _ = event;
                Ok(())
            }
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

    use docspec_core::AssetProvider;
    #[cfg(any(
        feature = "blocknote-writer",
        feature = "html-writer",
        feature = "oxa-writer",
        feature = "pandoc-native-writer"
    ))]
    use docspec_core::{Event, EventSink as _};

    #[cfg(any(
        feature = "blocknote-writer",
        feature = "html-writer",
        feature = "oxa-writer",
        feature = "pandoc-native-writer"
    ))]
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

    #[cfg(feature = "blocknote-writer")]
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

    #[cfg(feature = "oxa-writer")]
    #[test]
    fn with_assets_constructs_writer_for_oxa() {
        let assets = NullAssets;
        let mut buf = Vec::new();
        let mut writer = AnyWriter::with_assets(OutputFormat::Oxa, &mut buf, &assets);
        assert!(writer
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        assert!(writer.finish().is_ok());
        assert_eq!(buf, br#"{"type":"Document","children":[]}"#);
    }

    #[cfg(feature = "html-writer")]
    #[test]
    fn with_assets_ignores_provider_for_html_writer() {
        let assets = NullAssets;
        let mut buf = Vec::new();
        let mut writer = AnyWriter::with_assets(OutputFormat::Html, &mut buf, &assets);
        assert!(writer
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        assert!(writer.finish().is_ok());
        assert_eq!(buf, b"<html><body></body></html>");
    }

    #[cfg(feature = "pandoc-native-writer")]
    #[test]
    fn with_assets_ignores_provider_for_pandoc_native_writer() {
        let assets = NullAssets;
        let mut buf = Vec::new();
        let mut writer = AnyWriter::with_assets(OutputFormat::PandocNative, &mut buf, &assets);
        assert!(writer
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        assert!(writer.finish().is_ok());
        assert_eq!(buf, b"[]");
    }
}
