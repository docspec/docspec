# docspec-docx-reader

Streaming DOCX to DocSpec event stream reader.

See the [main DocSpec repository](https://github.com/docspec/docspec) for documentation,
architecture, and the event protocol.

## Supported

- Paragraphs (`<w:p>`) and direct text (`<w:t>` inside `<w:r>`)
- Line breaks (`<w:br>`, including `w:type="page"` and `w:type="column"` — all emit `LineBreak`)
- Tabs (`<w:tab>` — emitted as a `Text` event containing the single character `"\t"`)
- Tables (`<w:tbl>`, `<w:tr>`, `<w:tc>`) — structural events. Horizontal cell merging via `<w:gridSpan>` is emitted as `colspan`. Header rows via `<w:trPr><w:tblHeader/></w:trPr>` are emitted as `StartTableHeader` (with `scope: Column`) for cells in the contiguous header band at the top of the outermost table. Nested tables emit all cells as `StartTableCell` (header rows in nested tables are not supported). Vertical merging via `<w:vMerge>` is NOT yet supported — rowspan information is lost.
- Run properties (`<w:rPr>`): `<w:b>` (bold), `<w:i>` (italic), `<w:u>` (underline — any `w:val` other than `none`), `<w:strike>` (strikethrough), `<w:dstrike>` (double-strike, collapses to strikethrough), `<w:vertAlign>` (`subscript` and `superscript` only; `baseline` resets to neither). These are emitted as deferred `StartTextStyle { kind, id: None }` / `EndTextStyle` wrapper events around the first run content, not as fields on `Text` events. Empty styled runs emit no style wrapper events; multiple `<w:t>` elements in one styled run share a single wrapper span.
- Run color properties (`<w:rPr>`):
  - `<w:color w:val="HEX">` — foreground (text) color. Emitted as `StartTextStyle { kind: TextColor(Color::Rgb { r, g, b }) }`. `w:val="auto"` and non-hex values are silently dropped. Black `(0,0,0)` is preserved by the reader; whether to treat it as "default color" is writer policy.
  - `<w:highlight w:val="namedColor">` — highlight color using the 17-entry ECMA-376 named palette. Emitted as `StartTextStyle { kind: Mark(Color::Rgb { r, g, b }) }`. `w:val="none"` and unknown names are silently dropped.
  - `<w:shd w:fill="HEX">` — background fill, used as a fallback highlight when `<w:highlight>` is absent. Emitted as `StartTextStyle { kind: Mark(Color::Rgb { r, g, b }) }`. `w:fill="auto"` and a missing `w:fill` attribute are silently dropped.
- Paragraph properties (`<w:pPr>`): `<w:jc>` (alignment — `left`/`start` to Left, `right`/`end` to Right, `center` to Center, `both`/`distribute` to Justify)
- Empty `<w:rPr/>` and `<w:pPr/>` are treated as no properties (default style / alignment None)
- A `<w:rPr>` or `<w:pPr>` that appears after content in the same parent is silently ignored (per the OOXML spec, both must be the first child element)
- Hyperlinks (`<w:hyperlink>`) — link text content is emitted as plain runs. The URL target is dropped.
- Structured Document Tags (`<w:sdt>`) — the content of an SDT is emitted normally. The property containers `<w:sdtPr>` and `<w:sdtEndPr>` are dropped.
- Tracked insertions and moves (`<w:ins>`, `<w:moveTo>`) — the inserted/moved-in content is emitted (accept-changes semantics).
- Emits: `StartDocument`, `StartParagraph`, `StartTextStyle`, `Text`, `EndTextStyle`, `LineBreak`, `EndParagraph`, `StartTable`, `StartTableRow`, `StartTableCell`, `StartTableHeader`, `EndTableHeader`, `EndTableCell`, `EndTableRow`, `EndTable`, `EndDocument`
- Symbol font character normalization for Wingdings, Wingdings 2, Wingdings 3, Webdings, and Symbol fonts — codepoints are mapped to their Unicode equivalents; unmapped codepoints are dropped
- Compression: `Stored` and `Deflated` only

### Color and Highlight Precedence

When both `<w:highlight>` and `<w:shd w:fill>` appear in the same `<w:rPr>`, `<w:highlight>` wins. The `<w:shd>` fill is ignored for that run.

### No-Collapse Rule

Adjacent runs with the same color emit separate `StartTextStyle`/`EndTextStyle` pairs. The reader maintains per-run discipline and does not merge consecutive runs, even when their style properties are identical.

## Out of Scope (subtree silently dropped)

The XML elements listed below are the reader's denylist — their entire subtree is silently dropped during parsing. Any element NOT listed (whether a known structural tag like `<w:p>` or an unknown extension) is processed normally; the reader just continues into its children.

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
- Vertical cell merging (`<w:vMerge>`) — every cell still emits with `rowspan: None`; covered cells emit as ordinary `StartTableCell` events (visual merge is lost)
- Header rows in nested tables — only the outermost table honors `<w:tblHeader>`
- Table-level property exceptions (`<w:tblPrEx>`) — silently ignored (consistent with `<w:tblPr>`)
- Table, row, and cell visual properties (`<w:tblPr>`, `<w:trPr>` visual fields, `<w:tcPr>` visual fields, `<w:tblGrid>`) — silently dropped
- Lists
- Drawings and images (`<w:drawing>`, `<w:pict>`)
- Comments, footnotes, headers, footers
- Document metadata
- Tracked deletions and moves-from (`<w:del>`, `<w:moveFrom>`) — silently dropped (accept-changes semantics). Their text content uses `<w:delText>` which is not part of the reader's text-matching set.
- Structured document tag properties (`<w:sdtPr>`, `<w:sdtEndPr>`) — metadata containers; subtree dropped.
- Hyperlink URL targets — link text is preserved, but the `r:id` attribute pointing to the relationship is dropped.

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
