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

## Supported Raw HTML Tags

Raw HTML tags embedded in markdown source are translated into DocSpec events. All
attributes on these tags are silently ignored. All other HTML tags are silently dropped.

### Inline formatting

| Tag(s) | DocSpec event |
|---|---|
| `<b>`, `<strong>` | `StartTextStyle { kind: Bold }` / `EndTextStyle` |
| `<i>`, `<em>` | `StartTextStyle { kind: Italic }` / `EndTextStyle` |
| `<u>` | `StartTextStyle { kind: Underline }` / `EndTextStyle` |
| `<s>`, `<strike>`, `<del>` | `StartTextStyle { kind: Strikethrough }` / `EndTextStyle` |
| `<code>` | `StartTextStyle { kind: Code }` / `EndTextStyle` |
| `<sub>` | `StartTextStyle { kind: Subscript }` / `EndTextStyle` |
| `<sup>` | `StartTextStyle { kind: Superscript }` / `EndTextStyle` |
| `<mark>` | `StartTextStyle { kind: Mark }` with constant yellow `#FFFF00` |

### Self-closing / void

| Tag(s) | DocSpec event |
|---|---|
| `<br>`, `<br/>`, `<br />` | `Event::LineBreak` |
| `<hr>` | `Event::ThematicBreak` (block context only; ignored in paragraph context) |

### Block headings

`<h1>` through `<h6>` inside an HTML block emit `StartHeading { level: N }` + content +
`EndHeading`. Inline styles inside headings are fully supported.

### Known limitations

- `<pre><code>...</code></pre>` is NOT treated as a code block. The `<pre>` is dropped and
  `<code>` becomes an inline style. Use markdown fenced code blocks instead.
- HTML attributes (id, class, style, href, src, etc.) are NOT extracted.
- Unclosed tags are auto-closed at the end of the containing block.

## Out of Scope (silently dropped)

- Definition lists and footnotes
- Math blocks and inline math
- Subscript and superscript formatting (use `<sub>` / `<sup>` raw HTML instead)
- All HTML tags not listed in "Supported Raw HTML Tags" above

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
- [ARCHITECTURE.md](../../ARCHITECTURE.md) — pipeline design, event model decisions, and pointers to the in-code event reference
- [`docspec_core` on docs.rs](https://docs.rs/docspec-core) — every event variant, field, and well-formedness rule
