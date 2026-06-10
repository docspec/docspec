//! ZIP/OPC package navigation for DOCX archives.

use std::io::{Read, Seek};

use docspec_core::{Error, Result};
use zip::result::ZipError;

use crate::rels;
use crate::rels::HyperlinkMap;
use crate::styles::StyleList;

pub fn open_package<R: Read + Seek + Send + 'static>(
    mut reader: R,
) -> Result<(StyleList, HyperlinkMap, Box<dyn Read + Send>)> {
    let mut archive = zip::ZipArchive::new(&mut reader).map_err(|err| match err {
        ZipError::InvalidArchive(_) | ZipError::UnsupportedArchive(_) => Error::Parse {
            message: "not a valid ZIP archive".to_string(),
            position: None,
        },
        ZipError::Io(source) => Error::Io { source },
        ZipError::FileNotFound
        | ZipError::InvalidPassword
        | ZipError::CompressionMethodNotSupported(_)
        | _ => parse_error(format!("not a valid ZIP archive: {err}")),
    })?;

    let rels_bytes = {
        let mut rels_entry = archive.by_name("_rels/.rels").map_err(|err| {
            if matches!(err, ZipError::FileNotFound) {
                Error::Parse {
                    message: "missing _rels/.rels".to_string(),
                    position: None,
                }
            } else {
                parse_error(format!("malformed ZIP: {err}"))
            }
        })?;
        let mut bytes = Vec::new();
        rels_entry.read_to_end(&mut bytes).map_err(Error::from)?;
        bytes
    };

    let document_path = rels::find_document_target(std::io::Cursor::new(rels_bytes))?;
    let (style_list, hyperlink_map) =
        load_style_list_and_hyperlink_map(&mut archive, &document_path)?;

    let (data_start, compressed_size, method) = {
        let entry = archive
            .by_name(&document_path)
            .map_err(|_err| Error::Parse {
                message: format!("document target not found: {document_path}"),
                position: None,
            })?;
        let data_start = entry
            .data_start()
            .ok_or_else(|| parse_error("document.xml has no data offset".to_string()))?;
        (data_start, entry.compressed_size(), entry.compression())
    };
    drop(archive);

    reader
        .seek(std::io::SeekFrom::Start(data_start))
        .map_err(Error::from)?;

    let limited = reader.take(compressed_size);

    let stream: Box<dyn Read + Send> = if method == zip::CompressionMethod::Stored {
        Box::new(limited)
    } else if method == zip::CompressionMethod::Deflated {
        Box::new(flate2::read::DeflateDecoder::new(limited))
    } else {
        return Err(Error::Parse {
            message: format!("unsupported compression: {method:?}"),
            position: None,
        });
    };

    Ok((style_list, hyperlink_map, stream))
}

fn load_style_list_and_hyperlink_map<R: Read + Seek>(
    archive: &mut zip::ZipArchive<&mut R>,
    document_path: &str,
) -> Result<(StyleList, HyperlinkMap)> {
    let doc_rels_path = rels::derive_part_rels_path(document_path);
    let maybe_doc_rels_bytes = if doc_rels_path == "word/_rels/document.xml.rels" {
        match archive.by_name("word/_rels/document.xml.rels") {
            Ok(mut entry) => {
                let mut bytes = Vec::new();
                entry.read_to_end(&mut bytes).map_err(Error::from)?;
                Some(bytes)
            }
            Err(ZipError::FileNotFound) => None,
            Err(err) => return Err(parse_error(format!("malformed ZIP: {err}"))),
        }
    } else {
        read_optional_entry(archive, &doc_rels_path)?
    };
    let Some(doc_rels_bytes) = maybe_doc_rels_bytes else {
        return Ok((StyleList::default(), HyperlinkMap::default()));
    };

    let hyperlink_map =
        rels::collect_hyperlink_map(std::io::Cursor::new(doc_rels_bytes.as_slice()))?;

    let Some(styles_target) =
        rels::find_styles_target(std::io::Cursor::new(doc_rels_bytes.as_slice()))?
    else {
        return Ok((StyleList::default(), hyperlink_map));
    };

    let styles_path = rels::resolve_relative_target(document_path, &styles_target);
    let styles_bytes = match archive.by_name(&styles_path) {
        Ok(mut entry) => {
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).map_err(Error::from)?;
            bytes
        }
        Err(ZipError::FileNotFound) => return Ok((StyleList::default(), hyperlink_map)),
        Err(err) => return Err(parse_error(format!("malformed ZIP: {err}"))),
    };

    let style_list = StyleList::parse(std::io::Cursor::new(styles_bytes))?;
    Ok((style_list, hyperlink_map))
}

fn read_optional_entry<R: Read + Seek>(
    archive: &mut zip::ZipArchive<&mut R>,
    path: &str,
) -> Result<Option<Vec<u8>>> {
    match archive.by_name(path) {
        Ok(mut entry) => {
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).map_err(Error::from)?;
            Ok(Some(bytes))
        }
        Err(ZipError::FileNotFound) => Ok(None),
        Err(err) => Err(parse_error(format!("malformed ZIP: {err}"))),
    }
}

fn parse_error(message: String) -> Error {
    Error::Parse {
        message,
        position: None,
    }
}

#[cfg(test)]
#[cfg(not(coverage))]
mod tests {
    #![allow(clippy::unwrap_used)]
    use core::fmt::Write as _;
    use std::collections::HashMap;
    use std::io::{Cursor, Read as _, Write as _};
    use zip::ZipWriter;

    use super::{load_style_list_and_hyperlink_map, open_package};
    use crate::styles::StyleList;
    use docspec_core::Error;

    fn synth_empty_zip() -> core::result::Result<Vec<u8>, zip::result::ZipError> {
        let buf = Cursor::new(Vec::new());
        let writer = ZipWriter::new(buf);
        Ok(writer.finish()?.into_inner())
    }

    fn synth_zip(
        entries: &[(&str, &[u8])],
    ) -> core::result::Result<Vec<u8>, zip::result::ZipError> {
        let buf = Cursor::new(Vec::new());
        let mut writer = ZipWriter::new(buf);
        for (name, content) in entries {
            add_stored_entry(&mut writer, name, content);
        }
        Ok(writer.finish()?.into_inner())
    }

    fn add_stored_entry(writer: &mut ZipWriter<Cursor<Vec<u8>>>, name: &str, content: &[u8]) {
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        writer.start_file(name, options).unwrap();
        writer.write_all(content).unwrap();
    }

    fn root_rels_xml(document_target: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="{document_target}"/>
</Relationships>"#
        )
    }

    fn doc_rels_xml(styles_target: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="{styles_target}"/>
</Relationships>"#
        )
    }

    fn hyperlink_doc_rels_xml(entries: &[(&str, &str)]) -> String {
        let mut xml = String::from(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
        );
        for (id, target) in entries {
            write!(
                xml,
                r#"
  <Relationship Id="{id}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="{target}" TargetMode="External"/>"#
            )
            .unwrap();
        }
        xml.push_str("\n</Relationships>");
        xml
    }

    fn doc_rels_with_styles_and_hyperlinks(
        styles_target: &str,
        hyperlink_entries: &[(&str, &str)],
    ) -> String {
        let mut xml = String::from(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
        );
        write!(
            xml,
            r#"
  <Relationship Id="rStyle" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="{styles_target}"/>"#
        )
        .unwrap();
        for (id, target) in hyperlink_entries {
            write!(
                xml,
                r#"
  <Relationship Id="{id}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="{target}" TargetMode="External"/>"#
            )
            .unwrap();
        }
        xml.push_str("\n</Relationships>");
        xml
    }

    fn load_package_data(
        entries: &[(&str, &[u8])],
    ) -> core::result::Result<(StyleList, HashMap<String, String>), zip::result::ZipError> {
        let zip_bytes = synth_zip(entries)?;
        let mut reader = Cursor::new(zip_bytes);
        let mut archive = zip::ZipArchive::new(&mut reader)?;
        load_style_list_and_hyperlink_map(&mut archive, "word/document.xml")
            .map_err(|err| zip::result::ZipError::Io(std::io::Error::other(format!("{err:?}"))))
    }

    fn minimal_styles_xml() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:style w:type="paragraph" w:styleId="Normal">
    <w:name w:val="Normal"/>
  </w:style>
</w:styles>"#
    }

    fn minimal_document_xml() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body><w:p/></w:body>
</w:document>"#
    }

    #[test]
    fn open_package_returns_empty_hyperlink_map_when_no_rels_file() {
        let result = load_package_data(&[("word/document.xml", minimal_document_xml().as_bytes())]);

        match result {
            Ok((style_list, hyperlink_map)) => {
                assert_eq!(style_list, StyleList::default());
                assert_eq!(hyperlink_map, HashMap::new());
            }
            Err(err) => assert_eq!(format!("{err:?}"), "expected empty package data"),
        }
    }

    #[test]
    fn open_package_returns_empty_hyperlink_map_when_rels_has_no_hyperlinks() {
        let doc_rels = doc_rels_xml("styles.xml");
        let result = load_package_data(&[
            ("word/_rels/document.xml.rels", doc_rels.as_bytes()),
            ("word/document.xml", minimal_document_xml().as_bytes()),
            ("word/styles.xml", minimal_styles_xml().as_bytes()),
        ]);

        match result {
            Ok((style_list, hyperlink_map)) => {
                assert!(style_list.get_by_id("Normal").is_some());
                assert_eq!(hyperlink_map, HashMap::new());
            }
            Err(err) => assert_eq!(format!("{err:?}"), "expected empty hyperlink map"),
        }
    }

    #[test]
    fn open_package_returns_hyperlink_map_with_single_entry() {
        let doc_rels = hyperlink_doc_rels_xml(&[("rId5", "https://example.test/")]);
        let result = load_package_data(&[
            ("word/_rels/document.xml.rels", doc_rels.as_bytes()),
            ("word/document.xml", minimal_document_xml().as_bytes()),
        ]);

        let mut expected = HashMap::new();
        expected.insert("rId5".to_string(), "https://example.test/".to_string());
        match result {
            Ok((style_list, hyperlink_map)) => {
                assert_eq!(style_list, StyleList::default());
                assert_eq!(hyperlink_map, expected);
            }
            Err(err) => assert_eq!(format!("{err:?}"), "expected one hyperlink"),
        }
    }

    #[test]
    fn open_package_returns_hyperlink_map_with_styles_and_hyperlinks() {
        let doc_rels = doc_rels_with_styles_and_hyperlinks(
            "styles.xml",
            &[("rId9", "https://docspec.example/hyperlink")],
        );
        let result = load_package_data(&[
            ("word/_rels/document.xml.rels", doc_rels.as_bytes()),
            ("word/document.xml", minimal_document_xml().as_bytes()),
            ("word/styles.xml", minimal_styles_xml().as_bytes()),
        ]);

        let mut expected = HashMap::new();
        expected.insert(
            "rId9".to_string(),
            "https://docspec.example/hyperlink".to_string(),
        );
        match result {
            Ok((style_list, hyperlink_map)) => {
                assert!(style_list.get_by_id("Normal").is_some());
                assert_eq!(hyperlink_map, expected);
            }
            Err(err) => assert_eq!(format!("{err:?}"), "expected styles and hyperlink"),
        }
    }

    #[test]
    fn open_package_returns_hyperlink_map_with_multiple_hyperlinks() {
        let doc_rels = hyperlink_doc_rels_xml(&[
            ("rId2", "https://one.example/"),
            ("rId3", "https://two.example/path"),
            ("rId4", "mailto:team@example.test"),
        ]);
        let result = load_package_data(&[
            ("word/_rels/document.xml.rels", doc_rels.as_bytes()),
            ("word/document.xml", minimal_document_xml().as_bytes()),
        ]);

        let expected = HashMap::from([
            ("rId2".to_string(), "https://one.example/".to_string()),
            ("rId3".to_string(), "https://two.example/path".to_string()),
            ("rId4".to_string(), "mailto:team@example.test".to_string()),
        ]);
        match result {
            Ok((style_list, hyperlink_map)) => {
                assert_eq!(style_list, StyleList::default());
                assert_eq!(hyperlink_map, expected);
            }
            Err(err) => assert_eq!(format!("{err:?}"), "expected multiple hyperlinks"),
        }
    }

    #[test]
    fn open_package_errors_when_rels_missing() {
        let bytes = match synth_empty_zip() {
            Ok(b) => b,
            Err(err) => {
                assert_eq!(format!("{err:?}"), "expected valid ZIP");
                return;
            }
        };

        let result = open_package(Cursor::new(bytes));

        match result {
            Err(Error::Parse { message, position }) => {
                assert_eq!(message, "missing _rels/.rels");
                assert_eq!(position, None);
            }
            Err(other) => assert_eq!(format!("{other:?}"), "expected missing rels parse error"),
            Ok(_) => assert_eq!(
                "opened document stream",
                "expected missing rels parse error"
            ),
        }
    }

    #[test]
    fn open_package_with_styles() {
        let root_rels = root_rels_xml("word/document.xml");
        let doc_rels = doc_rels_xml("styles.xml");
        let bytes = synth_zip(&[
            ("_rels/.rels", root_rels.as_bytes()),
            ("word/_rels/document.xml.rels", doc_rels.as_bytes()),
            ("word/document.xml", minimal_document_xml().as_bytes()),
            ("word/styles.xml", minimal_styles_xml().as_bytes()),
        ]);

        let result = bytes.and_then(|zip_bytes| {
            open_package(Cursor::new(zip_bytes))
                .map_err(|err| zip::result::ZipError::Io(std::io::Error::other(format!("{err:?}"))))
        });

        match result {
            Ok((style_list, _hyperlink_map, mut stream)) => {
                assert!(style_list.get_by_id("Normal").is_some());
                let mut document = String::new();
                let read_result = stream.read_to_string(&mut document);
                assert!(read_result.is_ok());
                assert_eq!(document, minimal_document_xml());
            }
            Err(err) => assert_eq!(format!("{err:?}"), "expected styles and document stream"),
        }
    }

    #[test]
    fn open_package_without_styles() {
        let root_rels = root_rels_xml("word/document.xml");
        let bytes = synth_zip(&[
            ("_rels/.rels", root_rels.as_bytes()),
            ("word/document.xml", minimal_document_xml().as_bytes()),
        ]);

        let result = bytes.and_then(|zip_bytes| {
            open_package(Cursor::new(zip_bytes))
                .map_err(|err| zip::result::ZipError::Io(std::io::Error::other(format!("{err:?}"))))
        });

        match result {
            Ok((style_list, _hyperlink_map, _stream)) => {
                assert_eq!(style_list, StyleList::default());
            }
            Err(err) => assert_eq!(format!("{err:?}"), "expected default StyleList"),
        }
    }

    #[test]
    fn open_package_dangling_styles_target_falls_back() {
        let root_rels = root_rels_xml("word/document.xml");
        let doc_rels = doc_rels_xml("styles.xml");
        let bytes = synth_zip(&[
            ("_rels/.rels", root_rels.as_bytes()),
            ("word/_rels/document.xml.rels", doc_rels.as_bytes()),
            ("word/document.xml", minimal_document_xml().as_bytes()),
        ]);

        let result = bytes.and_then(|zip_bytes| {
            open_package(Cursor::new(zip_bytes))
                .map_err(|err| zip::result::ZipError::Io(std::io::Error::other(format!("{err:?}"))))
        });

        match result {
            Ok((style_list, _hyperlink_map, _stream)) => {
                assert_eq!(style_list, StyleList::default());
            }
            Err(err) => assert_eq!(format!("{err:?}"), "expected default StyleList"),
        }
    }
}
