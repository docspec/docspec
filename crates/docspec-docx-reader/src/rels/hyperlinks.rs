//! Hyperlink relationship collection from DOCX relationship parts.

use super::{parse_error, HyperlinkMap};
use std::collections::HashMap;
use std::io::Read;

use docspec_core::Error;
use quick_xml::{events::Event, XmlVersion};

pub(crate) fn collect_hyperlink_map<R>(reader: R) -> Result<HyperlinkMap, Error>
where
    R: Read,
{
    let mut xml_reader = quick_xml::Reader::from_reader(std::io::BufReader::new(reader));
    let mut buf = Vec::new();
    let mut element_depth: usize = 0;
    let mut hyperlinks = HashMap::new();

    loop {
        match xml_reader.read_event_into(&mut buf) {
            Ok(Event::Start(element)) => {
                element_depth = element_depth.saturating_add(1);
                if element.local_name().as_ref() == b"Relationship" {
                    if let Some((id, target)) = hyperlink_entry(&xml_reader, &element)? {
                        hyperlinks.insert(id, target);
                    }
                }
            }
            Ok(Event::Empty(element)) if element.local_name().as_ref() == b"Relationship" => {
                if let Some((id, target)) = hyperlink_entry(&xml_reader, &element)? {
                    hyperlinks.insert(id, target);
                }
            }
            Ok(Event::End(_)) => {
                let Some(next_depth) = element_depth.checked_sub(1) else {
                    return Err(parse_error("malformed _rels/.rels".to_string()));
                };
                element_depth = next_depth;
            }
            Ok(Event::Eof) => {
                if element_depth != 0 {
                    return Err(parse_error("malformed _rels/.rels".to_string()));
                }
                return Ok(hyperlinks);
            }
            Err(_err) => {
                return Err(parse_error("malformed _rels/.rels".to_string()));
            }
            Ok(_) => {}
        }
        buf.clear();
    }
}

fn hyperlink_entry<R>(
    reader: &quick_xml::Reader<R>,
    element: &quick_xml::events::BytesStart<'_>,
) -> Result<Option<(String, String)>, Error>
where
    R: Read,
{
    let mut id = None;
    let mut rel_type = None;
    let mut target = None;

    for attribute_result in element.attributes() {
        let attribute = attribute_result.map_err(|err| Error::Parse {
            message: format!("malformed _rels/.rels: {err}"),
            position: None,
        })?;
        let value = attribute
            .decoded_and_normalized_value(XmlVersion::Implicit1_0, reader.decoder())
            .map_err(|err| Error::Parse {
                message: format!("malformed _rels/.rels: {err}"),
                position: None,
            })?
            .into_owned();

        match attribute.key.local_name().as_ref() {
            b"Id" => id = Some(value),
            b"Type" => rel_type = Some(value),
            b"Target" => target = Some(value),
            _ => {}
        }
    }

    Ok(match (id, rel_type, target) {
        (Some(found_id), Some(found_type), Some(found_target))
            if found_type.ends_with("/hyperlink") =>
        {
            Some((found_id, found_target))
        }
        _ => None,
    })
}

#[cfg(all(test, coverage))]
mod coverage_tests {
    use super::*;
    use std::io::Cursor;

    fn assert_parse_error(result: docspec_core::Result<HyperlinkMap>, expected_message: &str) {
        assert_eq!(
            format!("{result:?}"),
            format!("Err(Parse {{ message: {expected_message:?}, position: None }})")
        );
    }

    #[test]
    fn collect_hyperlink_map_accepts_non_empty_relationship_element() {
        let rels_xml = r#"<Relationships>
  <Relationship Id="rId1" Type="http://example.com/hyperlink" Target="https://example.com"></Relationship>
</Relationships>"#;

        let result = collect_hyperlink_map(Cursor::new(rels_xml.as_bytes()));

        let expected = HashMap::from([("rId1".to_string(), "https://example.com".to_string())]);
        assert_eq!(result.as_ref().ok(), Some(&expected));
    }

    #[test]
    fn collect_hyperlink_map_rejects_unexpected_closing_element() {
        let result = collect_hyperlink_map(Cursor::new("</Relationships>".as_bytes()));

        assert_parse_error(result, "malformed _rels/.rels");
    }

    #[test]
    fn collect_hyperlink_map_rejects_unclosed_relationships_element() {
        let result = collect_hyperlink_map(Cursor::new("<Relationships>".as_bytes()));

        assert_parse_error(result, "malformed _rels/.rels");
    }

    #[test]
    fn collect_hyperlink_map_rejects_xml_parser_error() {
        let result = collect_hyperlink_map(Cursor::new("<Relationships><".as_bytes()));

        assert_parse_error(result, "malformed _rels/.rels");
    }

    #[test]
    fn collect_hyperlink_map_rejects_malformed_relationship_attribute() {
        let rels_xml = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target=word/document.xml/>
</Relationships>"#;

        let result = collect_hyperlink_map(Cursor::new(rels_xml.as_bytes()));

        assert_parse_error(
            result,
            "malformed _rels/.rels: position 120: attribute value must be enclosed in `\"` or `'`",
        );
    }

    #[test]
    fn collect_hyperlink_map_rejects_bad_attribute_entity() {
        let rels_xml = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/&bogus;.xml"/>
</Relationships>"#;

        let result = collect_hyperlink_map(Cursor::new(rels_xml.as_bytes()));

        assert_parse_error(
            result,
            "malformed _rels/.rels: at 6..11: unrecognized entity `bogus`",
        );
    }
}

#[cfg(test)]
#[cfg(not(coverage))]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn assert_hyperlink_map(result: docspec_core::Result<HyperlinkMap>, expected: &[(&str, &str)]) {
        let expected_map = expected
            .iter()
            .map(|(id, target)| ((*id).to_string(), (*target).to_string()))
            .collect::<HyperlinkMap>();

        match result {
            Ok(map) => assert_eq!(map, expected_map),
            Err(err) => assert_eq!(format!("{err:?}"), "expected hyperlink map"),
        }
    }

    #[test]
    fn collect_hyperlink_map_returns_empty_for_empty_relationships() {
        let rels_xml = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"/>"#;

        let result = collect_hyperlink_map(Cursor::new(rels_xml.as_bytes()));

        assert_hyperlink_map(result, &[]);
    }

    #[test]
    fn collect_hyperlink_map_finds_strict_uri_hyperlink() {
        let rels_xml = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://purl.oclc.org/ooxml/officeDocument/relationships/hyperlink" Target="https://example.com"/>
</Relationships>"#;

        let result = collect_hyperlink_map(Cursor::new(rels_xml.as_bytes()));

        assert_hyperlink_map(result, &[("rId1", "https://example.com")]);
    }

    #[test]
    fn collect_hyperlink_map_finds_transitional_uri_hyperlink() {
        let rels_xml = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com"/>
</Relationships>"#;

        let result = collect_hyperlink_map(Cursor::new(rels_xml.as_bytes()));

        assert_hyperlink_map(result, &[("rId1", "https://example.com")]);
    }

    #[test]
    fn collect_hyperlink_map_skips_non_hyperlink_relationships() {
        let rels_xml = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="word/styles.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png"/>
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#;

        let result = collect_hyperlink_map(Cursor::new(rels_xml.as_bytes()));

        assert_hyperlink_map(result, &[]);
    }

    #[test]
    fn collect_hyperlink_map_collects_multiple_hyperlinks() {
        let rels_xml = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com/one"/>
  <Relationship Id="rId2" Type="http://purl.oclc.org/ooxml/officeDocument/relationships/hyperlink" Target="https://example.com/two"/>
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com/three"/>
</Relationships>"#;

        let result = collect_hyperlink_map(Cursor::new(rels_xml.as_bytes()));

        assert_hyperlink_map(
            result,
            &[
                ("rId1", "https://example.com/one"),
                ("rId2", "https://example.com/two"),
                ("rId3", "https://example.com/three"),
            ],
        );
    }

    #[test]
    fn collect_hyperlink_map_preserves_id_to_target_mapping() {
        let rels_xml = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId7" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://rust-lang.org"/>
</Relationships>"#;

        let result = collect_hyperlink_map(Cursor::new(rels_xml.as_bytes()));

        assert_hyperlink_map(result, &[("rId7", "https://rust-lang.org")]);
    }

    #[test]
    fn collect_hyperlink_map_handles_internal_target_mode() {
        let rels_xml = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="other.docx" TargetMode="Internal"/>
</Relationships>"#;

        let result = collect_hyperlink_map(Cursor::new(rels_xml.as_bytes()));

        assert_hyperlink_map(result, &[("rId1", "other.docx")]);
    }

    #[test]
    fn collect_hyperlink_map_handles_external_target_mode() {
        let rels_xml = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com" TargetMode="External"/>
</Relationships>"#;

        let result = collect_hyperlink_map(Cursor::new(rels_xml.as_bytes()));

        assert_hyperlink_map(result, &[("rId1", "https://example.com")]);
    }

    #[test]
    fn collect_hyperlink_map_handles_missing_target_mode() {
        let rels_xml = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com"/>
</Relationships>"#;

        let result = collect_hyperlink_map(Cursor::new(rels_xml.as_bytes()));

        assert_hyperlink_map(result, &[("rId1", "https://example.com")]);
    }

    #[test]
    fn collect_hyperlink_map_returns_err_on_malformed_xml() {
        let result = collect_hyperlink_map(Cursor::new("<broken>".as_bytes()));

        match result {
            Err(Error::Parse { message, position }) => {
                assert_eq!(message, "malformed _rels/.rels");
                assert_eq!(position, None);
            }
            other => assert_eq!(format!("{other:?}"), "expected malformed rels parse error"),
        }
    }
}
