# docspec-docx-reader

Streaming DOCX to DocSpec event stream reader.

See the [main DocSpec repository](https://github.com/docspec/docspec) for documentation,
architecture, and the event protocol.

## Supported

- Paragraphs (`<w:p>`) and direct text (`<w:t>` inside `<w:r>`)
- Line breaks (`<w:br>`, including `w:type="page"` and `w:type="column"` — all emit `LineBreak`)
- Tabs (`<w:tab>` — emitted as a `Text` event containing the single character `"\t"`)
- Tables (`<w:tbl>`, `<w:tr>`, `<w:tc>`) — emitted as structural events only; cell merging, header rows, and table styles are not represented
- Run properties (`<w:rPr>`): `<w:b>` (bold), `<w:i>` (italic), `<w:u>` (underline — any `w:val` other than `none`), `<w:strike>` (strikethrough), `<w:dstrike>` (double-strike, collapses to strikethrough), `<w:vertAlign>` (`subscript` and `superscript` only; `baseline` resets to neither). These are emitted as deferred `StartTextStyle { kind, id: None }` / `EndTextStyle` wrapper events around the first run content, not as fields on `Text` events. Empty styled runs emit no style wrapper events; multiple `<w:t>` elements in one styled run share a single wrapper span.
- Run color properties (`<w:rPr>`):
  - `<w:color w:val="HEX">` — foreground (text) color. Emitted as `StartTextStyle { kind: TextColor(Color::Rgb { r, g, b }) }`. `w:val="auto"` and non-hex values are silently dropped. Black `(0,0,0)` is preserved by the reader; whether to treat it as "default color" is writer policy.
  - `<w:highlight w:val="namedColor">` — highlight color using the 17-entry ECMA-376 named palette. Emitted as `StartTextStyle { kind: Mark(Color::Rgb { r, g, b }) }`. `w:val="none"` and unknown names are silently dropped.
  - `<w:shd w:fill="HEX">` — background fill, used as a fallback highlight when `<w:highlight>` is absent. Emitted as `StartTextStyle { kind: Mark(Color::Rgb { r, g, b }) }`. `w:fill="auto"` and a missing `w:fill` attribute are silently dropped.
- Paragraph properties (`<w:pPr>`): `<w:jc>` (alignment — `left`/`start` to Left, `right`/`end` to Right, `center` to Center, `both`/`distribute` to Justify)
- Empty `<w:rPr/>` and `<w:pPr/>` are treated as no properties (default style / alignment None)
- A `<w:rPr>` or `<w:pPr>` that appears after content in the same parent is silently ignored (per the OOXML spec, both must be the first child element)
- Emits: `StartDocument`, `StartParagraph`, `StartTextStyle`, `Text`, `EndTextStyle`, `LineBreak`, `EndParagraph`, `StartTable`, `StartTableRow`, `StartTableCell`, `EndTableCell`, `EndTableRow`, `EndTable`, `EndDocument`
- Symbol font character normalization for Wingdings, Wingdings 2, Wingdings 3, Webdings, and Symbol fonts — codepoints are mapped to their Unicode equivalents; unmapped codepoints are dropped
- Compression: `Stored` and `Deflated` only

### Color and Highlight Precedence

When both `<w:highlight>` and `<w:shd w:fill>` appear in the same `<w:rPr>`, `<w:highlight>` wins. The `<w:shd>` fill is ignored for that run.

### No-Collapse Rule

Adjacent runs with the same color emit separate `StartTextStyle`/`EndTextStyle` pairs. The reader maintains per-run discipline and does not merge consecutive runs, even when their style properties are identical.

## Out of Scope (silently dropped)

- Headings (any `<w:pStyle>` value — every paragraph is `StartParagraph`)
- Style references (`<w:rStyle>`, `<w:pStyle>`)
- Run formatting not listed above: `<w:sz>`, `<w:szCs>`, `<w:caps>`, `<w:smallCaps>`, `<w:position>`, `<w:spacing>`, `<w:kern>`, `<w:lang>`, `<w:noProof>`
- `<w:rFonts>` (general font tracking is not exposed as events, *except for symbol font resolution (Wingdings, Wingdings 2, Wingdings 3, Webdings, Symbol) which is used internally to normalize codepoints to Unicode*)
- `themeColor` / `themeTint` / `themeShade` attributes on `<w:color>` and `<w:shd>` — silently dropped. The reader does not parse `styles.xml` or `theme1.xml`, so theme-referenced colors cannot be resolved. Future work.
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
