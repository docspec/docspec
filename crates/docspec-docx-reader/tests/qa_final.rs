//! F3: Real Manual QA — end-to-end `DocxReader` scenarios with in-memory DOCX fixtures.
#![allow(
    clippy::arbitrary_source_item_ordering,
    clippy::expect_used,
    clippy::panic,
    clippy::redundant_test_prefix,
    clippy::std_instead_of_core,
    clippy::tests_outside_test_module,
    clippy::unwrap_used,
    dead_code
)]

mod fixture;

use std::io::Cursor;

use docspec_core::{Event, EventSource as _, TextStyleKind};
use docspec_docx_reader::DocxReader;

const ROOT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1"
    Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument"
    Target="word/document.xml"/>
</Relationships>"#;

const DOC_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1"
    Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles"
    Target="styles.xml"/>
</Relationships>"#;

fn synth_docx_with_styles(document_xml: &str, styles_xml: &str) -> Vec<u8> {
    fixture::synth_docx_with_entries(&[
        (
            "_rels/.rels",
            zip::CompressionMethod::Deflated,
            ROOT_RELS.as_bytes(),
        ),
        (
            "word/_rels/document.xml.rels",
            zip::CompressionMethod::Deflated,
            DOC_RELS.as_bytes(),
        ),
        (
            "word/document.xml",
            zip::CompressionMethod::Deflated,
            document_xml.as_bytes(),
        ),
        (
            "word/styles.xml",
            zip::CompressionMethod::Deflated,
            styles_xml.as_bytes(),
        ),
    ])
}

fn drive(reader: &mut DocxReader) -> Vec<Event> {
    let mut events = Vec::new();
    while let Some(event) = reader.next_event().expect("next_event") {
        events.push(event);
    }
    events
}

#[test]
fn scenario_1_heading_paragraph_emits_start_heading_level_1() {
    let styles_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:style w:type="paragraph" w:styleId="Heading1">
    <w:name w:val="heading 1"/>
  </w:style>
</w:styles>"#;

    let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr><w:pStyle w:val="Heading1"/></w:pPr>
      <w:r><w:t>My Heading</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;

    let bytes = synth_docx_with_styles(document_xml, styles_xml);
    let mut reader = DocxReader::from_reader(Cursor::new(bytes)).expect("from_reader");
    let events = drive(&mut reader);

    assert_eq!(
        events,
        vec![
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartHeading { level: 1, id: None },
            Event::Text {
                content: "My Heading".to_string(),
            },
            Event::EndHeading,
            Event::EndDocument,
        ]
    );
}

#[test]
fn scenario_2_block_quote_paragraph_emits_start_block_quote() {
    let styles_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:style w:type="paragraph" w:styleId="Quote">
    <w:name w:val="Block Quote"/>
  </w:style>
</w:styles>"#;

    let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr><w:pStyle w:val="Quote"/></w:pPr>
      <w:r><w:t>Some quoted text</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;

    let bytes = synth_docx_with_styles(document_xml, styles_xml);
    let mut reader = DocxReader::from_reader(Cursor::new(bytes)).expect("from_reader");
    let events = drive(&mut reader);

    assert_eq!(
        events,
        vec![
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartBlockQuote { id: None },
            Event::Text {
                content: "Some quoted text".to_string(),
            },
            Event::EndBlockQuote,
            Event::EndDocument,
        ]
    );
}

#[test]
fn scenario_3_preformatted_paragraph_emits_start_preformatted() {
    let styles_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:style w:type="paragraph" w:styleId="Code1">
    <w:name w:val="Source Code"/>
  </w:style>
</w:styles>"#;

    let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr><w:pStyle w:val="Code1"/></w:pPr>
      <w:r><w:t>fn main() {}</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;

    let bytes = synth_docx_with_styles(document_xml, styles_xml);
    let mut reader = DocxReader::from_reader(Cursor::new(bytes)).expect("from_reader");
    let events = drive(&mut reader);

    assert_eq!(
        events,
        vec![
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartPreformatted {
                id: None,
                syntax: None,
            },
            Event::Text {
                content: "fn main() {}".to_string(),
            },
            Event::EndPreformatted,
            Event::EndDocument,
        ]
    );
}

#[test]
fn scenario_4_inline_code_run_emits_start_text_style_code() {
    let styles_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:style w:type="character" w:styleId="CodeChar">
    <w:name w:val="Source Code"/>
  </w:style>
</w:styles>"#;

    let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:r>
        <w:rPr><w:rStyle w:val="CodeChar"/></w:rPr>
        <w:t>inline_code</w:t>
      </w:r>
    </w:p>
  </w:body>
</w:document>"#;

    let bytes = synth_docx_with_styles(document_xml, styles_xml);
    let mut reader = DocxReader::from_reader(Cursor::new(bytes)).expect("from_reader");
    let events = drive(&mut reader);

    assert_eq!(
        events,
        vec![
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
            Event::StartTextStyle {
                kind: TextStyleKind::Code,
                id: None,
            },
            Event::Text {
                content: "inline_code".to_string(),
            },
            Event::EndTextStyle,
            Event::EndParagraph,
            Event::EndDocument,
        ]
    );
}

#[test]
fn scenario_5_no_styles_xml_and_no_doc_rels_emits_only_start_paragraph() {
    let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr><w:pStyle w:val="Heading1"/></w:pPr>
      <w:r><w:t>Would-be heading</w:t></w:r>
    </w:p>
    <w:p>
      <w:r><w:t>Plain paragraph</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;

    let bytes = fixture::synth_docx_with_entries(&[
        (
            "_rels/.rels",
            zip::CompressionMethod::Deflated,
            ROOT_RELS.as_bytes(),
        ),
        (
            "word/document.xml",
            zip::CompressionMethod::Deflated,
            document_xml.as_bytes(),
        ),
    ]);

    let mut reader = DocxReader::from_reader(Cursor::new(bytes)).expect("from_reader");
    let events = drive(&mut reader);

    assert_eq!(
        events,
        vec![
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
            Event::Text {
                content: "Would-be heading".to_string(),
            },
            Event::EndParagraph,
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
            Event::Text {
                content: "Plain paragraph".to_string(),
            },
            Event::EndParagraph,
            Event::EndDocument,
        ]
    );
}
