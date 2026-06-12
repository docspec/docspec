//! Integration tests for the embedded image happy path end-to-end pipeline.
#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::redundant_test_prefix,
    clippy::separated_literal_suffix,
    clippy::tests_outside_test_module,
    clippy::unseparated_literal_suffix,
    clippy::unwrap_used
)]

mod fixture;

use std::io::Cursor;
use std::sync::Arc;

use docspec_blocknote_writer::BlockNoteWriter;
use docspec_core::StackTrackingSink;
use docspec_core::{AssetHandle, Event, EventSink as _, EventSource as _, ImageSource};
use docspec_docx_reader::DocxReader;
use zip::CompressionMethod;

const PNG_SIGNATURE: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

#[derive(Debug)]
struct StubHandle(String);
impl AssetHandle for StubHandle {
    fn asset_id(&self) -> &str {
        &self.0
    }
    fn content_type(&self) -> Option<std::borrow::Cow<'_, str>> {
        None
    }
    fn stream_to(&self, _: &mut dyn std::io::Write) -> std::io::Result<u64> {
        Ok(0)
    }
}
fn asset_source(id: &str) -> ImageSource {
    ImageSource::Asset(Arc::new(StubHandle(id.to_string())))
}

fn root_rels() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#
}

fn doc_rels_with_image() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId4" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png"/>
</Relationships>"#
}

fn content_types_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="png" ContentType="image/png"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#
}

fn document_with_embedded_image() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
    xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
    xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
    xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"
    xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing">
  <w:body><w:p><w:r><w:drawing>
    <wp:inline>
      <wp:docPr descr="alt text"/>
      <a:graphic><a:graphicData>
        <pic:pic><pic:blipFill><a:blip r:embed="rId4"/></pic:blipFill></pic:pic>
      </a:graphicData></a:graphic>
    </wp:inline>
  </w:drawing></w:r></w:p></w:body>
</w:document>"#
}

fn synth_image_docx() -> Vec<u8> {
    fixture::synth_docx_with_entries(&[
        (
            "_rels/.rels",
            CompressionMethod::Deflated,
            root_rels().as_bytes(),
        ),
        (
            "[Content_Types].xml",
            CompressionMethod::Deflated,
            content_types_xml().as_bytes(),
        ),
        (
            "word/_rels/document.xml.rels",
            CompressionMethod::Deflated,
            doc_rels_with_image().as_bytes(),
        ),
        (
            "word/document.xml",
            CompressionMethod::Deflated,
            document_with_embedded_image().as_bytes(),
        ),
        (
            "word/media/image1.png",
            CompressionMethod::Stored,
            &PNG_SIGNATURE,
        ),
    ])
}

fn collect_events(bytes: Vec<u8>) -> Vec<Event> {
    let mut reader = DocxReader::from_reader(Cursor::new(bytes)).unwrap();
    let mut events = Vec::new();
    loop {
        match reader.next_event() {
            Ok(Some(event)) => events.push(event),
            Ok(None) => break,
            Err(e) => panic!("reader error: {e:?}"),
        }
    }
    events
}

#[test]
fn docx_reader_emits_exact_image_event() {
    let bytes = synth_image_docx();
    let events = collect_events(bytes);

    let image_event = events
        .iter()
        .find(|e| matches!(e, Event::Image { .. }))
        .expect("expected at least one Image event");

    assert_eq!(
        *image_event,
        Event::Image {
            source: asset_source("zip://word/media/image1.png"),
            alt: Some("alt text".to_string()),
            decorative: false,
            title: None,
            id: None,
        }
    );
}

#[test]
fn docx_reader_handle_streams_exact_bytes() {
    let bytes = synth_image_docx();
    let events = collect_events(bytes);

    let image_event = events
        .iter()
        .find(|e| matches!(e, Event::Image { .. }))
        .expect("expected at least one Image event");

    if let Event::Image {
        source: ImageSource::Asset(handle),
        ..
    } = image_event
    {
        let mut buf = Vec::new();
        let n = handle
            .stream_to(&mut buf)
            .expect("stream_to should succeed");
        assert_eq!(n, 8u64);
        assert_eq!(buf.as_slice(), PNG_SIGNATURE.as_ref());
    } else {
        panic!("expected ImageSource::Asset, got: {image_event:?}");
    }
}

#[test]
fn blocknote_writer_produces_data_uri_from_handle() {
    let bytes = synth_image_docx();
    let mut reader = DocxReader::from_reader(Cursor::new(bytes)).unwrap();

    let mut out = Vec::<u8>::new();
    let mut writer = StackTrackingSink::new(BlockNoteWriter::new(&mut out));

    loop {
        match reader.next_event() {
            Ok(Some(event)) => writer.handle_event(event).unwrap(),
            Ok(None) => break,
            Err(e) => panic!("reader error: {e:?}"),
        }
    }
    writer.finish().unwrap();

    let json: serde_json::Value =
        serde_json::from_slice(&out).expect("BlockNoteWriter output must be valid JSON");
    let blocks = json.as_array().expect("expected top-level JSON array");

    let image_block = blocks
        .iter()
        .find(|b| b["type"] == "image")
        .expect("expected at least one image block in JSON output");

    let url = image_block["props"]["url"]
        .as_str()
        .expect("expected url field to be a string");

    assert_eq!(url, "data:image/png;base64,iVBORw0KGgo=");
}
