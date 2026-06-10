# `docspec-pandoc-native-writer`

Streaming [Pandoc native](https://pandoc.org/MANUAL.html#native-pandoc) (block-list) writer for DocSpec events.

Converts a DocSpec event stream into compact Pandoc native block-list syntax, suitable for being read by Pandoc's `-f native` reader. Implements the `EventSink` trait and emits output directly to any `Write` target as events arrive — no intermediate document representation, constant memory regardless of file size.

## Supported Events

| DocSpec Event | Pandoc native output |
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

## Not Supported

The following DocSpec events are silently ignored:

- Block quotes — `StartBlockQuote` / `EndBlockQuote`
- Images — `Image`
- Tables — `StartTable` / `EndTable` and related events
- List items — `StartOrderedListItem` / `StartUnorderedListItem` and related events
- Inline links — `StartLink` / `EndLink`
- Footnotes — `StartFootnote` / `EndFootnote` / `FootnoteRef`
- Definition lists — `StartDefinitionList` / `StartDefinitionTerm` / `StartDefinitionDetail`
- Captions — `StartCaption` / `EndCaption`
- Text formatting styles — `StartTextStyle { kind: Mark | TextColor }` are accepted but silently flattened (text inside is preserved without a wrapper)

## Usage

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

## Limitations

- **Compact output only**: no pretty-printing, no indentation.
- **No metadata wrapper**: emits block-list form `[Para [...]]`, not `Pandoc (Meta ...) [...]`.
- **No `Space` inlines**: adjacent `Text` events produce adjacent `Str` constructors with no auto-inserted `Space`.
- **Targets pandoc-types >= 1.23**: uses `Str Text` (not `Str String`).

## License

See the [repository LICENSE](../../LICENSE).
