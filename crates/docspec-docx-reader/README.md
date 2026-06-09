# docspec-docx-reader

Streaming DOCX to DocSpec event stream reader.

See the [main DocSpec repository](https://github.com/docspec/docspec) for documentation,
architecture, and the event protocol.

## Supported

- Paragraphs (`<w:p>`) and direct text (`<w:t>` inside `<w:r>`)
- Line breaks (`<w:br>`, including `w:type="page"` and `w:type="column"` — all emit `LineBreak`)
- Tabs (`<w:tab>` — emitted as a `Text` event containing the single character `"\t"`)
- Tables (`<w:tbl>`, `<w:tr>`, `<w:tc>`) — emitted as structural events only; cell merging, header rows, and table styles are not represented
- Emits: `StartDocument`, `StartParagraph`, `Text`, `LineBreak`, `EndParagraph`, `StartTable`, `StartTableRow`, `StartTableCell`, `EndTableCell`, `EndTableRow`, `EndTable`, `EndDocument`
- Compression: `Stored` and `Deflated` only

## Out of Scope (silently dropped)

- Run styling (`<w:rPr>`, bold, italic, underline, etc.)
- Headings (any `<w:pStyle>` value — every paragraph is `StartParagraph`)
- Cell merging (`<w:gridSpan>`, `<w:vMerge>`) — every cell emits with `colspan: None` and `rowspan: None`
- Header rows (`<w:tblHeader>`) — every cell emits as `StartTableCell`, never `StartTableHeader`
- Table, row, and cell properties (`<w:tblPr>`, `<w:trPr>`, `<w:tcPr>`, `<w:tblGrid>`)
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
