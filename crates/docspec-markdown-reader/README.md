# docspec-markdown-reader

Streaming Markdown to DocSpec event stream reader.

See the [main DocSpec repository](https://github.com/docspec/docspec) for documentation,
architecture, and the event protocol.

## Supported Elements

- Headings (h1–h6)
- Paragraphs
- Block quotes
- Code blocks (fenced and indented)
- Bold (`StartTextStyle { kind: Bold }`), italic (`StartTextStyle { kind: Italic }`),
  inline code (`StartTextStyle { kind: Code }`), strikethrough
  (`StartTextStyle { kind: Strikethrough }`)
- Images
- Hard and soft line breaks
- Thematic breaks
- Tables (GFM)
- Bullet and numbered lists (nested)
- Links (inline, reference, autolink)

## Out of Scope (silently dropped)

- Definition lists and footnotes
- HTML blocks and inline HTML
- Math blocks and inline math
- Subscript and superscript formatting

## Memory Model

`MarkdownReader` owns its source `String` for the parser's lifetime. Events still flow
one at a time via `next_event()`, but the full source text stays in memory until the
reader is dropped. This is a constraint of `pulldown-cmark`, which is permanently
borrow-based by design.

For true constant-memory streaming, use `docspec-html-reader`'s `HtmlReader`, which
reads through a 16 KB sliding-window buffer regardless of document size.

## Quick Start

```rust
use docspec_markdown_reader::{MarkdownReader, EventSource};

let mut reader = MarkdownReader::from_str("# Hello\n\nWorld");
while let Some(event) = reader.next_event()? {
    println!("{event:?}");
}
# Ok::<(), docspec_core::Error>(())
```

From a file or any `Read + Seek` source:

```rust,no_run
use std::fs::File;
use docspec_markdown_reader::{MarkdownReader, EventSource};

let file = File::open("document.md")?;
let mut reader = MarkdownReader::from_reader(file)?;
while let Some(event) = reader.next_event()? {
    println!("{event:?}");
}
# Ok::<(), docspec_core::Error>(())
```

## See Also

- [MANIFESTO.md](../../MANIFESTO.md) — philosophy and values
- [EVENTS.md](../../EVENTS.md) — event types and well-formedness rules
