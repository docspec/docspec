# docspec

**One import, every reader and writer.**

`docspec` is the convenience facade over the DocSpec workspace. It re-exports the core
event types and traits, and — behind feature flags — the format readers and writers you
opt into. Reach for it when you want a single entry point; drop to the individual crates
(`docspec-core`, `docspec-markdown-reader`, …) when you want the smallest possible
dependency footprint. Streaming, like the rest of DocSpec. (See the [Manifesto](https://github.com/docspec/docspec/blob/main/MANIFESTO.md) for why.)

## Add it

Pick the formats you need through features:

```toml
[dependencies]
docspec = { version = "1", features = ["markdown", "blocknote"] }
```

Default features are `markdown`, `blocknote`, and `pandoc-native`; disable them when you
want to opt in explicitly.

## Convert a document

Wire a reader to a writer and let the events flow — here, Markdown into BlockNote JSON:

```rust,no_run
use docspec::readers::MarkdownReader;
use docspec::writers::BlockNoteWriter;
use docspec::{EventSink, EventSource, StackTrackingSink};

let markdown = "# Hello\n\nWorld";
let mut reader = MarkdownReader::from_str(markdown);
let mut buf = Vec::<u8>::new();
let mut writer = StackTrackingSink::new(BlockNoteWriter::new(&mut buf));

while let Some(event) = reader.next_event()? {
    writer.handle_event(event)?;
}
writer.finish()?;
# Ok::<(), docspec_core::Error>(())
```

Opening a file by format instead? `AnyReader::from_path(InputFormat::Docx, "document.docx")`
picks the right reader for you.

## Feature flags

### Readers

| Feature    | Format                                                                                      | Crate                     |
| ---------- | ------------------------------------------------------------------------------------------- | ------------------------- |
| `markdown` | Markdown (CommonMark + GFM tables/strikethrough)                                            | `docspec-markdown-reader` |
| `html`     | HTML (paragraphs only)                                                                       | `docspec-html-reader`     |
| `docx`     | DOCX (paragraphs, headings, tables, lists, hyperlinks, images, run styles, color/highlight) | `docspec-docx-reader`     |

`DocxReader` is dispatched through `AnyReader::from_reader` and `AnyReader::from_path`. See
[`docspec-docx-reader`](https://docs.rs/docspec-docx-reader) for the authoritative list of supported and
out-of-scope OOXML elements.

### Writers

| Feature                | Format                                  | Crate                          |
| ---------------------- | --------------------------------------- | ------------------------------ |
| `blocknote-writer`     | BlockNote JSON                          | `docspec-blocknote-writer`     |
| `oxa-writer`           | oxa.dev JSON                            | `docspec-oxa-writer`           |
| `html-writer`          | HTML (paragraphs only)                  | `docspec-html-writer`          |
| `pandoc-native-writer` | Pandoc native block list                | `docspec-pandoc-native-writer` |
| `markdown-writer`      | Markdown (paragraphs and headings only) | `docspec-markdown-writer`      |

### Primitives

| Feature | Crate          | Use case                           |
| ------- | -------------- | ---------------------------------- |
| `json`  | `docspec-json` | Building custom JSON-based writers |

### Convenience

| Feature         | Enables                                            |
| --------------- | -------------------------------------------------- |
| `blocknote`     | BlockNote (writer only until the reader lands)     |
| `oxa`           | oxa.dev (writer only until the reader lands)       |
| `pandoc-native` | Pandoc native (writer only until the reader lands) |
| `all-readers`   | Every reader feature                               |
| `all-writers`   | Every writer feature                               |
| `all-libs`      | Every primitive feature (currently `json`)         |
| `full`          | Everything                                         |

There is no `markdown` convenience feature for the writer — `markdown` is already the
reader feature, so enable `markdown-writer` explicitly.

## Related

- [Architecture](https://github.com/docspec/docspec/blob/main/ARCHITECTURE.md) — the streaming pipeline and event model
- [`docspec` on docs.rs](https://docs.rs/docspec) — the full API and feature reference
