//! ZIP/OPC package navigation for DOCX archives.

use std::fs::File;
use std::io::{Cursor, Read, Seek};
use std::path::Path;
use std::sync::{Arc, Mutex};

use docspec_core::{Error, Result};
use zip::result::ZipError;
use zip::ZipArchive;

use crate::content_types;
use crate::content_types::ContentTypes;
use crate::rels;
use crate::rels::{HyperlinkMap, ImageMap};
use crate::styles::StyleList;

/// Object-safe alias combining [`Read`], [`Seek`], and [`Send`] for use in trait objects.
pub(crate) trait ReadSeek: Read + Seek + Send {}
impl<T: Read + Seek + Send> ReadSeek for T {}

type PackageContents = (
    StyleList,
    crate::numbering::MinimalNumbering,
    HyperlinkMap,
    ImageMap,
    Arc<ContentTypes>,
    Arc<Mutex<ZipArchive<Box<dyn ReadSeek + 'static>>>>,
    Box<dyn Read + Send>,
);

/// Opens a DOCX ZIP archive, loads `styles.xml` and `numbering.xml` from the package
/// relationships, locates `document.xml`, reads `[Content_Types].xml`, and returns
/// a [`PackageContents`] tuple with all parsed data and a streaming reader for the
/// main document part.
pub fn open_package<R>(reader: R) -> Result<PackageContents>
where
    R: Read + Seek + Send + 'static,
{
    let boxed: Box<dyn ReadSeek + 'static> = Box::new(reader);
    let mut archive = ZipArchive::new(boxed).map_err(map_zip_open_error)?;

    let rels_bytes = read_root_rels_bytes(&mut archive)?;

    let document_path = rels::find_document_target(std::io::Cursor::new(rels_bytes))?;

    let (style_list, numbering, hyperlink_map, image_map, content_types) =
        load_small_package_parts(&mut archive, &document_path)?;

    let mut document_xml_bytes = Vec::new();
    archive
        .by_name(&document_path)
        .map_err(|_err| Error::Parse {
            message: format!("document target not found: {document_path}"),
            position: None,
        })?
        .read_to_end(&mut document_xml_bytes)
        .map_err(Error::from)?;

    let content_types_arc = Arc::new(content_types);
    let archive_arc = Arc::new(Mutex::new(archive));

    Ok((
        style_list,
        numbering,
        hyperlink_map,
        image_map,
        content_types_arc,
        archive_arc,
        Box::new(Cursor::new(document_xml_bytes)),
    ))
}

pub(crate) fn open_package_from_path(path: &Path) -> Result<PackageContents> {
    let asset_file = File::open(path).map_err(Error::from)?;
    let boxed: Box<dyn ReadSeek + 'static> = Box::new(asset_file);
    let mut asset_archive = ZipArchive::new(boxed).map_err(map_zip_open_error)?;

    let rels_bytes = read_root_rels_bytes(&mut asset_archive)?;

    let document_path = rels::find_document_target(std::io::Cursor::new(rels_bytes))?;

    let (style_list, numbering, hyperlink_map, image_map, content_types) =
        load_small_package_parts(&mut asset_archive, &document_path)?;

    let streaming = crate::streaming_archive::StreamingArchive::open(path, &document_path)?;

    let content_types_arc = Arc::new(content_types);
    let archive_arc = Arc::new(Mutex::new(asset_archive));

    Ok((
        style_list,
        numbering,
        hyperlink_map,
        image_map,
        content_types_arc,
        archive_arc,
        Box::new(streaming),
    ))
}

fn map_zip_open_error(err: ZipError) -> Error {
    match err {
        ZipError::InvalidArchive(_) | ZipError::UnsupportedArchive(_) => Error::Parse {
            message: "not a valid ZIP archive".to_string(),
            position: None,
        },
        ZipError::Io(source) => Error::Io { source },
        ZipError::FileNotFound
        | ZipError::InvalidPassword
        | ZipError::CompressionMethodNotSupported(_)
        | _ => parse_error(format!("not a valid ZIP archive: {err}")),
    }
}

fn read_root_rels_bytes<R>(archive: &mut ZipArchive<R>) -> Result<Vec<u8>>
where
    R: Read + Seek,
{
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
    Ok(bytes)
}

fn load_small_package_parts<R>(
    archive: &mut ZipArchive<R>,
    document_path: &str,
) -> Result<(
    StyleList,
    crate::numbering::MinimalNumbering,
    HyperlinkMap,
    ImageMap,
    ContentTypes,
)>
where
    R: Read + Seek,
{
    let (style_list, hyperlink_map, image_map) =
        load_style_list_and_hyperlink_map(archive, document_path)?;
    let numbering = load_numbering(archive, document_path)?;
    let content_types = match read_optional_entry(archive, "[Content_Types].xml")? {
        Some(bytes) => content_types::parse(&bytes)?,
        None => ContentTypes::default(),
    };
    Ok((
        style_list,
        numbering,
        hyperlink_map,
        image_map,
        content_types,
    ))
}

fn load_style_list_and_hyperlink_map<R>(
    archive: &mut ZipArchive<R>,
    document_path: &str,
) -> Result<(StyleList, HyperlinkMap, ImageMap)>
where
    R: Read + Seek,
{
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
        return Ok((
            StyleList::default(),
            HyperlinkMap::default(),
            ImageMap::default(),
        ));
    };

    let hyperlink_map =
        rels::collect_hyperlink_map(std::io::Cursor::new(doc_rels_bytes.as_slice()))?;
    let image_map = rels::collect_image_map(doc_rels_bytes.as_slice(), document_path)?;

    let Some(styles_target) =
        rels::find_styles_target(std::io::Cursor::new(doc_rels_bytes.as_slice()))?
    else {
        return Ok((StyleList::default(), hyperlink_map, image_map));
    };

    let styles_path = rels::resolve_relative_target(document_path, &styles_target);
    let styles_bytes = match archive.by_name(&styles_path) {
        Ok(mut entry) => {
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).map_err(Error::from)?;
            bytes
        }
        Err(ZipError::FileNotFound) => return Ok((StyleList::default(), hyperlink_map, image_map)),
        Err(err) => return Err(parse_error(format!("malformed ZIP: {err}"))),
    };

    let style_list = StyleList::parse(std::io::Cursor::new(styles_bytes))?;
    Ok((style_list, hyperlink_map, image_map))
}

fn read_optional_entry<R>(archive: &mut ZipArchive<R>, path: &str) -> Result<Option<Vec<u8>>>
where
    R: Read + Seek,
{
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

fn load_numbering<R>(
    archive: &mut ZipArchive<R>,
    document_path: &str,
) -> Result<crate::numbering::MinimalNumbering>
where
    R: Read + Seek,
{
    let doc_rels_path = rels::derive_part_rels_path(document_path);
    let doc_rels_bytes = match archive.by_name(&doc_rels_path) {
        Ok(mut entry) => {
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).map_err(Error::from)?;
            bytes
        }
        Err(ZipError::FileNotFound) => return Ok(crate::numbering::MinimalNumbering::new()),
        Err(err) => return Err(parse_error(format!("malformed ZIP: {err}"))),
    };

    let Some(numbering_target) = rels::find_numbering_target(std::io::Cursor::new(doc_rels_bytes))?
    else {
        return Ok(crate::numbering::MinimalNumbering::new());
    };

    let numbering_path = rels::resolve_relative_target(document_path, &numbering_target);
    let numbering_bytes = match archive.by_name(&numbering_path) {
        Ok(mut entry) => {
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).map_err(Error::from)?;
            bytes
        }
        Err(ZipError::FileNotFound) => return Ok(crate::numbering::MinimalNumbering::new()),
        Err(err) => return Err(parse_error(format!("malformed ZIP: {err}"))),
    };

    crate::numbering::parse_numbering(std::io::Cursor::new(numbering_bytes))
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
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]
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

    fn image_doc_rels_xml(entries: &[(&str, &str)]) -> String {
        let mut xml = String::from(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
        );
        for (id, target) in entries {
            write!(
                xml,
                r#"
  <Relationship Id="{id}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="{target}"/>"#
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
            .map(|(style_list, hyperlink_map, _image_map)| (style_list, hyperlink_map))
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
            Ok((
                style_list,
                _numbering,
                _hyperlink_map,
                _image_map,
                _content_types,
                _archive,
                mut stream,
            )) => {
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
            Ok((
                style_list,
                _numbering,
                _hyperlink_map,
                _image_map,
                _content_types,
                _archive,
                _stream,
            )) => {
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
            Ok((
                style_list,
                _numbering,
                _hyperlink_map,
                _image_map,
                _content_types,
                _archive,
                _stream,
            )) => {
                assert_eq!(style_list, StyleList::default());
            }
            Err(err) => assert_eq!(format!("{err:?}"), "expected default StyleList"),
        }
    }

    fn doc_rels_with_numbering_xml(styles_target: &str, numbering_target: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="{styles_target}"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="{numbering_target}"/>
</Relationships>"#
        )
    }

    fn doc_rels_with_only_numbering(numbering_target: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="{numbering_target}"/>
</Relationships>"#
        )
    }

    fn minimal_numbering_xml() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:abstractNum w:abstractNumId="1">
    <w:lvl w:ilvl="0">
      <w:numFmt w:val="decimal"/>
    </w:lvl>
  </w:abstractNum>
  <w:num w:numId="1">
    <w:abstractNumId w:val="1"/>
  </w:num>
</w:numbering>"#
    }

    #[test]
    fn open_package_loads_numbering_when_present() {
        let root_rels = root_rels_xml("word/document.xml");
        let doc_rels = doc_rels_with_numbering_xml("styles.xml", "numbering.xml");
        let bytes = synth_zip(&[
            ("_rels/.rels", root_rels.as_bytes()),
            ("word/_rels/document.xml.rels", doc_rels.as_bytes()),
            ("word/document.xml", minimal_document_xml().as_bytes()),
            ("word/styles.xml", minimal_styles_xml().as_bytes()),
            ("word/numbering.xml", minimal_numbering_xml().as_bytes()),
        ]);

        let result = bytes.and_then(|zip_bytes| {
            open_package(Cursor::new(zip_bytes))
                .map_err(|err| zip::result::ZipError::Io(std::io::Error::other(format!("{err:?}"))))
        });

        match result {
            Ok((
                _style_list,
                numbering,
                _hyperlink_map,
                _image_map,
                _content_types,
                _archive,
                _stream,
            )) => {
                assert!(numbering.resolve(1, 0).is_list);
            }
            Err(err) => assert_eq!(format!("{err:?}"), "expected numbering and document stream"),
        }
    }

    #[test]
    fn open_package_returns_empty_numbering_when_absent() {
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
            Ok((
                _style_list,
                numbering,
                _hyperlink_map,
                _image_map,
                _content_types,
                _archive,
                _stream,
            )) => {
                assert!(!numbering.resolve(1, 0).is_list);
            }
            Err(err) => assert_eq!(format!("{err:?}"), "expected empty numbering"),
        }
    }

    #[test]
    fn open_package_returns_empty_numbering_when_rels_missing_link() {
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
            Ok((
                _style_list,
                numbering,
                _hyperlink_map,
                _image_map,
                _content_types,
                _archive,
                _stream,
            )) => {
                assert!(!numbering.resolve(1, 0).is_list);
            }
            Err(err) => assert_eq!(format!("{err:?}"), "expected empty numbering"),
        }
    }

    #[test]
    fn open_package_returns_err_on_malformed_numbering_xml() {
        let root_rels = root_rels_xml("word/document.xml");
        let doc_rels = doc_rels_with_only_numbering("numbering.xml");
        let malformed = b"<w:numbering xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:lvl";
        let bytes = synth_zip(&[
            ("_rels/.rels", root_rels.as_bytes()),
            ("word/_rels/document.xml.rels", doc_rels.as_bytes()),
            ("word/document.xml", minimal_document_xml().as_bytes()),
            ("word/numbering.xml", &malformed[..]),
        ]);

        let result = bytes.and_then(|zip_bytes| {
            open_package(Cursor::new(zip_bytes))
                .map_err(|err| zip::result::ZipError::Io(std::io::Error::other(format!("{err:?}"))))
        });

        match result {
            Err(_) => {}
            Ok(_) => assert_eq!(
                "opened without error",
                "expected parse error for malformed numbering.xml",
            ),
        }
    }

    #[test]
    fn open_package_with_images() {
        let root_rels = root_rels_xml("word/document.xml");
        let doc_rels = image_doc_rels_xml(&[("rId5", "media/image1.png")]);
        let bytes = synth_zip(&[
            ("_rels/.rels", root_rels.as_bytes()),
            ("word/_rels/document.xml.rels", doc_rels.as_bytes()),
            ("word/document.xml", minimal_document_xml().as_bytes()),
            ("word/media/image1.png", b"\x89PNG\r\n\x1a\n"),
        ]);

        let result = bytes.and_then(|zip_bytes| {
            open_package(Cursor::new(zip_bytes))
                .map_err(|err| zip::result::ZipError::Io(std::io::Error::other(format!("{err:?}"))))
        });

        match result {
            Ok((
                _style_list,
                _numbering,
                _hyperlink_map,
                image_map,
                _content_types,
                _archive,
                _stream,
            )) => {
                assert_eq!(image_map.len(), 1);
                let rel = &image_map["rId5"];
                assert_eq!(rel.target, "word/media/image1.png");
                assert!(!rel.is_external);
            }
            Err(err) => assert_eq!(format!("{err:?}"), "expected image map with one entry"),
        }
    }

    #[test]
    fn open_package_without_content_types() {
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
            Ok((
                _style_list,
                _numbering,
                _hyperlink_map,
                _image_map,
                content_types,
                _archive,
                _stream,
            )) => {
                assert_eq!(content_types.lookup("word/document.xml"), None);
                assert_eq!(content_types.lookup("word/media/image1.png"), None);
            }
            Err(err) => assert_eq!(format!("{err:?}"), "expected default content types"),
        }
    }

    #[test]
    fn content_types_lookup_works() {
        let root_rels = root_rels_xml("word/document.xml");
        let content_types_xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="png" ContentType="image/png"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#;
        let bytes = synth_zip(&[
            ("_rels/.rels", root_rels.as_bytes()),
            ("word/document.xml", minimal_document_xml().as_bytes()),
            ("[Content_Types].xml", content_types_xml),
        ]);

        let result = bytes.and_then(|zip_bytes| {
            open_package(Cursor::new(zip_bytes))
                .map_err(|err| zip::result::ZipError::Io(std::io::Error::other(format!("{err:?}"))))
        });

        match result {
            Ok((
                _style_list,
                _numbering,
                _hyperlink_map,
                _image_map,
                content_types,
                _archive,
                _stream,
            )) => {
                assert_eq!(
                    content_types.lookup("word/media/image1.png"),
                    Some("image/png")
                );
                assert_eq!(
                    content_types.lookup("word/document.xml"),
                    Some(
                        "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"
                    )
                );
            }
            Err(err) => assert_eq!(format!("{err:?}"), "expected content types lookup to work"),
        }
    }
}
