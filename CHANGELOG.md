# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

---

## Unreleased — Cluster 1 streaming fixes

### Fixed

- **BUG-1** (`docspec-blocknote-writer`): Asset encoding no longer buffers the entire asset in a `Vec<u8>`. `write_asset_as_data_uri_keyed` streams base64 bytes directly into the JSON string slot via `base64::write::EncoderWriter`. Verified by `tests/asset_memory.rs`: heap delta < 64 KB across 100× asset scale.
- **BUG-3** (`docspec-cli`): File input now uses `memmap2::Mmap` — O(working-set) memory, not O(file-size). The kernel pages the file on demand; the Rust heap sees only parser overhead. Verified by `tests/cli_memory.rs`: heap delta < 1 MB across 100× input scale. Stdin path is unchanged (documented limitation: pipes cannot be mmap'd).
- **BUG-6** (`docspec-wasm`): New `convert_markdown_to_blocknote_streaming(markdown, on_chunk)` export streams JSON chunks to a JS callback as they are produced. The existing `convert_markdown_to_blocknote` is unchanged. Verified by `tests/streaming.rs`: callback invoked ≥2 times for large input.

### Added

- `docspec-json`: `JsonBackend::write_string_streaming` trait method — streams a JSON string value via a callback, with always-close-string contract on error.
- `docspec-json`: `JsonEmitter::value_streaming` and `KeyedEmitter::value_streaming` — emitter-level streaming API.
- `docspec-cli`: `load_input()` helper in `src/input.rs` — encapsulates mmap/stdin logic, exposed for integration tests.
- `docspec-wasm`: `convert_markdown_to_blocknote_streaming` — streaming WASM export with JS callback.

### Changed

- Workspace `unsafe_code` lint changed from `forbid` to `deny` to allow the single documented `unsafe` block in `docspec-cli`. Library crates remain 100% safe. See `CODING_STANDARDS.md` Section 2.
