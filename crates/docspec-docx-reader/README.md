# docspec-docx-reader

Streaming DOCX to DocSpec event stream reader.

See the [main DocSpec repository](https://github.com/docspec/docspec) for documentation,
architecture, and the event protocol.

## Supported

- Paragraphs (`<w:p>`) and direct text (`<w:t>` inside `<w:r>`)
- Emits exactly: `StartDocument`, `StartParagraph`, `Text`, `EndParagraph`, `EndDocument`
- Compression: `Stored` and `Deflated` only

## Out of Scope (silently dropped)

- Run styling (`<w:rPr>`, bold, italic, underline, etc.)
- Line and page breaks (`<w:br>`)
- Tabs (`<w:tab>`)
- Headings (any `<w:pStyle>` value — every paragraph is `StartParagraph`)
- Tables (`<w:tbl>`, `<w:tr>`, `<w:tc>`)
- Lists
- Hyperlinks (`<w:hyperlink>`)
- Drawings and images (`<w:drawing>`, `<w:pict>`)
- Structured document tags (`<w:sdt>`)
- Comments, footnotes, headers, footers
- Document metadata
- Tracked changes (`<w:ins>`, `<w:del>`, `<w:moveFrom>`, `<w:moveTo>`)

## Streaming Guarantee

`DocxReader` streams `document.xml` event by event using constant memory regardless
of document size. Only `_rels/.rels` (a few hundred bytes) is fully read into memory
to discover the document target path.

## Quick Start

```rust,no_run
use docspec_docx_reader::{DocxReader, EventSource};

let mut reader = DocxReader::from_path("document.docx")?;
while let Some(event) = reader.next_event()? {
    println!("{event:?}");
}
# Ok::<(), docspec_core::Error>(())
```

## See Also

- [MANIFESTO.md](../../MANIFESTO.md) — philosophy and values
- [EVENTS.md](../../EVENTS.md) — event types and well-formedness rules
