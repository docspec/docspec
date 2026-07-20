# docspec-markdown-writer

**Paragraphs and headings, written as CommonMark.**

Events arrive; Markdown leaves. `docspec-markdown-writer` turns a DocSpec event stream into CommonMark-compliant Markdown, writing directly to any `Write` target as each event passes through. Nothing accumulates. Streaming, like the rest of DocSpec. (See the [Manifesto](https://github.com/docspec/docspec/blob/main/MANIFESTO.md) for why.)

## Add it

```toml
[dependencies]
docspec-markdown-writer = "1"
```

## Write some Markdown

`MarkdownWriter` is an `EventSink`: hand it events and it writes CommonMark as they arrive. We escape text as it passes and close each paragraph with a double newline:

```rust
use docspec_markdown_writer::MarkdownWriter;
use docspec_core::{Event, EventSink};

let mut buf = Vec::<u8>::new();
let mut writer = MarkdownWriter::new(&mut buf);

writer.handle_event(Event::StartDocument { id: None, language: None, metadata: None })?;
writer.handle_event(Event::StartParagraph { alignment: None, id: None })?;
writer.handle_event(Event::Text {
    content: "Hello, world".to_string(),
})?;
writer.handle_event(Event::EndParagraph)?;
writer.handle_event(Event::EndDocument)?;
writer.finish()?;

let output = String::from_utf8(buf)?;
assert_eq!(output, "Hello, world\n\n");
# Ok::<(), Box<dyn std::error::Error>>(())
```

## What it handles today

| DocSpec event | Markdown output |
| --- | --- |
| `StartDocument` / `EndDocument` | (no output — Markdown has no document framing) |
| `StartParagraph` / `EndParagraph` | paragraph text + `\n\n` (empty paragraphs produce zero bytes) |
| `StartHeading { level, id }` / `EndHeading` | ATX heading — prefixes from `#` through `######`, each followed by a space, then the heading text and `\n\n` (level clamped to 1–6; `id` dropped) |
| `Text` | CommonMark-escaped text |

Everything else is silently ignored: text styles (`StartTextStyle` / `EndTextStyle`), line breaks (`LineBreak`, `SoftBreak`), thematic breaks, block quotes, lists, tables, links, images, footnotes, definition lists, captions, and preformatted blocks. Heading IDs are dropped — we never emit `{#id}` syntax. Inline formatting markers (`**bold**`, `*italic*`, `` `code` ``, `~~strike~~`) are not emitted either.

A few more hard edges: ATX headings only, no setext `===` / `---` underlines. LF line endings only — no CRLF, no BOM, no trailing spaces. No GFM extensions: tables, task lists, and footnotes are not supported.

## Related

- [Architecture](https://github.com/docspec/docspec/blob/main/ARCHITECTURE.md) — the streaming pipeline and event model
- [`docspec-markdown-writer` on docs.rs](https://docs.rs/docspec-markdown-writer)
