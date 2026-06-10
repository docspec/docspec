# docspec-html-reader

Streaming HTML to DocSpec event stream reader.

See the [main DocSpec repository](https://github.com/docspec/docspec) for documentation,
architecture, and the event protocol.

## Supported Elements

- Paragraphs (`<p>`)
- Emits exactly: `StartDocument`, `StartParagraph`, `Text`, `EndParagraph`, `EndDocument`

## Out of Scope (silently dropped)

All other HTML elements are silently ignored. Text content inside inline elements
(e.g., `<strong>`, `<em>`) is preserved as `Text` events, but the formatting
structure is dropped.

## Streaming Guarantee

`HtmlReader` streams its source via `html5gum::IoReader`'s 16 KB sliding-window
buffer. Memory usage is constant regardless of document size — the document need
not fit in memory. Both `from_str` and `from_reader` use this streaming path
internally.

## Quick Start

```rust
use docspec_html_reader::{HtmlReader, EventSource};

let mut reader = HtmlReader::from_str("<p>Hello world</p>");
while let Some(event) = reader.next_event()? {
    println!("{event:?}");
}
# Ok::<(), docspec_core::Error>(())
```

From a file or any `Read + Seek` source:

```rust,no_run
use std::fs::File;
use docspec_html_reader::{HtmlReader, EventSource};

let file = File::open("document.html")?;
let mut reader = HtmlReader::from_reader(file)?;
while let Some(event) = reader.next_event()? {
    println!("{event:?}");
}
# Ok::<(), docspec_core::Error>(())
```

## See Also

- [MANIFESTO.md](../../MANIFESTO.md) — philosophy and values
- [ARCHITECTURE.md](../../ARCHITECTURE.md) — pipeline design, event model decisions, and pointers to the in-code event reference
- [`docspec_core` on docs.rs](https://docs.rs/docspec-core) — every event variant, field, and well-formedness rule
