# `docspec-markdown-writer`

Streaming Markdown (CommonMark) writer for DocSpec events — paragraphs and headings only.

Converts a DocSpec event stream into CommonMark-compliant Markdown output. Implements the `EventSink` trait and emits output directly to any `Write` target as events arrive — no intermediate document representation, constant memory regardless of file size.

## Supported Events

| DocSpec Event | Markdown output |
| --- | --- |
| `StartDocument` / `EndDocument` | (no output — Markdown has no document framing) |
| `StartParagraph` / `EndParagraph` | paragraph text + `\n\n` (empty paragraphs produce zero bytes) |
| `StartHeading { level, id }` / `EndHeading` | ATX heading `# ` ... `###### ` + `\n\n` (level clamped to 1–6; `id` dropped) |
| `Text` | CommonMark-escaped text |

## Not Supported

The following DocSpec events are silently ignored:

- Text styles — `StartTextStyle` / `EndTextStyle`
- Line breaks — `LineBreak`, `SoftBreak`
- Thematic breaks — `ThematicBreak`
- Block quotes, lists, tables, links, images, footnotes, definition lists, captions, preformatted blocks
- Heading IDs (`{#id}` syntax not emitted)
- Inline formatting markers (`**bold**`, `*italic*`, `` `code` ``, `~~strike~~`)

## Usage

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

## Limitations

- **ATX headings only**: no setext `===` / `---` underlines.
- **No inline style markers**: bold, italic, code, and strikethrough are not emitted.
- **No GFM extensions**: tables, task lists, and footnotes are not supported.
- **LF line endings only**: no CRLF, no BOM, no trailing spaces.
- **Heading IDs dropped silently**: `{#id}` syntax is never emitted.

## License

See the [repository LICENSE](../../LICENSE).
