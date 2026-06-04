# `docspec`

Streaming document conversion. Convenience facade re-exporting the DocSpec readers,
writers, and core event types.

Use this crate when you want a single entry point. For the smallest possible dependency
footprint, depend directly on the individual sub-crates (`docspec-core`,
`docspec-markdown-reader`, etc.) instead.

## Usage

```toml
[dependencies]
docspec = { version = "0.5", features = ["markdown", "blocknote"] }
```

```rust
use docspec::readers::MarkdownReader;
use docspec::writers::BlockNoteWriter;
use docspec::{EventSink, EventSource, StackTrackingSink};

let markdown = "# Hello\n\nWorld";
let mut reader = MarkdownReader::new(markdown);
let mut buf = Vec::<u8>::new();
let mut writer = StackTrackingSink::new(BlockNoteWriter::new(&mut buf));

while let Some(event) = reader.next_event()? {
    writer.handle_event(event)?;
}
writer.finish()?;
```

## Feature Flags

### Readers

| Feature    | Format                                           | Crate                     |
| ---------- | ------------------------------------------------ | ------------------------- |
| `markdown` | Markdown (CommonMark + GFM tables/strikethrough) | `docspec-markdown-reader` |
| `html`     | HTML (paragraphs only)                           | `docspec-html-reader`     |

### Writers

| Feature            | Format                 | Crate                      |
| ------------------ | ---------------------- | -------------------------- |
| `blocknote-writer` | BlockNote JSON         | `docspec-blocknote-writer` |
| `oxa-writer`       | oxa.dev JSON           | `docspec-oxa-writer`       |
| `html-writer`      | HTML (paragraphs only) | `docspec-html-writer`      |

### Primitives

| Feature | Crate          | Use case                           |
| ------- | -------------- | ---------------------------------- |
| `json`  | `docspec-json` | Building custom JSON-based writers |

### Convenience

| Feature       | Enables                                                       |
| ------------- | ------------------------------------------------------------- |
| `blocknote`   | BlockNote in both directions (writer only until reader lands) |
| `oxa`         | oxa.dev in both directions (writer only until reader lands)   |
| `all-readers` | All reader features                                           |
| `all-writers` | All writer features                                           |
| `all-libs`    | All primitive/library features (currently `json`)             |
| `full`        | Everything (`all-readers` + `all-writers` + `all-libs`)       |

No features are enabled by default — opt into what you need.

## Documentation

See the [main DocSpec repository](https://github.com/docspec/docspec) for the full
project documentation, architecture, and event protocol.
