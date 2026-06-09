//! ZIP/OPC package navigation for DOCX archives.

use std::io::{Read, Seek};

use docspec_core::{Error, Result};
use zip::result::ZipError;

use crate::rels;

pub fn open_main_document<R: Read + Seek + Send + 'static>(
    mut reader: R,
) -> Result<Box<dyn Read + Send>> {
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

    Ok(stream)
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
    use std::io::Cursor;
    use zip::ZipWriter;

    use super::open_main_document;
    use docspec_core::Error;

    fn synth_empty_zip() -> core::result::Result<Vec<u8>, zip::result::ZipError> {
        let buf = Cursor::new(Vec::new());
        let writer = ZipWriter::new(buf);
        Ok(writer.finish()?.into_inner())
    }

    #[test]
    fn open_main_document_errors_when_rels_missing() {
        let bytes = match synth_empty_zip() {
            Ok(b) => b,
            Err(err) => {
                assert_eq!(format!("{err:?}"), "expected valid ZIP");
                return;
            }
        };

        let result = open_main_document(Cursor::new(bytes));

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
}
