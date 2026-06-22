//! Snapshot helpers for corpus-level golden event tests.
//!
//! See TESTING.md §Snapshot Review for the first-run and CI-mode workflow.

use std::fmt;
use std::path::Path;

use sha2::{Digest as _, Sha256};

use docspec_core::{Event, ImageSource};

/// Snapshot of a complete reader run against one fixture file.
///
/// Serialized via [`fmt::Debug`] and compared by `insta::assert_debug_snapshot!`.
#[derive(Clone, PartialEq, Eq)]
pub struct CorpusSnapshot {
    /// All events emitted before the stream ended or errored.
    pub events: Vec<EventSnapshot>,
    /// How the stream terminated.
    pub terminal: Terminal,
}

impl fmt::Debug for CorpusSnapshot {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "CorpusSnapshot {{")?;
        writeln!(f, "  events: [")?;
        for (i, ev) in self.events.iter().enumerate() {
            writeln!(f, "    {i:>4}: {ev:?}")?;
        }
        writeln!(f, "  ],")?;
        writeln!(f, "  terminal: {:?},", self.terminal)?;
        write!(f, "}}")
    }
}

/// A single event captured from the reader stream.
#[non_exhaustive]
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum EventSnapshot {
    /// Any non-image event; holds the `{ev:?}` Debug string of the event.
    Other(String),
    /// An image event with stable metadata and asset descriptor.
    Image {
        /// Stable representation of the non-asset fields.
        metadata: String,
        /// Asset content descriptor.
        asset: AssetDescriptor,
    },
}

/// Stable descriptor for an image asset, including its SHA-256 content hash.
#[derive(Clone, PartialEq, Eq)]
pub struct AssetDescriptor {
    /// Reader-assigned asset identifier.
    pub asset_id: String,
    /// MIME content type, or `"unknown"` if not provided.
    pub content_type: String,
    /// Number of bytes streamed.
    pub bytes: u64,
    /// Lowercase hex SHA-256 of the asset bytes.
    pub sha256: String,
}

impl fmt::Debug for AssetDescriptor {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AssetDescriptor {{ asset_id: {:?}, content_type: {:?}, bytes: {}, sha256: {:?} }}",
            self.asset_id, self.content_type, self.bytes, self.sha256
        )
    }
}

/// How the event stream ended.
#[non_exhaustive]
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Terminal {
    /// Stream ended normally via `Ok(None)`.
    Ok,
    /// Stream ended with an error; holds the `Display` string of the error.
    Err(String),
}

/// Returns `true` if `bytes` begins with the Git LFS pointer prefix.
///
/// LFS pointer files start with `version https://git-lfs.github.com/spec/v1`.
/// When this returns `true`, the file has not been smudged; run `git lfs pull`.
#[inline]
#[must_use]
pub fn is_lfs_pointer(bytes: &[u8]) -> bool {
    bytes.starts_with(b"version https://git-lfs.github.com/spec/v1")
}

/// Reads a fixture file, opens it with `open`, drains the event stream, and
/// returns a [`CorpusSnapshot`].
///
/// # Panics
///
/// - Panics with a path-inclusive message if the file cannot be read.
/// - Panics with a message containing `"Git LFS pointer"` and `"git lfs pull"` if
///   the file is an LFS pointer rather than smudged bytes.
#[inline]
pub fn capture<P, R, E, F>(path: P, open: F) -> CorpusSnapshot
where
    P: AsRef<Path>,
    R: docspec_core::EventSource,
    E: fmt::Display,
    F: FnOnce(Vec<u8>) -> Result<R, E>,
{
    let fixture = path.as_ref();
    let bytes = std::fs::read(fixture)
        .unwrap_or_else(|e| panic!("failed to read fixture {}: {e}", fixture.display()));
    assert!(
        !is_lfs_pointer(&bytes),
        "fixture {} is a Git LFS pointer — run 'git lfs pull' to fetch real bytes",
        fixture.display()
    );
    let mut reader = match open(bytes) {
        Ok(r) => r,
        Err(e) => {
            return CorpusSnapshot {
                events: vec![],
                terminal: Terminal::Err(format!("open error: {e}")),
            };
        }
    };
    let mut events = Vec::new();
    loop {
        match reader.next_event() {
            Ok(Some(ev)) => events.push(event_to_snapshot(&ev)),
            Ok(None) => {
                return CorpusSnapshot {
                    events,
                    terminal: Terminal::Ok,
                };
            }
            Err(e) => {
                return CorpusSnapshot {
                    events,
                    terminal: Terminal::Err(format!("{e}")),
                };
            }
        }
    }
}

fn event_to_snapshot(ev: &Event) -> EventSnapshot {
    match ev {
        Event::Image {
            source,
            alt,
            decorative,
            id,
            title,
        } => {
            let metadata = format!("alt={alt:?} decorative={decorative} id={id:?} title={title:?}");
            let asset = describe_asset(source);
            EventSnapshot::Image { metadata, asset }
        }
        other => EventSnapshot::Other(format!("{other:?}")),
    }
}

fn digest_to_hex(digest: &[u8]) -> String {
    digest
        .iter()
        .flat_map(|byte| {
            let byte = *byte;
            [hex_digit(byte >> 4), hex_digit(byte & 0x0f)]
        })
        .collect()
}

fn hex_digit(nibble: u8) -> char {
    const HEX: [char; 16] = [
        '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
    ];

    HEX.get(usize::from(nibble)).copied().unwrap_or('0')
}

fn describe_asset(source: &ImageSource) -> AssetDescriptor {
    match source {
        ImageSource::Asset(handle) => {
            let mut buf = Vec::new();
            let bytes = handle
                .stream_to(&mut buf)
                .unwrap_or_else(|e| panic!("failed to stream asset {}: {e}", handle.asset_id()));
            let digest = Sha256::new().chain_update(&buf).finalize();
            let sha256 = digest_to_hex(&digest);
            AssetDescriptor {
                asset_id: handle.asset_id().to_owned(),
                content_type: handle
                    .content_type()
                    .map_or_else(|| "unknown".to_owned(), std::borrow::Cow::into_owned),
                bytes,
                sha256,
            }
        }
        ImageSource::Uri { uri } => AssetDescriptor {
            asset_id: format!("uri:{uri}"),
            content_type: "unknown".to_owned(),
            bytes: 0,
            sha256: "n/a".to_owned(),
        },
        // ImageSource is #[non_exhaustive]; handle future variants explicitly.
        _ => panic!("describe_asset: unrecognised ImageSource variant — update snapshot.rs"),
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unused_result_ok
    )]

    use std::io::Cursor;

    use std::sync::Arc;

    use docspec_core::{EventSource, ImageSource};
    use docspec_docx_reader::DocxReader;

    use super::*;
    use crate::{synth_docx, synth_docx_with_image_png};

    struct AlwaysErrReader;
    impl EventSource for AlwaysErrReader {
        fn next_event(&mut self) -> docspec_core::Result<Option<Event>> {
            Err(std::io::Error::other("deliberate test error").into())
        }
    }

    fn make_always_err_reader(_bytes: Vec<u8>) -> Result<AlwaysErrReader, String> {
        Ok(AlwaysErrReader)
    }

    struct MockAssetHandle {
        asset_id: String,
        content_type: Option<String>,
        should_err_on_stream: bool,
    }

    impl docspec_core::AssetHandle for MockAssetHandle {
        fn content_type(&self) -> Option<std::borrow::Cow<'_, str>> {
            self.content_type.as_deref().map(std::borrow::Cow::Borrowed)
        }

        fn stream_to(&self, _writer: &mut dyn std::io::Write) -> std::io::Result<u64> {
            if self.should_err_on_stream {
                Err(std::io::Error::other("mock stream error"))
            } else {
                Ok(0)
            }
        }

        fn asset_id(&self) -> &str {
            &self.asset_id
        }
    }

    impl core::fmt::Debug for MockAssetHandle {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            write!(f, "MockAssetHandle({})", self.asset_id)
        }
    }

    fn tmp_path(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("docspec-test-utils-{}", std::process::id()));
        std::fs::create_dir_all(&dir).ok();
        dir.join(name)
    }

    #[test]
    fn is_lfs_pointer_accepts_canonical_prefix() {
        assert!(is_lfs_pointer(
            b"version https://git-lfs.github.com/spec/v1\noid sha256:abc\nsize 1234\n"
        ));
    }

    #[test]
    fn is_lfs_pointer_rejects_zip_magic() {
        assert!(!is_lfs_pointer(b"\x50\x4b\x03\x04"));
    }

    #[test]
    fn is_lfs_pointer_rejects_empty() {
        assert!(!is_lfs_pointer(b""));
    }

    #[test]
    fn is_lfs_pointer_rejects_version_without_lfs_url() {
        assert!(!is_lfs_pointer(b"version 1.0\nsome data"));
    }

    #[test]
    fn capture_returns_ok_for_clean_docx() {
        let bytes = synth_docx(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#,
            r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body><w:p><w:r><w:t>hello</w:t></w:r></w:p></w:body>
</w:document>"#,
        );
        let path = tmp_path("test5_clean.docx");
        std::fs::write(&path, &bytes).expect("write tmp");
        let snapshot = capture(&path, |b| DocxReader::from_reader(Cursor::new(b)));
        std::fs::remove_file(&path).ok();
        assert_eq!(snapshot.terminal, Terminal::Ok);
        assert!(
            snapshot.events.len() >= 2,
            "expected at least StartDocument + EndDocument events"
        );
    }

    #[test]
    fn capture_returns_err_for_corrupted_input() {
        let path = tmp_path("test6_corrupt.bin");
        std::fs::write(&path, b"not a zip").expect("write tmp");
        let snapshot = capture(&path, |b| DocxReader::from_reader(Cursor::new(b)));
        std::fs::remove_file(&path).ok();
        assert!(
            matches!(&snapshot.terminal, Terminal::Err(msg) if !msg.is_empty()),
            "expected non-empty Terminal::Err, got {:?}",
            snapshot.terminal
        );
    }

    #[test]
    fn capture_panics_on_lfs_pointer_with_actionable_message() {
        let path = tmp_path("test7_lfs_pointer.docx");
        let lfs_pointer =
            b"version https://git-lfs.github.com/spec/v1\noid sha256:abc123\nsize 1234\n";
        std::fs::write(&path, lfs_pointer).expect("write tmp");
        let result = std::panic::catch_unwind(|| capture(&path, make_always_err_reader));
        std::fs::remove_file(&path).ok();
        let err = result.expect_err("capture should panic on LFS pointer");
        let msg = err.downcast_ref::<String>().unwrap().clone();
        assert!(
            msg.contains("Git LFS pointer"),
            "panic message must contain 'Git LFS pointer': {msg}"
        );
        assert!(
            msg.contains("git lfs pull"),
            "panic message must contain 'git lfs pull': {msg}"
        );
    }

    #[test]
    fn describe_asset_produces_stable_sha256_for_png() {
        let bytes = synth_docx_with_image_png();
        let path = tmp_path("test8_image.docx");
        std::fs::write(&path, &bytes).expect("write tmp");
        let snapshot = capture(&path, |b| DocxReader::from_reader(Cursor::new(b)));
        std::fs::remove_file(&path).ok();
        let asset = snapshot
            .events
            .iter()
            .find_map(|ev| {
                if let EventSnapshot::Image { asset, .. } = ev {
                    Some(asset)
                } else {
                    None
                }
            })
            .expect("expected at least one Image event");
        assert_eq!(
            asset.sha256,
            "4c4b6a3be1314ab86138bef4314dde022e600960d8689a2c8f8631802d20dab6"
        );
        assert_eq!(asset.bytes, 8);
        assert_eq!(asset.content_type, "image/png");
    }

    #[test]
    fn terminal_ok_debug_format() {
        assert_eq!(format!("{:?}", Terminal::Ok), "Ok");
    }

    #[test]
    fn terminal_err_debug_format() {
        assert_eq!(
            format!("{:?}", Terminal::Err("boom".to_owned())),
            r#"Err("boom")"#
        );
    }

    #[test]
    fn asset_descriptor_debug_format() {
        let desc = AssetDescriptor {
            asset_id: "zip://word/media/image1.png".to_owned(),
            content_type: "image/png".to_owned(),
            bytes: 8,
            sha256: "4c4b6a3be1314ab86138bef4314dde022e600960d8689a2c8f8631802d20dab6".to_owned(),
        };
        let expected = r#"AssetDescriptor { asset_id: "zip://word/media/image1.png", content_type: "image/png", bytes: 8, sha256: "4c4b6a3be1314ab86138bef4314dde022e600960d8689a2c8f8631802d20dab6" }"#;
        assert_eq!(format!("{desc:?}"), expected);
    }

    #[test]
    fn corpus_snapshot_debug_shows_indexed_events() {
        let snapshot = CorpusSnapshot {
            events: vec![
                EventSnapshot::Other("StartDocument".to_owned()),
                EventSnapshot::Other("EndDocument".to_owned()),
            ],
            terminal: Terminal::Ok,
        };
        let debug_output = format!("{snapshot:?}");
        assert!(
            debug_output.contains("   0: "),
            "expected '   0: ' index prefix: {debug_output}"
        );
        assert!(
            debug_output.contains("   1: "),
            "expected '   1: ' index prefix: {debug_output}"
        );
    }

    #[test]
    fn capture_panics_when_fixture_file_not_found() {
        let path = tmp_path("nonexistent_snapshot_test_file.docx");
        std::fs::remove_file(&path).ok();
        let result = std::panic::catch_unwind(|| capture(&path, make_always_err_reader));
        let err = result.expect_err("capture should panic when file not found");
        let msg = err.downcast_ref::<String>().unwrap();
        assert!(
            msg.contains("failed to read fixture"),
            "expected file-not-found message: {msg}"
        );
    }

    #[test]
    fn capture_returns_err_when_next_event_returns_error() {
        let path = tmp_path("error_reader_test.bin");
        std::fs::write(&path, b"not lfs content").expect("write tmp");
        let snapshot = capture(&path, make_always_err_reader);
        std::fs::remove_file(&path).ok();
        assert!(
            matches!(&snapshot.terminal, Terminal::Err(msg) if msg.contains("deliberate test error")),
            "expected Terminal::Err with test error message, got {:?}",
            snapshot.terminal
        );
    }

    #[test]
    fn describe_asset_uri_variant_produces_correct_descriptor() {
        let source = ImageSource::Uri {
            uri: "https://example.com/img.png".to_owned(),
        };
        let desc = describe_asset(&source);
        assert_eq!(desc.asset_id, "uri:https://example.com/img.png");
        assert_eq!(desc.content_type, "unknown");
        assert_eq!(desc.bytes, 0);
        assert_eq!(desc.sha256, "n/a");
    }

    #[test]
    fn describe_asset_uses_unknown_when_content_type_is_none() {
        let mock = MockAssetHandle {
            asset_id: "test://no-content-type".to_owned(),
            content_type: None,
            should_err_on_stream: false,
        };
        assert_eq!(
            format!("{mock:?}"),
            "MockAssetHandle(test://no-content-type)"
        );
        let handle: Arc<dyn docspec_core::AssetHandle> = Arc::new(mock);
        let source = ImageSource::Asset(handle);
        let desc = describe_asset(&source);
        assert_eq!(desc.content_type, "unknown");
        assert_eq!(desc.bytes, 0);
        assert_eq!(desc.asset_id, "test://no-content-type");
        assert_eq!(
            desc.sha256,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn describe_asset_panics_when_stream_to_fails() {
        let handle: Arc<dyn docspec_core::AssetHandle> = Arc::new(MockAssetHandle {
            asset_id: "test://failing-stream".to_owned(),
            content_type: Some("image/png".to_owned()),
            should_err_on_stream: true,
        });
        let source = ImageSource::Asset(handle);
        let result =
            std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| describe_asset(&source)));
        let err = result.unwrap_err();
        let msg = err.downcast_ref::<String>().unwrap();
        assert!(
            msg.contains("failed to stream asset"),
            "panic msg should mention asset: {msg}"
        );
    }
}
