# docspec-docx-reader

Streaming DOCX to DocSpec event stream reader.

See the [main DocSpec repository](https://github.com/docspec/docspec) for documentation,
architecture, and the event protocol.

## Supported

- Paragraphs (`<w:p>`) and direct text (`<w:t>` inside `<w:r>`)
- Line breaks (`<w:br>`, including `w:type="page"` and `w:type="column"` — all emit `LineBreak`)
- Tabs (`<w:tab>` — emitted as a `Text` event containing the single character `"\t"`)
- Tables (`<w:tbl>`, `<w:tr>`, `<w:tc>`) — emitted as structural events only; cell merging, header rows, and table styles are not represented
- Run properties (`<w:rPr>`): `<w:b>` (bold), `<w:i>` (italic), `<w:u>` (underline — any `w:val` other than `none`), `<w:strike>` (strikethrough), `<w:dstrike>` (double-strike, collapses to strikethrough), `<w:vertAlign>` (`subscript` and `superscript` only; `baseline` resets to neither)
- Paragraph properties (`<w:pPr>`): `<w:jc>` (alignment — `left`/`start` to Left, `right`/`end` to Right, `center` to Center, `both`/`distribute` to Justify)
- Empty `<w:rPr/>` and `<w:pPr/>` are treated as no properties (default style / alignment None)
- A `<w:rPr>` or `<w:pPr>` that appears after content in the same parent is silently ignored (per the OOXML spec, both must be the first child element)
- Emits: `StartDocument`, `StartParagraph`, `Text`, `LineBreak`, `EndParagraph`, `StartTable`, `StartTableRow`, `StartTableCell`, `EndTableCell`, `EndTableRow`, `EndTable`, `EndDocument`
- Compression: `Stored` and `Deflated` only

## Out of Scope (silently dropped)

- Headings (any `<w:pStyle>` value — every paragraph is `StartParagraph`)
- Style references (`<w:rStyle>`, `<w:pStyle>`)
- Run formatting not listed above: `<w:color>`, `<w:sz>`, `<w:szCs>`, `<w:rFonts>`, `<w:shd>`, `<w:highlight>`, `<w:caps>`, `<w:smallCaps>`, `<w:position>`, `<w:spacing>`, `<w:kern>`, `<w:lang>`, `<w:noProof>`
- Revision tracking (`<w:rPrChange>`, `<w:pPrChange>`)
- Advanced paragraph layout beyond alignment: `<w:numPr>`, `<w:ind>`, `<w:tabs>`, `<w:framePr>`, `<w:sectPr>`
- `<w:rPr>` nested inside `<w:pPr>` (paragraph mark / pilcrow run properties)
- BiDi-aware logical alignment (`start`/`end` flipping based on paragraph direction is not tracked)
- Math (`m:rPr`) and DrawingML (`a:rPr`) namespaces
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
