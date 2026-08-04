//! End-to-end integration test: DOCX → `BlockNote` color pipeline.
//!
//! Verifies that the full DOCX → `BlockNote` conversion correctly maps:
//! - `<w:color>` (text color) → `"textColor"` JSON key
//! - `<w:highlight>` (highlight) → `"backgroundColor"` JSON key
//! - `<w:shd>` (shading) → `"backgroundColor"` JSON key
//! - Black `(0,0,0)` text color → filtered (no `"textColor"` key emitted)
//! - Yellow highlight `(255,255,0)` → `"backgroundColor":"orange"` (counterintuitive palette snap)

#![cfg(all(feature = "docx", feature = "blocknote-writer"))]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::tests_outside_test_module,
    clippy::unused_trait_names,
    clippy::indexing_slicing,
    clippy::doc_markdown,
    clippy::doc_paragraphs_missing_punctuation
)]

use std::io::Cursor;

use serde_json::json;

use docspec::writers::BlockNoteWriter;
use docspec::{AnyReader, EventSink as _, EventSource as _, InputFormat, StackTrackingSink};
use docspec_test_utils::synth_docx;

const SIMPLE_RELS: &str = r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;

/// End-to-end test: DOCX with color, highlight, and shading → BlockNote JSON.
///
/// Run 1: `<w:color w:val="D9730D"/>` (RGB 217,115,13) → text palette `"orange"`
/// Run 2: `<w:highlight w:val="yellow"/>` (RGB 255,255,0) → background palette `"orange"`
///        (counterintuitive: squared-Euclidean distance to bg-orange=47654 < bg-yellow=48121)
/// Run 3: `<w:color w:val="000000"/>` (black, filtered) +
///        `<w:shd w:fill="DDEDEA"/>` (RGB 221,237,234) → background palette `"green"`, no textColor
#[test]
fn docx_color_highlight_shading_round_trip_to_blocknote() {
    let doc_xml = concat!(
        r#"<?xml version="1.0"?>"#,
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">"#,
        r#"<w:body><w:p>"#,
        // Run 1: text color D9730D → textColor "orange"
        r#"<w:r><w:rPr><w:color w:val="D9730D"/></w:rPr><w:t>orange-text</w:t></w:r>"#,
        // Run 2: highlight yellow (255,255,0) → backgroundColor "orange" (counterintuitive!)
        r#"<w:r><w:rPr><w:highlight w:val="yellow"/></w:rPr><w:t>highlighted</w:t></w:r>"#,
        // Run 3: black text (filtered) + shading DDEDEA (221,237,234) → backgroundColor "green"
        r#"<w:r><w:rPr><w:color w:val="000000"/><w:shd w:val="clear" w:fill="DDEDEA"/></w:rPr><w:t>green-bg</w:t></w:r>"#,
        r#"</w:p></w:body></w:document>"#,
    );

    let bytes = synth_docx(SIMPLE_RELS, doc_xml);
    let mut reader = AnyReader::from_reader(InputFormat::Docx, Cursor::new(bytes)).unwrap();

    let mut buf = Vec::<u8>::new();
    let mut writer = StackTrackingSink::new(BlockNoteWriter::new(&mut buf));

    while let Some(event) = reader.next_event().unwrap() {
        writer.handle_event(event).unwrap();
    }
    writer.finish().unwrap();

    let actual: serde_json::Value =
        serde_json::from_slice(&buf).expect("BlockNote output must be valid JSON");

    let content = actual[0]["content"]
        .as_array()
        .expect("first block must have a content array");

    // Run 1: D9730D (RGB 217,115,13) → text palette "orange" → textColor key
    assert_eq!(
        content[0],
        json!({"type": "text", "text": "orange-text", "styles": {"textColor": "orange"}}),
        "run 1: D9730D text color must snap to palette \"orange\""
    );

    // Run 2: yellow highlight (255,255,0) → background palette "orange" (counterintuitive!)
    // Squared-Euclidean distance: bg-orange=47654 < bg-yellow=48121
    assert_eq!(
        content[1],
        json!({"type": "text", "text": "highlighted", "styles": {"backgroundColor": "orange"}}),
        "run 2: yellow highlight (255,255,0) must snap to background palette \"orange\", not \"yellow\""
    );

    // Run 3: black text filtered (no textColor) + shading DDEDEA (221,237,234) → bg palette "green"
    assert_eq!(
        content[2],
        json!({"type": "text", "text": "green-bg", "styles": {"backgroundColor": "green"}}),
        "run 3: DDEDEA shading must snap to background palette \"green\", black text must be filtered (no textColor key)"
    );
}
