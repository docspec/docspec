# docspec-oxa-writer

**DocSpec event stream to oxa.dev JSON writer**

Events arrive; oxa.dev JSON leaves. `docspec-oxa-writer` turns a DocSpec event stream into the [oxa.dev](https://oxa.dev/) JSON format, writing tokens directly to any `Write` target as each event passes through. Nothing accumulates. Streaming, like the rest of DocSpec. (See the [Manifesto](https://github.com/docspec/docspec/blob/main/MANIFESTO.md) for why.)

Built on [`docspec-json`](https://docs.rs/docspec-json), which guarantees the output is structurally valid JSON before a single byte reaches the backend.

## Add it

```toml
[dependencies]
docspec-oxa-writer = "1"
docspec-core = "1"
```

## Emit some oxa.dev JSON

`OxaWriter` is an `EventSink`: hand it events and it writes oxa.dev JSON as they arrive. We open a `Document` object, emit a `Paragraph` for each paragraph, and nest `Text` nodes inside:

```rust
use docspec_oxa_writer::OxaWriter;
use docspec_core::{Event, EventSink};

let mut buf = Vec::<u8>::new();
let mut writer = OxaWriter::new(&mut buf);

writer.handle_event(Event::StartDocument { id: None, language: None, metadata: None })?;
writer.handle_event(Event::StartParagraph { alignment: None, id: None })?;
writer.handle_event(Event::Text {
    content: "Hello, world".to_string(),
})?;
writer.handle_event(Event::EndParagraph)?;
writer.handle_event(Event::EndDocument)?;
writer.finish()?;

let json = String::from_utf8(buf)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

The output for that sequence is:

```json
{"type":"Document","children":[{"type":"Paragraph","children":[{"type":"Text","value":"Hello, world"}]}]}
```

## What it handles today

Paragraphs and the text inside them. `Text` style information is dropped — we emit the content string only. Everything else is silently ignored: headings, block quotes, preformatted blocks, images, tables, thematic breaks, hard and soft line breaks, lists, links, footnotes, definition lists, and captions. No half-formed JSON, no silent guesses.

## Related

- [Architecture](https://github.com/docspec/docspec/blob/main/ARCHITECTURE.md) — the streaming pipeline and event model
- [`docspec-oxa-writer` on docs.rs](https://docs.rs/docspec-oxa-writer)
