use std::io::Read;

use docspec_core::Error;
use quick_xml::events::Event;
use quick_xml::XmlVersion;

pub fn find_document_target<R: Read>(reader: R) -> docspec_core::Result<String> {
    let mut xml_reader = quick_xml::Reader::from_reader(std::io::BufReader::new(reader));
    let mut buf = Vec::new();
    let mut element_depth: usize = 0;

    loop {
        match xml_reader.read_event_into(&mut buf) {
            Ok(Event::Start(element)) => {
                element_depth = element_depth.saturating_add(1);
                if element.local_name().as_ref() == b"Relationship" {
                    if let Some(target) = office_document_target(&xml_reader, &element)? {
                        let document_path = target.strip_prefix('/').unwrap_or(&target).to_string();
                        return validate_document_path(&document_path);
                    }
                }
            }
            Ok(Event::Empty(element)) if element.local_name().as_ref() == b"Relationship" => {
                if let Some(target) = office_document_target(&xml_reader, &element)? {
                    let document_path = target.strip_prefix('/').unwrap_or(&target).to_string();
                    return validate_document_path(&document_path);
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
                return Err(Error::Parse {
                    message: "no officeDocument relationship".to_string(),
                    position: None,
                });
            }
            Err(_err) => {
                return Err(parse_error("malformed _rels/.rels".to_string()));
            }
            Ok(_) => {}
        }
        buf.clear();
    }
}

fn parse_error(message: String) -> Error {
    Error::Parse {
        message,
        position: None,
    }
}

fn validate_document_path(document_path: &str) -> docspec_core::Result<String> {
    if document_path.split('/').any(|component| component == "..") {
        return Err(Error::Parse {
            message: format!("rels target contains parent reference: {document_path}"),
            position: None,
        });
    }

    Ok(document_path.to_string())
}

fn office_document_target<R: Read>(
    reader: &quick_xml::Reader<R>,
    element: &quick_xml::events::BytesStart<'_>,
) -> docspec_core::Result<Option<String>> {
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
            b"Type" => rel_type = Some(value),
            b"Target" => target = Some(value),
            _ => {}
        }
    }

    Ok(match (rel_type, target) {
        (Some(found_type), Some(found_target)) if found_type.ends_with("/officeDocument") => {
            Some(found_target)
        }
        _ => None,
    })
}

#[cfg(test)]
#[cfg(not(coverage))]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn minimal_rels(target: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="{target}"/>
</Relationships>"#
        )
    }

    fn assert_document_path(result: docspec_core::Result<String>, expected: &str) {
        match result {
            Ok(path) => assert_eq!(path, expected),
            Err(err) => assert_eq!(format!("{err:?}"), "expected document path"),
        }
    }

    #[test]
    fn find_document_target_returns_target_for_simple_rels() {
        let rels_xml = minimal_rels("word/document.xml");

        let result = find_document_target(Cursor::new(rels_xml.as_bytes()));

        assert_document_path(result, "word/document.xml");
    }

    #[test]
    fn find_document_target_errors_when_no_office_document_relationship() {
        let rels_xml = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://example.com/not-office" Target="word/document.xml"/>
</Relationships>"#;

        let result = find_document_target(Cursor::new(rels_xml.as_bytes()));

        match result {
            Err(Error::Parse { message, position }) => {
                assert_eq!(message, "no officeDocument relationship");
                assert_eq!(position, None);
            }
            other => assert_eq!(
                format!("{other:?}"),
                "expected no officeDocument parse error"
            ),
        }
    }

    #[test]
    fn find_document_target_errors_after_balanced_nested_non_matching_rels() {
        let rels_xml = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Group><Relationship Id="rId1" Type="http://example.com/not-office" Target="word/document.xml"></Relationship></Group>
</Relationships>"#;

        let result = find_document_target(Cursor::new(rels_xml.as_bytes()));

        match result {
            Err(Error::Parse { message, position }) => {
                assert_eq!(message, "no officeDocument relationship");
                assert_eq!(position, None);
            }
            other => assert_eq!(
                format!("{other:?}"),
                "expected balanced traversal parse error"
            ),
        }
    }

    #[test]
    fn find_document_target_errors_on_unexpected_closing_element() {
        let result = find_document_target(Cursor::new("</Relationships>".as_bytes()));

        match result {
            Err(Error::Parse { message, position }) => {
                assert_eq!(message, "malformed _rels/.rels");
                assert_eq!(position, None);
            }
            other => assert_eq!(
                format!("{other:?}"),
                "expected unexpected closing element parse error"
            ),
        }
    }

    #[test]
    fn find_document_target_errors_on_rels_xml_parser_error() {
        let result = find_document_target(Cursor::new("<Relationships><".as_bytes()));

        match result {
            Err(Error::Parse { message, position }) => {
                assert_eq!(message, "malformed _rels/.rels");
                assert_eq!(position, None);
            }
            other => assert_eq!(format!("{other:?}"), "expected rels parser error"),
        }
    }

    #[test]
    fn find_document_target_strips_leading_slash() {
        let rels_xml = minimal_rels("/word/document.xml");

        let result = find_document_target(Cursor::new(rels_xml.as_bytes()));

        assert_document_path(result, "word/document.xml");
    }

    #[test]
    fn find_document_target_picks_office_document_among_multiple_relationships() {
        let rels_xml = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://example.com/metadata" Target="docProps/core.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
  <Relationship Id="rId3" Type="http://example.com/theme" Target="word/theme/theme1.xml"/>
</Relationships>"#;

        let result = find_document_target(Cursor::new(rels_xml.as_bytes()));

        assert_document_path(result, "word/document.xml");
    }

    #[test]
    fn find_document_target_accepts_non_empty_relationship_element() {
        let rels_xml = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"></Relationship>
</Relationships>"#;

        let result = find_document_target(Cursor::new(rels_xml.as_bytes()));

        assert_document_path(result, "word/document.xml");
    }

    #[test]
    fn find_document_target_errors_on_malformed_relationship_attribute() {
        let rels_xml = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target=word/document.xml/>
</Relationships>"#;

        let result = find_document_target(Cursor::new(rels_xml.as_bytes()));

        match result {
            Err(Error::Parse { message, position }) => {
                assert_eq!(
                    message,
                    "malformed _rels/.rels: position 120: attribute value must be enclosed in `\"` or `'`"
                );
                assert_eq!(position, None);
            }
            other => assert_eq!(format!("{other:?}"), "expected attribute parse error"),
        }
    }

    #[test]
    fn find_document_target_errors_on_bad_attribute_entity() {
        let rels_xml = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/&bogus;.xml"/>
</Relationships>"#;

        let result = find_document_target(Cursor::new(rels_xml.as_bytes()));

        match result {
            Err(Error::Parse { message, position }) => {
                assert_eq!(
                    message,
                    "malformed _rels/.rels: at 6..11: unrecognized entity `bogus`"
                );
                assert_eq!(position, None);
            }
            other => assert_eq!(format!("{other:?}"), "expected entity parse error"),
        }
    }

    #[test]
    fn find_document_target_tolerates_namespaced_relationship_element() {
        let rels_xml = r#"<r:Relationships xmlns:r="http://schemas.openxmlformats.org/package/2006/relationships">
  <r:Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</r:Relationships>"#;

        let result = find_document_target(Cursor::new(rels_xml.as_bytes()));

        assert_document_path(result, "word/document.xml");
    }

    #[test]
    fn find_document_target_errors_on_malformed_rels_xml() {
        let result = find_document_target(Cursor::new("<broken>".as_bytes()));

        match result {
            Err(Error::Parse { message, position }) => {
                assert_eq!(message, "malformed _rels/.rels");
                assert_eq!(position, None);
            }
            other => assert_eq!(format!("{other:?}"), "expected malformed rels parse error"),
        }
    }

    #[test]
    fn find_document_target_rejects_target_with_dotdot_segment() {
        let rels_xml = minimal_rels("../foo/document.xml");

        let result = find_document_target(Cursor::new(rels_xml.as_bytes()));

        match result {
            Err(Error::Parse { message, position }) => {
                assert_eq!(
                    message,
                    "rels target contains parent reference: ../foo/document.xml"
                );
                assert_eq!(position, None);
            }
            other => assert_eq!(format!("{other:?}"), "expected dotdot parse error"),
        }
    }

    #[test]
    fn find_document_target_handles_target_with_entities() {
        let rels_xml = minimal_rels("word/doc&amp;ument.xml");

        let result = find_document_target(Cursor::new(rels_xml.as_bytes()));
        assert_document_path(result, "word/doc&ument.xml");
    }
}
