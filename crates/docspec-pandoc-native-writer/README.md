# docspec-pandoc-native-writer

**Pandoc's native block-list syntax, written as events flow.**

Events arrive; Pandoc native leaves. `docspec-pandoc-native-writer` turns a DocSpec event stream into compact Pandoc native block-list syntax, suitable for Pandoc's `-f native` reader, writing directly to any `Write` target as each event passes through. Nothing accumulates. Streaming, like the rest of DocSpec. (See the [Manifesto](https://github.com/docspec/docspec/blob/main/MANIFESTO.md) for why.)

## Add it

```toml
[dependencies]
docspec-pandoc-native-writer = "1"
```

## Emit Pandoc native

`PandocNativeWriter` is an `EventSink`: hand it events and it writes Pandoc native syntax as they arrive. We open the block list with `[`, emit a `Para` for each paragraph, and close with `]`:

```rust
use docspec_pandoc_native_writer::PandocNativeWriter;
use docspec_core::{Event, EventSink};

let mut buf = Vec::<u8>::new();
let mut writer = PandocNativeWriter::new(&mut buf);

writer.handle_event(Event::StartDocument { id: None, language: None, metadata: None })?;
writer.handle_event(Event::StartParagraph { alignment: None, id: None })?;
writer.handle_event(Event::Text {
    content: "Hello, world".to_string(),
})?;
writer.handle_event(Event::EndParagraph)?;
writer.handle_event(Event::EndDocument)?;
writer.finish()?;

let output = String::from_utf8(buf)?;
// output == "[Para [Str \"Hello, world\"]]"
# Ok::<(), Box<dyn std::error::Error>>(())
```

## What it handles today

| DocSpec event | Pandoc native output |
| --- | --- |
| `StartDocument` / `EndDocument` | `[` / `]` (block list framing) |
| `StartParagraph` / `EndParagraph` | `Para [` / `]` |
| `StartHeading { level, id }` / `EndHeading` | `Header N ("id",[],[]) [` / `]` (level passed through raw; classes and key-value attrs always empty) |
| `Text` | `Str "..."` |
| `ThematicBreak` (id dropped) | `HorizontalRule` |
| `LineBreak` | `LineBreak` |
| `SoftBreak` | `SoftBreak` |
| `StartTextStyle { kind: Bold }` / `EndTextStyle` | `Strong [` / `]` |
| `StartTextStyle { kind: Italic }` / `EndTextStyle` | `Emph [` / `]` |
| `StartTextStyle { kind: Strikethrough }` / `EndTextStyle` | `Strikeout [` / `]` |
| `StartTextStyle { kind: Underline }` / `EndTextStyle` | `Underline [` / `]` |
| `StartTextStyle { kind: Subscript }` / `EndTextStyle` | `Subscript [` / `]` |
| `StartTextStyle { kind: Superscript }` / `EndTextStyle` | `Superscript [` / `]` |
| `StartTextStyle { kind: Code, id }` / `EndTextStyle` | `Code ("id",[],[]) "..."` (Text payload buffered between Start and End) |
| `StartPreformatted { id, syntax }` / `EndPreformatted` | `CodeBlock ("id",["syntax"],[]) "..."` (Text payload buffered; literal newlines preserved) |

The following are silently ignored: block quotes, images, tables, lists, links, footnotes, definition lists, and captions. `StartTextStyle { kind: Mark | TextColor }` are accepted but silently flattened — the text inside is preserved without a wrapper.

Four hard edges: compact output only, no pretty-printing or indentation. No metadata wrapper — we emit block-list form `[Para [...]]`, not `Pandoc (Meta ...) [...]`. No `Space` inlines — adjacent `Text` events produce adjacent `Str` constructors with no auto-inserted `Space`. We target pandoc-types >= 1.23, which uses `Str Text` rather than `Str String`.

## Related

- [Architecture](https://github.com/docspec/docspec/blob/main/ARCHITECTURE.md) — the streaming pipeline and event model
- [`docspec-pandoc-native-writer` on docs.rs](https://docs.rs/docspec-pandoc-native-writer)
