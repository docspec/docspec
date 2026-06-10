//! ZIP/OPC package navigation for DOCX archives.

use std::io::{Read, Seek};

use docspec_core::{Error, Result};
use zip::result::ZipError;

use crate::rels;
use crate::styles::StyleList;

pub fn open_package<R: Read + Seek + Send + 'static>(
    mut reader: R,
) -> Result<(StyleList, Box<dyn Read + Send>)> {
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
    let style_list = load_style_list(&mut archive, &document_path)?;

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

    Ok((style_list, stream))
}

fn load_style_list<R: Read + Seek>(
    archive: &mut zip::ZipArchive<&mut R>,
    document_path: &str,
) -> Result<StyleList> {
    let doc_rels_path = rels::derive_part_rels_path(document_path);
    let doc_rels_bytes = match archive.by_name(&doc_rels_path) {
        Ok(mut entry) => {
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).map_err(Error::from)?;
            bytes
        }
        Err(ZipError::FileNotFound) => return Ok(StyleList::default()),
        Err(err) => return Err(parse_error(format!("malformed ZIP: {err}"))),
    };

    let Some(styles_target) = rels::find_styles_target(std::io::Cursor::new(doc_rels_bytes))?
    else {
        return Ok(StyleList::default());
    };

    let styles_path = rels::resolve_relative_target(document_path, &styles_target);
    let styles_bytes = match archive.by_name(&styles_path) {
        Ok(mut entry) => {
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).map_err(Error::from)?;
            bytes
        }
        Err(ZipError::FileNotFound) => return Ok(StyleList::default()),
        Err(err) => return Err(parse_error(format!("malformed ZIP: {err}"))),
    };

    StyleList::parse(std::io::Cursor::new(styles_bytes))
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
    use std::io::{Cursor, Read as _, Write as _};
    use zip::ZipWriter;

    use super::open_package;
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
            Ok((style_list, mut stream)) => {
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
            Ok((style_list, _stream)) => assert_eq!(style_list, StyleList::default()),
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
            Ok((style_list, _stream)) => assert_eq!(style_list, StyleList::default()),
            Err(err) => assert_eq!(format!("{err:?}"), "expected default StyleList"),
        }
    }
}
