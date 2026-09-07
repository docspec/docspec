//! Verifies the public seekable streaming constructor at the crate boundary.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::shadow_unrelated,
    clippy::tests_outside_test_module,
    clippy::unwrap_used
)]

mod fixture;

use std::io::{self, Cursor, Read, Seek, SeekFrom};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use docspec_core::{Error, Event, EventSource as _, ImageSource};
use docspec_docx_reader::DocxReader;
use zip::CompressionMethod;

const ROOT_RELS: &str = r#"<?xml version="1.0"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#;

const DOCUMENT_XML: &str = r#"<?xml version="1.0"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body><w:p><w:r><w:t>streamed</w:t></w:r></w:p></w:body>
</w:document>"#;

fn docx(compression: CompressionMethod) -> Vec<u8> {
    fixture::synth_docx_with_entries(&[
        (
            "_rels/.rels",
            CompressionMethod::Stored,
            ROOT_RELS.as_bytes(),
        ),
        ("word/document.xml", compression, DOCUMENT_XML.as_bytes()),
    ])
}

fn collect(mut reader: DocxReader) -> docspec_core::Result<Vec<Event>> {
    let mut events = Vec::new();
    while let Some(event) = reader.next_event()? {
        events.push(event);
    }
    Ok(events)
}

struct ShortReader {
    inner: Cursor<Vec<u8>>,
}

impl Read for ShortReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let limit = buf.len().min(3);
        self.inner.read(
            buf.get_mut(..limit)
                .ok_or_else(|| io::Error::other("invalid short-read buffer"))?,
        )
    }
}

impl Seek for ShortReader {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.inner.seek(pos)
    }
}

struct SwitchableReader {
    inner: Cursor<Vec<u8>>,
    fail: Arc<AtomicBool>,
}

impl Read for SwitchableReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.fail.load(Ordering::Relaxed) {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "source stopped"));
        }
        self.inner.read(buf)
    }
}

impl Seek for SwitchableReader {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.inner.seek(pos)
    }
}

struct RejectWriter;

impl io::Write for RejectWriter {
    fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::BrokenPipe, "sink stopped"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn streaming_constructor_matches_buffered_constructor_for_stored_and_deflated_entries() {
    for compression in [CompressionMethod::Stored, CompressionMethod::Deflated] {
        let bytes = docx(compression);
        let expected =
            collect(DocxReader::from_reader(Cursor::new(bytes.clone())).unwrap()).unwrap();
        let actual =
            collect(DocxReader::from_reader_streaming(Cursor::new(bytes)).unwrap()).unwrap();
        assert_eq!(actual, expected);
    }
}

#[test]
fn streaming_constructor_accepts_short_source_reads() {
    let expected =
        collect(DocxReader::from_reader(Cursor::new(docx(CompressionMethod::Deflated))).unwrap())
            .unwrap();
    let reader = ShortReader {
        inner: Cursor::new(docx(CompressionMethod::Deflated)),
    };
    let actual = collect(DocxReader::from_reader_streaming(reader).unwrap()).unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn document_source_failure_is_reported_during_consumption() {
    let fail = Arc::new(AtomicBool::new(false));
    let source = SwitchableReader {
        inner: Cursor::new(docx(CompressionMethod::Stored)),
        fail: Arc::clone(&fail),
    };
    let mut reader = DocxReader::from_reader_streaming(source).unwrap();
    fail.store(true, Ordering::Relaxed);
    assert_eq!(
        reader.next_event().unwrap(),
        Some(Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        })
    );
    let error = reader.next_event().expect_err("source read must fail");
    match error {
        Error::Io { source } => {
            assert_eq!(source.kind(), io::ErrorKind::BrokenPipe);
            assert_eq!(source.to_string(), "source stopped");
        }
        other => panic!("expected I/O error, got {other:?}"),
    }
}

#[test]
fn unsupported_document_compression_fails_during_construction() {
    let error = DocxReader::from_reader_streaming(Cursor::new(docx(CompressionMethod::Bzip2)))
        .expect_err("bzip2 is unsupported by the streaming path");
    assert_eq!(
        error.to_string(),
        "parse error: unsupported ZIP compression method: Bzip2"
    );
}

#[test]
fn document_crc_failure_is_reported_before_end_document() {
    let mut bytes = docx(CompressionMethod::Stored);
    let data_start = {
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes.as_slice())).unwrap();
        let entry = archive.by_name("word/document.xml").unwrap();
        usize::try_from(entry.data_start().unwrap()).unwrap()
    };
    let newline = DOCUMENT_XML.find('\n').unwrap();
    bytes[data_start + newline] = b' ';
    let expected_crc = crc32fast::hash(DOCUMENT_XML.as_bytes());
    let actual_crc = crc32fast::hash(&bytes[data_start..data_start + DOCUMENT_XML.len()]);
    let mut reader = DocxReader::from_reader_streaming(Cursor::new(bytes)).unwrap();
    let mut saw_end = false;
    let error = loop {
        match reader.next_event() {
            Ok(Some(Event::EndDocument)) => saw_end = true,
            Ok(Some(_)) => {}
            Ok(None) => panic!("corrupt CRC was accepted"),
            Err(error) => break error,
        }
    };
    assert!(!saw_end);
    match error {
        Error::Io { source } => {
            assert_eq!(source.kind(), io::ErrorKind::InvalidData);
            assert_eq!(
                source.to_string(),
                format!(
                    "ZIP entry CRC mismatch: expected {expected_crc:08x}, calculated {actual_crc:08x}"
                )
            );
        }
        other => panic!("expected I/O error, got {other:?}"),
    }
}

#[test]
fn retained_asset_handle_supports_interleaved_and_repeated_reads() {
    let document = DOCUMENT_XML.replace(
        "<w:r><w:t>streamed</w:t></w:r>",
        r#"<w:r><w:t>before</w:t></w:r><w:r><w:drawing><wp:inline><a:graphic><a:graphicData><pic:pic><pic:blipFill><a:blip r:embed="rId4"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r><w:r><w:t>after</w:t></w:r>"#,
    ).replace(
        "xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"",
        "xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:pic=\"http://schemas.openxmlformats.org/drawingml/2006/picture\" xmlns:wp=\"http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing\"",
    );
    let doc_rels = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId4" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image.bin"/></Relationships>"#;
    let asset_bytes = b"asset payload";
    let bytes = fixture::synth_docx_with_entries(&[
        (
            "_rels/.rels",
            CompressionMethod::Stored,
            ROOT_RELS.as_bytes(),
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
            "word/media/image.bin",
            CompressionMethod::Stored,
            asset_bytes,
        ),
    ]);
    let mut reader = DocxReader::from_reader_streaming(Cursor::new(bytes)).unwrap();
    let handle = loop {
        match reader.next_event().unwrap() {
            Some(Event::Image {
                source: ImageSource::Asset(handle),
                ..
            }) => break handle,
            Some(_) => {}
            None => panic!("image event missing"),
        }
    };
    let mut first = Vec::new();
    assert_eq!(handle.stream_to(&mut first).unwrap(), 13);
    assert_eq!(first, asset_bytes);
    assert_eq!(
        collect(reader).unwrap(),
        vec![
            Event::Text {
                content: "after".to_string(),
            },
            Event::EndParagraph,
            Event::EndDocument,
        ]
    );
    let mut second = Vec::new();
    assert_eq!(handle.stream_to(&mut second).unwrap(), 13);
    assert_eq!(second, asset_bytes);
    let sink_error = handle
        .stream_to(&mut RejectWriter)
        .expect_err("asset sink failure must propagate");
    assert_eq!(sink_error.kind(), io::ErrorKind::BrokenPipe);
    assert_eq!(sink_error.to_string(), "sink stopped");
}
