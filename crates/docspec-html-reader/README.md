# docspec-html-reader

**HTML to DocSpec event stream reader**

Raw HTML arrives; a clean event stream leaves. `docspec-html-reader` parses HTML5 source and emits DocSpec events as it goes, touching only what it understands and dropping the rest without complaint. Streaming, like the rest of DocSpec. (See the [Manifesto](https://github.com/docspec/docspec/blob/main/MANIFESTO.md) for why.)

## Add it

```toml
[dependencies]
docspec-html-reader = "1"
```

## Read some HTML

`HtmlReader` is an `EventSource`: call `next_event()` and events arrive one at a time. From a string:

```rust
use docspec_html_reader::{HtmlReader, EventSource};

let mut reader = HtmlReader::from_str("<p>Hello world</p>");
while let Some(event) = reader.next_event()? {
    println!("{event:?}");
}
# Ok::<(), Box<dyn std::error::Error>>(())
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
# Ok::<(), Box<dyn std::error::Error>>(())
```

In a real pipeline, connect it to any `EventSink` with `docspec_core::pipe(reader, writer)`.

## What it handles today

We emit exactly five event kinds: `StartDocument`, `StartParagraph`, `Text`, `EndParagraph`, `EndDocument`. That means `<p>` elements and the text inside them. Text content inside inline elements like `<strong>` or `<em>` is preserved as `Text` events, but the formatting structure is dropped. Everything else — headings, lists, tables, images, every other HTML element — is silently ignored. No half-formed events, no silent guesses.

Memory stays constant regardless of document size. Both `from_str` and `from_reader` stream through `html5gum::IoReader`'s 16 KB sliding-window buffer; the document never needs to fit in memory.

## Related

- [Architecture](https://github.com/docspec/docspec/blob/main/ARCHITECTURE.md) — the streaming pipeline and event model
- [`docspec-html-reader` on docs.rs](https://docs.rs/docspec-html-reader)
