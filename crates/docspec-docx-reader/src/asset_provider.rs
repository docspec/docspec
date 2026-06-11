//! DOCX ZIP archive asset provider.

use std::borrow::Cow;
use std::io::{self, Read, Seek, Write};
use std::path::Path;
use std::sync::Mutex;

use docspec_core::{AssetProvider, Error, Result};
use zip::result::ZipError;
use zip::ZipArchive;

use crate::content_types::{self, ContentTypes};

/// Object-safe alias combining [`Read`], [`Seek`], and [`Send`] for use in trait objects.
trait ReadSeek: Read + Seek + Send {}
impl<T: Read + Seek + Send> ReadSeek for T {}

/// Provides streaming access to binary assets stored inside a DOCX ZIP archive.
///
/// Holds the docx ZIP file open until dropped. Uses internal Mutex to serialize
/// concurrent ZIP reads. Not Clone — use `Arc<DocxAssetProvider>` to share.
pub struct DocxAssetProvider {
    archive: Mutex<ZipArchive<Box<dyn ReadSeek + 'static>>>,
    content_types: ContentTypes,
}

impl DocxAssetProvider {
    /// Creates a `DocxAssetProvider` from a file path.
    ///
    /// Opens the DOCX ZIP file and reads `[Content_Types].xml` to build the
    /// content type lookup table.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if the file cannot be opened, or [`Error::Parse`]
    /// if the file is not a valid ZIP archive.
    #[inline]
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = std::fs::File::open(path.as_ref()).map_err(Error::from)?;
        Self::from_reader(file)
    }

    /// Creates a `DocxAssetProvider` from any [`Read`] + [`Seek`] + [`Send`] source.
    ///
    /// The source must be positioned at the start of a valid DOCX (ZIP) archive.
    /// Reads `[Content_Types].xml` to build the content type lookup table.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Parse`] if the input is not a valid ZIP archive, or
    /// [`Error::Io`] for I/O failures when reading `[Content_Types].xml`.
    #[inline]
    pub fn from_reader<R: Read + Seek + Send + 'static>(reader: R) -> Result<Self> {
        let boxed: Box<dyn ReadSeek + 'static> = Box::new(reader);
        let mut archive = ZipArchive::new(boxed).map_err(|err| match err {
            ZipError::InvalidArchive(_) | ZipError::UnsupportedArchive(_) => Error::Parse {
                message: "not a valid ZIP archive".to_string(),
                position: None,
            },
            ZipError::Io(source) => Error::Io { source },
            ZipError::FileNotFound
            | ZipError::InvalidPassword
            | ZipError::CompressionMethodNotSupported(_)
            | _ => Error::Parse {
                message: format!("not a valid ZIP archive: {err}"),
                position: None,
            },
        })?;

        let ct_bytes = match archive.by_name("[Content_Types].xml") {
            Ok(mut entry) => {
                let mut bytes: Vec<u8> = Vec::new();
                io::copy(&mut entry, &mut bytes).map_err(Error::from)?;
                bytes
            }
            Err(_) => Vec::new(),
        };

        let content_types = content_types::parse(&ct_bytes)?;

        Ok(Self {
            archive: Mutex::new(archive),
            content_types,
        })
    }
}

impl AssetProvider for DocxAssetProvider {
    /// Returns the MIME content type for an asset ID with a `zip://` scheme prefix.
    ///
    /// Strips the `zip://` prefix and looks up the path in `[Content_Types].xml`.
    /// Returns `None` if the scheme is not `zip://` or if no content type is registered
    /// for the path.
    #[inline]
    fn content_type(&self, asset_id: &str) -> Option<Cow<'_, str>> {
        asset_id
            .strip_prefix("zip://")
            .and_then(|p| self.content_types.lookup(p))
            .map(Cow::Borrowed)
    }

    /// Streams the asset bytes at `asset_id` to `writer`.
    ///
    /// Strips the `zip://` prefix, acquires the archive mutex, locates the ZIP entry,
    /// and copies bytes via [`io::copy`] — never buffers the full asset. Returns:
    ///
    /// - `None` if `asset_id` does not start with `zip://`
    /// - `None` if the mutex is poisoned
    /// - `None` if the entry is not found in the archive
    /// - `Some(Ok(n))` on success with `n` bytes written
    /// - `Some(Err(_))` on I/O error during copy
    #[inline]
    fn stream_to(&self, asset_id: &str, writer: &mut dyn Write) -> Option<io::Result<u64>> {
        let path = asset_id.strip_prefix("zip://")?;
        let mut archive = self.archive.lock().ok()?;
        let mut entry = archive.by_name(path).ok()?;
        Some(io::copy(&mut entry, writer))
    }
}

#[cfg(test)]
#[cfg(not(coverage))]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::separated_literal_suffix,
        clippy::unseparated_literal_suffix
    )]
    use std::borrow::Cow;
    use std::io::{Cursor, Write as _};
    use zip::write::SimpleFileOptions;
    use zip::CompressionMethod;

    use super::DocxAssetProvider;
    use docspec_core::AssetProvider as _;

    fn synth_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let buf = Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(buf);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        for (name, data) in entries {
            writer.start_file(*name, options).unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    fn content_types_png_xml() -> &'static [u8] {
        br#"<?xml version="1.0"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="png" ContentType="image/png"/>
</Types>"#
    }

    fn synth_png_docx() -> Vec<u8> {
        synth_zip(&[
            ("[Content_Types].xml", content_types_png_xml()),
            ("word/media/image1.png", &[0x89, 0x50, 0x4E, 0x47]),
        ])
    }

    #[test]
    fn is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<DocxAssetProvider>();
    }

    #[test]
    fn stream_to_exact_bytes() {
        let zip_bytes = synth_png_docx();
        let provider = DocxAssetProvider::from_reader(Cursor::new(zip_bytes)).expect("should open");
        let mut buf = Vec::new();
        let result = provider.stream_to("zip://word/media/image1.png", &mut buf);
        assert_eq!(
            result.expect("should return Some").expect("should be Ok"),
            4u64
        );
        assert_eq!(buf, &[0x89, 0x50, 0x4E, 0x47]);
    }

    #[test]
    fn content_type_from_default() {
        let zip_bytes = synth_png_docx();
        let provider = DocxAssetProvider::from_reader(Cursor::new(zip_bytes)).expect("should open");
        let ct = provider.content_type("zip://word/media/image1.png");
        assert_eq!(ct, Some(Cow::Borrowed("image/png")));
    }

    #[test]
    fn non_zip_scheme_returns_none() {
        let zip_bytes = synth_png_docx();
        let provider = DocxAssetProvider::from_reader(Cursor::new(zip_bytes)).expect("should open");
        assert_eq!(provider.content_type("rId99"), None);
        let mut buf = Vec::new();
        assert!(provider.stream_to("rId99", &mut buf).is_none());
    }

    #[test]
    fn missing_asset_stream_returns_none() {
        let zip_bytes = synth_png_docx();
        let provider = DocxAssetProvider::from_reader(Cursor::new(zip_bytes)).expect("should open");
        let mut buf = Vec::new();
        assert!(provider
            .stream_to("zip://word/media/noexist.png", &mut buf)
            .is_none());
    }

    #[test]
    fn content_type_returns_none_for_unregistered_extension() {
        let zip_bytes = synth_png_docx();
        let provider = DocxAssetProvider::from_reader(Cursor::new(zip_bytes)).expect("should open");
        assert_eq!(provider.content_type("zip://word/document.xml"), None);
    }

    #[test]
    fn from_path_opens_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("test.docx");
        let zip_bytes = synth_png_docx();
        std::fs::write(&path, &zip_bytes).expect("write file");
        let provider = DocxAssetProvider::from_path(&path).expect("should open");
        let ct = provider.content_type("zip://word/media/image1.png");
        assert_eq!(ct, Some(Cow::Borrowed("image/png")));
    }

    #[test]
    fn missing_content_types_yields_empty_lookup() {
        let zip_bytes = synth_zip(&[("word/media/image1.png", &[0x89, 0x50, 0x4E, 0x47])]);
        let provider = DocxAssetProvider::from_reader(Cursor::new(zip_bytes)).expect("should open");
        assert_eq!(provider.content_type("zip://word/media/image1.png"), None);
        let mut buf = Vec::new();
        let result = provider.stream_to("zip://word/media/image1.png", &mut buf);
        assert_eq!(
            result.expect("should return Some").expect("should be Ok"),
            4u64
        );
        assert_eq!(buf, &[0x89, 0x50, 0x4E, 0x47]);
    }
}
