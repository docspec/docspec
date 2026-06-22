//! Internal test fixtures shared across the docspec workspace.
//!
//! This crate is not published. It exists solely to share fixture helpers
//! between test modules of multiple workspace crates.

// Reason: Helpers panic on internal ZIP write failure to keep test call sites
// terse. Equivalent to the per-file allows used in workspace test modules.
#![allow(clippy::expect_used)]
// Reason: This test-utility crate intentionally panics on programmer errors
// (bad fixture paths, LFS pointers, unhandled enum variants) so call sites
// stay terse and failures are obvious.
#![allow(clippy::panic)]
// Reason: Literal suffix rules have conflicting requirements in this crate
#![allow(clippy::separated_literal_suffix)]

pub mod builders;

use std::io::{Cursor, Write as _};

pub use zip::CompressionMethod;
use zip::{write::SimpleFileOptions, ZipWriter};

mod collect;
mod drive;
pub use collect::{collect_events, try_collect_events};
pub use drive::{drive, try_drive};

mod failing_writer;
pub use failing_writer::FailingWriter;

pub mod snapshot;
pub use snapshot::{
    capture, is_lfs_pointer, AssetDescriptor, CorpusSnapshot, EventSnapshot, Terminal,
};

/// Builds a minimal 2-entry DOCX archive (Deflated) from raw XML strings.
///
/// Entries:
/// - `_rels/.rels` — the relationship file
/// - `word/document.xml` — the main document
///
/// # Panics
///
/// Panics if the ZIP writer fails (should never happen for in-memory buffers).
#[inline]
#[must_use]
pub fn synth_docx(rels_xml: &str, document_xml: &str) -> Vec<u8> {
    synth_docx_with_entries(&[
        (
            "_rels/.rels",
            CompressionMethod::Deflated,
            rels_xml.as_bytes(),
        ),
        (
            "word/document.xml",
            CompressionMethod::Deflated,
            document_xml.as_bytes(),
        ),
    ])
}

/// Builds a DOCX archive with arbitrary entries.
///
/// Each entry is a tuple of `(name, compression_method, bytes)`.
///
/// # Panics
///
/// Panics if the ZIP writer fails (should never happen for in-memory buffers).
#[inline]
#[must_use]
pub fn synth_docx_with_entries(entries: &[(&str, CompressionMethod, &[u8])]) -> Vec<u8> {
    let buf = Cursor::new(Vec::new());
    let mut writer = ZipWriter::new(buf);
    for (name, method, data) in entries {
        let options = SimpleFileOptions::default().compression_method(*method);
        writer
            .start_file(*name, options)
            .expect("start_file failed");
        writer.write_all(data).expect("write_all failed");
    }
    writer.finish().expect("finish failed").into_inner()
}

/// Builds a minimal DOCX archive with an embedded PNG image.
///
/// The synthetic DOCX contains:
/// - `[Content_Types].xml` with PNG content type registration
/// - `_rels/.rels` with root relationships
/// - `word/_rels/document.xml.rels` with image relationship (rId1)
/// - `word/document.xml` with inline image drawing
/// - `word/media/image1.png` with PNG signature bytes (8 bytes)
///
/// The PNG payload is exactly the PNG file signature: `[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]`.
///
/// This helper is useful for testing image asset handling in DOCX readers without
/// requiring real image files or rendering logic.
///
/// # Panics
///
/// Panics if the ZIP writer fails (should never happen for in-memory buffers).
#[inline]
#[must_use]
pub fn synth_docx_with_image_png() -> Vec<u8> {
    const PNG_SIGNATURE: &[u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

    let root_rels = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#;

    let content_types = r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="png" ContentType="image/png"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#;

    let doc_rels = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png"/>
</Relationships>"#;

    let document = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
    xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
    xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
    xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"
    xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing">
  <w:body><w:p><w:r><w:drawing>
    <wp:inline>
      <wp:docPr descr="alt text"/>
      <a:graphic><a:graphicData>
        <pic:pic><pic:blipFill><a:blip r:embed="rId1"/></pic:blipFill></pic:pic>
      </a:graphicData></a:graphic>
    </wp:inline>
  </w:drawing></w:r></w:p></w:body>
</w:document>"#;

    synth_docx_with_entries(&[
        (
            "_rels/.rels",
            CompressionMethod::Deflated,
            root_rels.as_bytes(),
        ),
        (
            "[Content_Types].xml",
            CompressionMethod::Deflated,
            content_types.as_bytes(),
        ),
        (
            "word/_rels/document.xml.rels",
            CompressionMethod::Deflated,
            doc_rels.as_bytes(),
        ),
        (
            "word/document.xml",
            CompressionMethod::Deflated,
            document.as_bytes(),
        ),
        (
            "word/media/image1.png",
            CompressionMethod::Stored,
            PNG_SIGNATURE,
        ),
    ])
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic)]

    use std::borrow::Cow;
    use std::io::Cursor;

    use super::*;

    fn get_image_handle(bytes: Vec<u8>) -> (docspec_core::ImageSource, Option<String>) {
        use docspec_core::{Event, EventSource as _};
        use docspec_docx_reader::DocxReader;

        let mut reader =
            DocxReader::from_reader(Cursor::new(bytes)).expect("should open DOCX with DocxReader");
        loop {
            match reader.next_event() {
                Ok(Some(Event::Image { source, alt, .. })) => return (source, alt),
                Ok(Some(_)) => {}
                Ok(None) => panic!("no Image event found"),
                Err(e) => panic!("reader error: {e:?}"),
            }
        }
    }

    #[test]
    fn synth_docx_with_image_png_handle_has_content_type() {
        let bytes = synth_docx_with_image_png();
        let (source, _) = get_image_handle(bytes);
        if let docspec_core::ImageSource::Asset(handle) = source {
            assert_eq!(handle.content_type(), Some(Cow::Borrowed("image/png")));
        } else {
            panic!("expected ImageSource::Asset");
        }
    }

    #[test]
    fn synth_docx_with_image_png_streams_png_signature() {
        const PNG_SIGNATURE: &[u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

        let bytes = synth_docx_with_image_png();
        let (source, _) = get_image_handle(bytes);
        if let docspec_core::ImageSource::Asset(handle) = source {
            let mut buf = Vec::new();
            let n = handle
                .stream_to(&mut buf)
                .expect("stream_to should succeed");
            assert_eq!(n, 8_u64);
            assert_eq!(buf.as_slice(), PNG_SIGNATURE);
        } else {
            panic!("expected ImageSource::Asset");
        }
    }

    #[test]
    #[allow(clippy::panic)]
    fn synth_docx_with_image_png_emits_image_event() {
        use docspec_core::{Event, EventSource as _, ImageSource};
        use docspec_docx_reader::DocxReader;

        let bytes = synth_docx_with_image_png();
        let mut reader =
            DocxReader::from_reader(Cursor::new(bytes)).expect("should open DOCX with DocxReader");

        let mut found_image_event = false;
        loop {
            match reader.next_event() {
                Ok(Some(event)) => {
                    if let Event::Image {
                        source: ImageSource::Asset(handle),
                        alt,
                        ..
                    } = event
                    {
                        assert_eq!(handle.asset_id(), "zip://word/media/image1.png");
                        assert_eq!(alt, Some("alt text".to_string()));
                        found_image_event = true;
                    }
                }
                Ok(None) => break,
                Err(e) => panic!("reader error: {e:?}"),
            }
        }

        assert!(found_image_event, "expected at least one Image event");
    }
}
