# docspec-markdown-reader

**Markdown to DocSpec event stream reader**

Markdown arrives as text; a typed event stream leaves. `docspec-markdown-reader` parses CommonMark and GitHub Flavored Markdown and emits DocSpec events as it goes, including raw HTML tags embedded in the source. Streaming, like the rest of DocSpec. (See the [Manifesto](https://github.com/docspec/docspec/blob/main/MANIFESTO.md) for why.)

## Add it

```toml
[dependencies]
docspec-markdown-reader = "1"
```

## Read some Markdown

`MarkdownReader` is an `EventSource`: call `next_event()` and events arrive one at a time. From a string:

```rust
use docspec_markdown_reader::{MarkdownReader, EventSource};

let mut reader = MarkdownReader::from_str("# Hello\n\nWorld");
while let Some(event) = reader.next_event()? {
    println!("{event:?}");
}
# Ok::<(), Box<dyn std::error::Error>>(())
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
# Ok::<(), Box<dyn std::error::Error>>(())
```

## What it handles today

We emit events for a broad slice of CommonMark and GFM:

- Headings (h1–h6)
- Paragraphs
- Block quotes
- Code blocks (fenced and indented)
- Bold (`StartTextStyle { kind: Bold }`), italic (`StartTextStyle { kind: Italic }`), inline code (`StartTextStyle { kind: Code }`), strikethrough (`StartTextStyle { kind: Strikethrough }`)
- Images
- Hard and soft line breaks
- Thematic breaks
- Tables (GFM)
- Bullet and numbered lists, nested
- Links (inline, reference, autolink)

Raw HTML tags embedded in Markdown source are translated into DocSpec events. All attributes on these tags are silently ignored. All other HTML tags are silently dropped.

**Inline formatting**

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

**Self-closing / void**

| Tag(s) | DocSpec event |
|---|---|
| `<br>`, `<br/>`, `<br />` | `Event::LineBreak` |
| `<hr>` | `Event::ThematicBreak` (block context only; ignored in paragraph context) |

**Block headings**

`<h1>` through `<h6>` inside an HTML block emit `StartHeading { level: N }` + content + `EndHeading`. Inline styles inside headings are fully supported.

The following are not supported and are silently dropped: definition lists, footnotes, math blocks, inline math, subscript and superscript formatting (use `<sub>` / `<sup>` raw HTML instead), and all HTML tags not listed above.

Three raw-HTML limitations to know: `<pre><code>...</code></pre>` is not treated as a code block — `<pre>` is dropped and `<code>` becomes an inline style, so use fenced code blocks instead. HTML attributes (`id`, `class`, `style`, `href`, `src`, etc.) are not extracted. Unclosed tags are auto-closed at the end of the containing block.

**A note on memory.** `MarkdownReader` owns its source `String` for the parser's lifetime. Events still flow one at a time via `next_event()`, but the full source text stays in memory until the reader is dropped — a constraint of `pulldown-cmark`, which is permanently borrow-based by design. For true constant-memory streaming, use `docspec-html-reader`'s `HtmlReader`, which reads through a 16 KB sliding-window buffer regardless of document size.

## Related

- [Architecture](https://github.com/docspec/docspec/blob/main/ARCHITECTURE.md) — the streaming pipeline and event model
- [`docspec-markdown-reader` on docs.rs](https://docs.rs/docspec-markdown-reader)
