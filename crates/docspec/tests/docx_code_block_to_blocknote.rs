//! End-to-end integration test: DOCX code block → `BlockNote` pipeline.
//!
//! Regression test for issue #351. A code-styled paragraph followed by a paragraph
//! without `<w:pPr>` used to leave the reader's `EndPreformatted` pending, so the
//! trailing paragraph was nested inside the code block and the `BlockNote` writer
//! failed with `cannot finish: open containers remain or no root value written`.

#![cfg(all(feature = "docx", feature = "blocknote-writer"))]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::tests_outside_test_module,
    clippy::unused_trait_names,
    clippy::doc_markdown,
    clippy::doc_paragraphs_missing_punctuation
)]

use std::io::Cursor;

use serde_json::json;

use docspec::writers::BlockNoteWriter;
use docspec::{AnyReader, EventSink as _, EventSource as _, InputFormat, StackTrackingSink};
use docspec_test_utils::{assert_json_eq, synth_docx_with_entries, CompressionMethod};

const ROOT_RELS: &str = r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;

const DOCUMENT_RELS: &str = r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#;

const STYLES: &str = r#"<?xml version="1.0"?><w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:styleId="SourceCode"><w:name w:val="Source Code"/></w:style></w:styles>"#;

fn docx_to_blocknote(body: &str) -> String {
    let document = format!(
        r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{body}</w:body></w:document>"#
    );
    let bytes = synth_docx_with_entries(&[
        (
            "_rels/.rels",
            CompressionMethod::Deflated,
            ROOT_RELS.as_bytes(),
        ),
        (
            "word/_rels/document.xml.rels",
            CompressionMethod::Deflated,
            DOCUMENT_RELS.as_bytes(),
        ),
        (
            "word/styles.xml",
            CompressionMethod::Deflated,
            STYLES.as_bytes(),
        ),
        (
            "word/document.xml",
            CompressionMethod::Deflated,
            document.as_bytes(),
        ),
    ]);
    let mut reader = AnyReader::from_reader(InputFormat::Docx, Cursor::new(bytes)).unwrap();

    let mut buf = Vec::<u8>::new();
    let mut writer = StackTrackingSink::new(BlockNoteWriter::new(&mut buf));
    while let Some(event) = reader.next_event().unwrap() {
        writer.handle_event(event).unwrap();
    }
    writer.finish().unwrap();

    String::from_utf8(buf).expect("BlockNote output must be valid UTF-8")
}

/// `[plain] [SourceCode with a line break] [plain without <w:pPr>]` → three top-level blocks.
#[test]
fn code_paragraph_between_plain_paragraphs_converts_to_sibling_blocks() {
    let body = concat!(
        r#"<w:p><w:r><w:t>This is some code:</w:t></w:r></w:p>"#,
        r#"<w:p><w:pPr><w:pStyle w:val="SourceCode"/></w:pPr>"#,
        r#"<w:r><w:t>alpha</w:t></w:r><w:r><w:br/></w:r><w:r><w:t>beta</w:t></w:r></w:p>"#,
        r#"<w:p><w:r><w:t>from the beginning of the docx reader.</w:t></w:r></w:p>"#,
    );

    assert_json_eq(
        &docx_to_blocknote(body),
        json!([
            {
                "type": "paragraph",
                "content": [{"type": "text", "text": "This is some code:", "styles": {}}],
                "children": []
            },
            {
                "type": "codeBlock",
                "content": [
                    {"type": "text", "text": "alpha", "styles": {}},
                    {"type": "text", "text": "\n", "styles": {}},
                    {"type": "text", "text": "beta", "styles": {}}
                ],
                "children": []
            },
            {
                "type": "paragraph",
                "content": [{
                    "type": "text",
                    "text": "from the beginning of the docx reader.",
                    "styles": {}
                }],
                "children": []
            }
        ]),
    );
}
