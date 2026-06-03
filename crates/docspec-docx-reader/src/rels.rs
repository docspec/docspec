use std::io::{Read, Seek};

use docspec_core::Error;
use quick_xml::events::Event;
use quick_xml::XmlVersion;
use zip::result::ZipError;

pub(in crate::rels) fn find_document_path<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> docspec_core::Result<String> {
    let mut rels = archive.by_name("_rels/.rels").map_err(|err| {
        if matches!(err, ZipError::FileNotFound) {
            Error::Parse {
                message: "missing _rels/.rels".to_string(),
                position: None,
            }
        } else {
            Error::Parse {
                message: format!("malformed ZIP: {err}"),
                position: None,
            }
        }
    })?;

    let mut bytes = Vec::new();
    rels.read_to_end(&mut bytes).map_err(Error::from)?;

    let mut reader = quick_xml::Reader::from_reader(bytes.as_slice());
    let mut buf = Vec::new();
    let mut element_depth: usize = 0;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(element)) => {
                element_depth = element_depth.saturating_add(1);
                if element.local_name().as_ref() == b"Relationship" {
                    if let Some(target) = office_document_target(&reader, &element)? {
                        let document_path = target.strip_prefix('/').unwrap_or(&target).to_string();
                        return validate_document_path(&document_path);
                    }
                }
            }
            Ok(Event::Empty(element)) if element.local_name().as_ref() == b"Relationship" => {
                if let Some(target) = office_document_target(&reader, &element)? {
                    let document_path = target.strip_prefix('/').unwrap_or(&target).to_string();
                    return validate_document_path(&document_path);
                }
            }
            Ok(Event::End(_)) => {
                let Some(next_depth) = element_depth.checked_sub(1) else {
                    return Err(Error::Parse {
                        message: "malformed _rels/.rels: unexpected closing element".to_string(),
                        position: None,
                    });
                };
                element_depth = next_depth;
            }
            Ok(Event::Eof) => {
                if element_depth != 0 {
                    return Err(Error::Parse {
                        message: "malformed _rels/.rels: unclosed element".to_string(),
                        position: None,
                    });
                }
                return Err(Error::Parse {
                    message: "no officeDocument relationship".to_string(),
                    position: None,
                });
            }
            Err(err) => {
                return Err(Error::Parse {
                    message: format!("malformed _rels/.rels: {err}"),
                    position: None,
                });
            }
            Ok(_) => {}
        }
        buf.clear();
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

fn office_document_target(
    reader: &quick_xml::Reader<&[u8]>,
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
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::io::Write as _;
    use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

    fn synth_zip_with_rels(rels_xml: &str) -> Result<Vec<u8>, zip::result::ZipError> {
        let buf = Cursor::new(Vec::new());
        let mut writer = ZipWriter::new(buf);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        writer.start_file("_rels/.rels", options)?;
        writer.write_all(rels_xml.as_bytes())?;
        Ok(writer.finish()?.into_inner())
    }

    fn synth_empty_zip() -> Result<Vec<u8>, zip::result::ZipError> {
        let buf = Cursor::new(Vec::new());
        let writer = ZipWriter::new(buf);
        Ok(writer.finish()?.into_inner())
    }

    fn archive_from_rels(
        rels_xml: &str,
    ) -> Result<zip::ZipArchive<Cursor<Vec<u8>>>, zip::result::ZipError> {
        zip::ZipArchive::new(Cursor::new(synth_zip_with_rels(rels_xml)?))
    }

    fn archive_from_empty_zip() -> Result<zip::ZipArchive<Cursor<Vec<u8>>>, zip::result::ZipError> {
        zip::ZipArchive::new(Cursor::new(synth_empty_zip()?))
    }

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

    fn assert_zip_result<T, E: core::fmt::Debug>(result: Result<T, E>) -> Option<T> {
        match result {
            Ok(value) => Some(value),
            Err(err) => {
                assert_eq!(format!("{err:?}"), "expected valid ZIP");
                None
            }
        }
    }

    #[test]
    fn find_document_path_returns_target_for_simple_rels() {
        let Some(mut archive) =
            assert_zip_result(archive_from_rels(&minimal_rels("word/document.xml")))
        else {
            return;
        };

        let result = find_document_path(&mut archive);

        assert_document_path(result, "word/document.xml");
    }

    #[test]
    fn find_document_path_errors_when_rels_missing() {
        let Some(mut archive) = assert_zip_result(archive_from_empty_zip()) else {
            return;
        };

        let result = find_document_path(&mut archive);

        match result {
            Err(Error::Parse { message, position }) => {
                assert_eq!(message, "missing _rels/.rels");
                assert_eq!(position, None);
            }
            other => assert_eq!(format!("{other:?}"), "expected missing rels parse error"),
        }
    }

    #[test]
    fn find_document_path_errors_when_no_office_document_relationship() {
        let rels_xml = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://example.com/not-office" Target="word/document.xml"/>
</Relationships>"#;
        let Some(mut archive) = assert_zip_result(archive_from_rels(rels_xml)) else {
            return;
        };

        let result = find_document_path(&mut archive);

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
    fn find_document_path_strips_leading_slash() {
        let Some(mut archive) =
            assert_zip_result(archive_from_rels(&minimal_rels("/word/document.xml")))
        else {
            return;
        };

        let result = find_document_path(&mut archive);

        assert_document_path(result, "word/document.xml");
    }

    #[test]
    fn find_document_path_picks_office_document_among_multiple_relationships() {
        let rels_xml = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://example.com/metadata" Target="docProps/core.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
  <Relationship Id="rId3" Type="http://example.com/theme" Target="word/theme/theme1.xml"/>
</Relationships>"#;
        let Some(mut archive) = assert_zip_result(archive_from_rels(rels_xml)) else {
            return;
        };

        let result = find_document_path(&mut archive);

        assert_document_path(result, "word/document.xml");
    }

    #[test]
    fn find_document_path_tolerates_namespaced_relationship_element() {
        let rels_xml = r#"<r:Relationships xmlns:r="http://schemas.openxmlformats.org/package/2006/relationships">
  <r:Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</r:Relationships>"#;
        let Some(mut archive) = assert_zip_result(archive_from_rels(rels_xml)) else {
            return;
        };

        let result = find_document_path(&mut archive);

        assert_document_path(result, "word/document.xml");
    }

    #[test]
    fn find_document_path_errors_on_malformed_rels_xml() {
        let Some(mut archive) = assert_zip_result(archive_from_rels("<broken>")) else {
            return;
        };

        let result = find_document_path(&mut archive);

        match result {
            Err(Error::Parse { message, position }) => {
                let prefix = "malformed _rels/.rels";
                assert_eq!(message.get(..prefix.len()), Some(prefix));
                assert_eq!(position, None);
            }
            other => assert_eq!(format!("{other:?}"), "expected malformed rels parse error"),
        }
    }

    #[test]
    fn find_document_path_rejects_target_with_dotdot_segment() {
        let Some(mut archive) =
            assert_zip_result(archive_from_rels(&minimal_rels("../foo/document.xml")))
        else {
            return;
        };

        let result = find_document_path(&mut archive);

        match result {
            Err(Error::Parse { message, position }) => {
                let prefix = "rels target contains parent reference";
                assert_eq!(message.get(..prefix.len()), Some(prefix));
                assert_eq!(position, None);
            }
            other => assert_eq!(format!("{other:?}"), "expected dotdot parse error"),
        }
    }

    #[test]
    fn find_document_path_handles_target_with_entities() {
        let Some(mut archive) =
            assert_zip_result(archive_from_rels(&minimal_rels("word/doc&amp;ument.xml")))
        else {
            return;
        };

        let result = find_document_path(&mut archive);
        assert_document_path(result, "word/doc&ument.xml");
    }
}
