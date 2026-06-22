//! Integration tests for `DocxReader` image edge cases.
#![allow(
    clippy::arbitrary_source_item_ordering,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::redundant_test_prefix,
    clippy::separated_literal_suffix,
    clippy::std_instead_of_core,
    clippy::tests_outside_test_module,
    clippy::unseparated_literal_suffix,
    clippy::unwrap_used
)]

mod fixture;

use std::io::Cursor;
use std::sync::Arc;

use docspec_core::{AssetHandle, Event, EventSource as _, ImageSource};
use docspec_docx_reader::DocxReader;
use zip::CompressionMethod;

const ROOT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1"
    Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument"
    Target="word/document.xml"/>
</Relationships>"#;

const IMAGE_REL_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";

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

fn doc_rels(body: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
{body}
</Relationships>"#
    )
}

fn drawing_doc(drawing_inner: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document
  xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
  xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
  xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
  xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"
  xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing">
  <w:body><w:p><w:r><w:drawing>{drawing_inner}</w:drawing></w:r></w:p></w:body>
</w:document>"#
    )
}

/// Wraps `pict_inner` XML into a minimal DOCX document XML string containing one paragraph with one VML `<w:pict>`.
/// Namespace declarations for VML, Office, MC, and Word namespaces are included on the root element.
fn pict_doc(pict_inner: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:wpc="http://schemas.microsoft.com/office/word/2010/wordprocessingCanvas"
  xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"
  xmlns:v="urn:schemas-microsoft-com:vml"
  xmlns:o="urn:schemas-microsoft-com:office:office"
  xmlns:w10="urn:schemas-microsoft-com:office:word"
  xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
  xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body>
    <w:p>
      <w:r>
        <w:pict>{pict_inner}</w:pict>
      </w:r>
    </w:p>
  </w:body>
</w:document>"#
    )
}

fn inline_pic(blip: &str) -> String {
    format!(
        "<wp:inline><a:graphic><a:graphicData><pic:pic><pic:blipFill>{blip}</pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:inline>"
    )
}

fn build_docx(doc_rels_xml: &str, document_xml: &str) -> Vec<u8> {
    fixture::synth_docx_with_entries(&[
        (
            "_rels/.rels",
            CompressionMethod::Deflated,
            ROOT_RELS.as_bytes(),
        ),
        (
            "word/_rels/document.xml.rels",
            CompressionMethod::Deflated,
            doc_rels_xml.as_bytes(),
        ),
        (
            "word/document.xml",
            CompressionMethod::Deflated,
            document_xml.as_bytes(),
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

fn start_doc() -> Event {
    Event::StartDocument {
        id: None,
        language: None,
        metadata: None,
    }
}

fn start_para() -> Event {
    Event::StartParagraph {
        alignment: None,
        id: None,
    }
}

fn image_event(source: ImageSource, alt: Option<&str>) -> Event {
    Event::Image {
        alt: alt.map(str::to_string),
        decorative: false,
        id: None,
        source,
        title: None,
    }
}

#[test]
fn external_link_emits_uri() {
    let rels = doc_rels(&format!(
        r#"<Relationship Id="rId2" Type="{IMAGE_REL_TYPE}" Target="https://example.com/img.png" TargetMode="External"/>"#
    ));
    let drawing = inline_pic(r#"<a:blip r:link="rId2"/>"#);
    let doc = drawing_doc(&drawing);
    let bytes = build_docx(&rels, &doc);
    let mut reader = DocxReader::from_reader(Cursor::new(bytes)).expect("from_reader");
    assert_eq!(
        drive(&mut reader),
        vec![
            start_doc(),
            start_para(),
            image_event(
                ImageSource::Uri {
                    uri: "https://example.com/img.png".to_string(),
                },
                None,
            ),
            Event::EndParagraph,
            Event::EndDocument,
        ]
    );
}

#[test]
fn embed_external_target_mode() {
    let rels = doc_rels(&format!(
        r#"<Relationship Id="rId5" Type="{IMAGE_REL_TYPE}" Target="https://cdn.example.com/x.png" TargetMode="External"/>"#
    ));
    let drawing = inline_pic(r#"<a:blip r:embed="rId5"/>"#);
    let doc = drawing_doc(&drawing);
    let bytes = build_docx(&rels, &doc);
    let mut reader = DocxReader::from_reader(Cursor::new(bytes)).expect("from_reader");
    assert_eq!(
        drive(&mut reader),
        vec![
            start_doc(),
            start_para(),
            image_event(
                ImageSource::Uri {
                    uri: "https://cdn.example.com/x.png".to_string(),
                },
                None,
            ),
            Event::EndParagraph,
            Event::EndDocument,
        ]
    );
}

#[test]
fn missing_rel_emits_raw_rid() {
    let rels = doc_rels("");
    let drawing = inline_pic(r#"<a:blip r:embed="rId99"/>"#);
    let doc = drawing_doc(&drawing);
    let bytes = build_docx(&rels, &doc);
    let mut reader = DocxReader::from_reader(Cursor::new(bytes)).expect("from_reader");
    assert_eq!(
        drive(&mut reader),
        vec![
            start_doc(),
            start_para(),
            image_event(asset_source("rId99"), None),
            Event::EndParagraph,
            Event::EndDocument,
        ]
    );
}

#[test]
fn empty_drawing_no_event() {
    let rels = doc_rels("");
    let drawing = r#"<wp:inline><wp:docPr id="1" name="img1"/></wp:inline>"#;
    let doc = drawing_doc(drawing);
    let bytes = build_docx(&rels, &doc);
    let mut reader = DocxReader::from_reader(Cursor::new(bytes)).expect("from_reader");
    assert_eq!(
        drive(&mut reader),
        vec![
            start_doc(),
            start_para(),
            Event::EndParagraph,
            Event::EndDocument,
        ]
    );
}

#[test]
fn embed_wins_over_link() {
    let rels = doc_rels(&format!(
        r#"<Relationship Id="rId4" Type="{IMAGE_REL_TYPE}" Target="media/embed.png"/>
<Relationship Id="rId5" Type="{IMAGE_REL_TYPE}" Target="https://example.com/link.png" TargetMode="External"/>"#
    ));
    let drawing = inline_pic(r#"<a:blip r:embed="rId4" r:link="rId5"/>"#);
    let doc = drawing_doc(&drawing);
    let bytes = build_docx(&rels, &doc);
    let mut reader = DocxReader::from_reader(Cursor::new(bytes)).expect("from_reader");
    assert_eq!(
        drive(&mut reader),
        vec![
            start_doc(),
            start_para(),
            image_event(asset_source("zip://word/media/embed.png"), None),
            Event::EndParagraph,
            Event::EndDocument,
        ]
    );
}

#[test]
fn pict_without_imagedata_emits_nothing() {
    let rels = doc_rels("");
    let doc = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document
  xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
  xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
  xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
  xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"
  xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing">
  <w:body>
    <w:p>
      <w:r><w:t>before</w:t></w:r>
      <w:r><w:drawing><w:pict><w:r><w:t>hidden</w:t></w:r></w:pict></w:drawing></w:r>
      <w:r><w:t>after</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;
    let bytes = build_docx(&rels, doc);
    let mut reader = DocxReader::from_reader(Cursor::new(bytes)).expect("from_reader");
    assert_eq!(
        drive(&mut reader),
        vec![
            start_doc(),
            start_para(),
            Event::Text {
                content: "before".to_string(),
            },
            Event::Text {
                content: "after".to_string(),
            },
            Event::EndParagraph,
            Event::EndDocument,
        ]
    );
}

#[test]
fn pict_with_imagedata_r_embed_emits_image_event() {
    let xml = pict_doc(r#"<v:shape><v:imagedata r:embed="rId2"/></v:shape>"#);
    let rels = doc_rels(&format!(
        r#"<Relationship Id="rId2" Type="{IMAGE_REL_TYPE}" Target="media/image1.png"/>"#
    ));
    let bytes = build_docx(&rels, &xml);
    let mut reader = DocxReader::from_reader(Cursor::new(bytes)).expect("from_reader");
    assert_eq!(
        drive(&mut reader),
        vec![
            start_doc(),
            start_para(),
            image_event(asset_source("zip://word/media/image1.png"), None),
            Event::EndParagraph,
            Event::EndDocument,
        ]
    );
}

#[test]
fn pict_with_imagedata_r_link_external_emits_uri() {
    let xml = pict_doc(r#"<v:shape><v:imagedata r:link="rId2"/></v:shape>"#);
    let rels = doc_rels(&format!(
        r#"<Relationship Id="rId2" Type="{IMAGE_REL_TYPE}" Target="https://example.com/img.png" TargetMode="External"/>"#
    ));
    let bytes = build_docx(&rels, &xml);
    let mut reader = DocxReader::from_reader(Cursor::new(bytes)).expect("from_reader");
    assert_eq!(
        drive(&mut reader),
        vec![
            start_doc(),
            start_para(),
            image_event(
                ImageSource::Uri {
                    uri: "https://example.com/img.png".to_string(),
                },
                None,
            ),
            Event::EndParagraph,
            Event::EndDocument,
        ]
    );
}

#[test]
fn pict_imagedata_r_id_external_target_mode_emits_uri() {
    let xml = pict_doc(r#"<v:shape><v:imagedata r:id="rId2"/></v:shape>"#);
    let rels = doc_rels(&format!(
        r#"<Relationship Id="rId2" Type="{IMAGE_REL_TYPE}" Target="https://example.com/img.png" TargetMode="External"/>"#
    ));
    let bytes = build_docx(&rels, &xml);
    let mut reader = DocxReader::from_reader(Cursor::new(bytes)).expect("from_reader");
    assert_eq!(
        drive(&mut reader),
        vec![
            start_doc(),
            start_para(),
            image_event(
                ImageSource::Uri {
                    uri: "https://example.com/img.png".to_string(),
                },
                None,
            ),
            Event::EndParagraph,
            Event::EndDocument,
        ]
    );
}

#[test]
fn pict_imagedata_o_title_populates_alt() {
    let xml = pict_doc(r#"<v:shape><v:imagedata r:id="rId2" o:title="My Image"/></v:shape>"#);
    let rels = doc_rels(&format!(
        r#"<Relationship Id="rId2" Type="{IMAGE_REL_TYPE}" Target="media/image1.png"/>"#
    ));
    let bytes = build_docx(&rels, &xml);
    let mut reader = DocxReader::from_reader(Cursor::new(bytes)).expect("from_reader");
    assert_eq!(
        drive(&mut reader),
        vec![
            start_doc(),
            start_para(),
            image_event(
                asset_source("zip://word/media/image1.png"),
                Some("My Image")
            ),
            Event::EndParagraph,
            Event::EndDocument,
        ]
    );
}

#[test]
fn pict_empty_o_title_falls_back_to_shape_alt() {
    let xml =
        pict_doc(r#"<v:shape alt="Shape Alt"><v:imagedata r:id="rId2" o:title=""/></v:shape>"#);
    let rels = doc_rels(&format!(
        r#"<Relationship Id="rId2" Type="{IMAGE_REL_TYPE}" Target="media/image1.png"/>"#
    ));
    let bytes = build_docx(&rels, &xml);
    let mut reader = DocxReader::from_reader(Cursor::new(bytes)).expect("from_reader");
    assert_eq!(
        drive(&mut reader),
        vec![
            start_doc(),
            start_para(),
            image_event(
                asset_source("zip://word/media/image1.png"),
                Some("Shape Alt")
            ),
            Event::EndParagraph,
            Event::EndDocument,
        ]
    );
}

#[test]
fn pict_multiple_imagedata_emit_multiple_images_in_order() {
    let xml = pict_doc(
        r#"<v:shape><v:imagedata r:id="rId2"/></v:shape><v:shape><v:imagedata r:id="rId3"/></v:shape>"#,
    );
    let rels = doc_rels(&format!(
        r#"<Relationship Id="rId2" Type="{IMAGE_REL_TYPE}" Target="media/image1.png"/>
<Relationship Id="rId3" Type="{IMAGE_REL_TYPE}" Target="media/image2.png"/>"#
    ));
    let bytes = build_docx(&rels, &xml);
    let mut reader = DocxReader::from_reader(Cursor::new(bytes)).expect("from_reader");
    assert_eq!(
        drive(&mut reader),
        vec![
            start_doc(),
            start_para(),
            image_event(asset_source("zip://word/media/image1.png"), None),
            image_event(asset_source("zip://word/media/image2.png"), None),
            Event::EndParagraph,
            Event::EndDocument,
        ]
    );
}

#[test]
fn pict_imagedata_inside_v_group_still_emits() {
    let xml = pict_doc(r#"<v:group><v:shape><v:imagedata r:id="rId2"/></v:shape></v:group>"#);
    let rels = doc_rels(&format!(
        r#"<Relationship Id="rId2" Type="{IMAGE_REL_TYPE}" Target="media/image1.png"/>"#
    ));
    let bytes = build_docx(&rels, &xml);
    let mut reader = DocxReader::from_reader(Cursor::new(bytes)).expect("from_reader");
    assert_eq!(
        drive(&mut reader),
        vec![
            start_doc(),
            start_para(),
            image_event(asset_source("zip://word/media/image1.png"), None),
            Event::EndParagraph,
            Event::EndDocument,
        ]
    );
}

#[test]
fn pict_imagedata_wordml_src_emits_no_event() {
    let xml = pict_doc(r#"<v:shape><v:imagedata src="wordml://image1.png"/></v:shape>"#);
    let rels = doc_rels("");
    let bytes = build_docx(&rels, &xml);
    let mut reader = DocxReader::from_reader(Cursor::new(bytes)).expect("from_reader");
    assert_eq!(
        drive(&mut reader),
        vec![
            start_doc(),
            start_para(),
            Event::EndParagraph,
            Event::EndDocument,
        ]
    );
}

#[test]
fn pict_imagedata_unresolvable_rid_emits_raw_rid_asset() {
    let xml = pict_doc(r#"<v:shape><v:imagedata r:id="rId999"/></v:shape>"#);
    let rels = doc_rels("");
    let bytes = build_docx(&rels, &xml);
    let mut reader = DocxReader::from_reader(Cursor::new(bytes)).expect("from_reader");
    assert_eq!(
        drive(&mut reader),
        vec![
            start_doc(),
            start_para(),
            image_event(asset_source("rId999"), None),
            Event::EndParagraph,
            Event::EndDocument,
        ]
    );
}

#[test]
fn pict_non_image_shapes_emit_nothing() {
    let xml = pict_doc("<v:rect/><v:line/>");
    let rels = doc_rels("");
    let bytes = build_docx(&rels, &xml);
    let mut reader = DocxReader::from_reader(Cursor::new(bytes)).expect("from_reader");
    assert_eq!(
        drive(&mut reader),
        vec![
            start_doc(),
            start_para(),
            Event::EndParagraph,
            Event::EndDocument,
        ]
    );
}

#[test]
fn alternate_content_with_drawing_choice_and_pict_fallback_emits_once() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"
  xmlns:v="urn:schemas-microsoft-com:vml"
  xmlns:o="urn:schemas-microsoft-com:office:office"
  xmlns:w10="urn:schemas-microsoft-com:office:word"
  xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
  xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
  xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
  xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
  xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture">
  <w:body>
    <w:p>
      <w:r>
        <mc:AlternateContent>
          <mc:Choice Requires="wpc">
            <w:drawing>
              <wp:inline>
                <a:graphic>
                  <a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture">
                    <pic:pic>
                      <pic:blipFill>
                        <a:blip r:embed="rId2"/>
                      </pic:blipFill>
                      <pic:spPr/>
                    </pic:pic>
                  </a:graphicData>
                </a:graphic>
              </wp:inline>
            </w:drawing>
          </mc:Choice>
          <mc:Fallback>
            <w:pict>
              <v:shape>
                <v:imagedata r:id="rId2"/>
              </v:shape>
            </w:pict>
          </mc:Fallback>
        </mc:AlternateContent>
      </w:r>
    </w:p>
  </w:body>
</w:document>"#;
    let rels = doc_rels(&format!(
        r#"<Relationship Id="rId2" Type="{IMAGE_REL_TYPE}" Target="media/image1.png"/>"#
    ));
    let bytes = build_docx(&rels, xml);
    let mut reader = DocxReader::from_reader(Cursor::new(bytes)).expect("from_reader");
    let events = drive(&mut reader);
    let image_count = events
        .iter()
        .filter(|event| matches!(event, Event::Image { .. }))
        .count();
    assert_eq!(
        image_count, 1,
        "Expected exactly 1 Image event, got {image_count}. Events: {events:?}"
    );
}

#[test]
fn smartart_blip_ignored() {
    let rels = doc_rels(&format!(
        r#"<Relationship Id="rIdSmart" Type="{IMAGE_REL_TYPE}" Target="word/media/smart.png"/>"#
    ));
    let drawing = r#"<wp:inline><a:graphic><a:graphicData><a:blip r:embed="rIdSmart"/></a:graphicData></a:graphic></wp:inline>"#;
    let doc = drawing_doc(drawing);
    let bytes = build_docx(&rels, &doc);
    let mut reader = DocxReader::from_reader(Cursor::new(bytes)).expect("from_reader");
    assert_eq!(
        drive(&mut reader),
        vec![
            start_doc(),
            start_para(),
            Event::EndParagraph,
            Event::EndDocument,
        ]
    );
}

#[test]
fn multiple_pics_multiple_events() {
    let rels = doc_rels(&format!(
        r#"<Relationship Id="rId1" Type="{IMAGE_REL_TYPE}" Target="media/one.png"/>
<Relationship Id="rId2" Type="{IMAGE_REL_TYPE}" Target="media/two.png"/>"#
    ));
    let drawing = format!(
        "{}{}",
        inline_pic(r#"<a:blip r:embed="rId1"/>"#),
        inline_pic(r#"<a:blip r:embed="rId2"/>"#),
    );
    let doc = drawing_doc(&drawing);
    let bytes = build_docx(&rels, &doc);
    let mut reader = DocxReader::from_reader(Cursor::new(bytes)).expect("from_reader");
    assert_eq!(
        drive(&mut reader),
        vec![
            start_doc(),
            start_para(),
            image_event(asset_source("zip://word/media/one.png"), None),
            image_event(asset_source("zip://word/media/two.png"), None),
            Event::EndParagraph,
            Event::EndDocument,
        ]
    );
}

const HYPERLINK_REL_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink";

fn hyperlink_drawing_doc(paragraph_inner: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document
  xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
  xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
  xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
  xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"
  xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing">
  <w:body><w:p>{paragraph_inner}</w:p></w:body>
</w:document>"#
    )
}

#[test]
fn hyperlink_wrapping_only_a_drawing_emits_start_and_end_link_around_image() {
    let rels = doc_rels(&format!(
        r#"<Relationship Id="rId1" Type="{HYPERLINK_REL_TYPE}" Target="https://example.com" TargetMode="External"/>
<Relationship Id="rId2" Type="{IMAGE_REL_TYPE}" Target="media/image1.png"/>"#
    ));
    let drawing = inline_pic(r#"<a:blip r:embed="rId2"/>"#);
    let paragraph_inner = format!(
        r#"<w:hyperlink r:id="rId1"><w:r><w:drawing>{drawing}</w:drawing></w:r></w:hyperlink>"#
    );
    let doc = hyperlink_drawing_doc(&paragraph_inner);
    let bytes = build_docx(&rels, &doc);
    let mut reader = DocxReader::from_reader(Cursor::new(bytes)).expect("from_reader");
    assert_eq!(
        drive(&mut reader),
        vec![
            start_doc(),
            start_para(),
            Event::StartLink {
                href: "https://example.com".to_string(),
                id: None,
                title: None,
            },
            image_event(asset_source("zip://word/media/image1.png"), None),
            Event::EndLink,
            Event::EndParagraph,
            Event::EndDocument,
        ]
    );
}

#[test]
fn sibling_drawing_without_docpr_does_not_inherit_alt() {
    let rels = doc_rels(&format!(
        r#"<Relationship Id="rId1" Type="{IMAGE_REL_TYPE}" Target="media/one.png"/>
<Relationship Id="rId2" Type="{IMAGE_REL_TYPE}" Target="media/two.png"/>"#
    ));
    let drawing_with_alt = r#"<wp:inline><wp:docPr id="1" name="img1" descr="first"/><a:graphic><a:graphicData><pic:pic><pic:blipFill><a:blip r:embed="rId1"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:inline>"#;
    let drawing_without_alt = inline_pic(r#"<a:blip r:embed="rId2"/>"#);
    let doc = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document
  xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
  xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
  xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
  xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"
  xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing">
  <w:body>
    <w:p><w:r><w:drawing>{drawing_with_alt}</w:drawing></w:r></w:p>
    <w:p><w:r><w:drawing>{drawing_without_alt}</w:drawing></w:r></w:p>
  </w:body>
</w:document>"#
    );
    let bytes = build_docx(&rels, &doc);
    let mut reader = DocxReader::from_reader(Cursor::new(bytes)).expect("from_reader");
    assert_eq!(
        drive(&mut reader),
        vec![
            start_doc(),
            start_para(),
            image_event(asset_source("zip://word/media/one.png"), Some("first")),
            Event::EndParagraph,
            start_para(),
            image_event(asset_source("zip://word/media/two.png"), None),
            Event::EndParagraph,
            Event::EndDocument,
        ]
    );
}
