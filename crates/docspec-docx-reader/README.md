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
- Heading-style paragraphs (`<w:pStyle>` resolving to `Title` → level 1, or to any style name matching `heading N` for N ≥ 1) emit `StartHeading { level }` / `EndHeading` instead of `StartParagraph` / `EndParagraph`. Style inheritance via `basedOn` is followed when resolving the style name.
- Block-quote-style paragraphs (`<w:pStyle>` resolving to `Quote`, `Block Text`, `Block Quote`, `Block Quotation`, or `Intense Quote`) emit `StartBlockQuote` / `EndBlockQuote` instead of `StartParagraph` / `EndParagraph`.
- Code-styled paragraphs (paragraph styles whose name resolves to `Source Code`, `SourceCode`, or `Codeblock`) emit `StartPreformatted` / `EndPreformatted`. Consecutive code-styled paragraphs are consolidated into one preformatted block with `LineBreak` events between paragraph boundaries. The character style `Verbatim Char` is recognized as inline code at the run level only and never promotes a paragraph to preformatted.
- Lists (`<w:p>` with `<w:numPr>`): ordered (Decimal/LowerAlpha/UpperAlpha/LowerRoman/UpperRoman) and unordered (Disc) — emitted as `Start*ListItem`/`End*ListItem` with `id` (numId stringified), `level` (ilvl), `start` (`Some(1)` on first item per numId), and `style_type`. Per-level classification: same numId can mix ordered and unordered levels. Continuation paragraphs — paragraphs without `<w:numPr>` between list items — attach to the preceding open list item as additional `StartParagraph`/`EndParagraph` content inside the still-open `Start*ListItem`. The list closes only at a heading, block quote, preformatted block, table boundary, table-cell boundary, or end of document.
- Empty `<w:rPr/>` and `<w:pPr/>` are treated as no properties (default style / alignment None)
- A `<w:rPr>` or `<w:pPr>` that appears after content in the same parent is silently ignored (per the OOXML spec, both must be the first child element)
- Hyperlinks (`<w:hyperlink>`): resolved via `word/_rels/document.xml.rels` and emitted as `StartLink`/`EndLink` events around inline content. Supports external URL targets (both Strict and Transitional OOXML relationship Type URIs), anchor-only links (`w:anchor` without `r:id` emits `#fragment`), and tooltips (`w:tooltip` → `StartLink.title`, XML-decoded). When the relationship cannot be resolved, the link wrapper is dropped and content passes through as plain runs.
- Structured Document Tags (`<w:sdt>`) — the content of an SDT is emitted normally. The property containers `<w:sdtPr>` and `<w:sdtEndPr>` are dropped.
- Tracked insertions and moves (`<w:ins>`, `<w:moveTo>`) — the inserted/moved-in content is emitted (accept-changes semantics).
- DrawingML images (`<w:drawing>`) — emitted as `Event::Image`. See [Image Support](#image-support) below.
- VML images (`<w:pict>` containing `<v:imagedata>`) — emitted as `Event::Image`. Alt text precedence: `o:title` attribute on `<v:imagedata>` (non-empty) falls back to `alt` attribute on parent `<v:shape>`. Multiple `<v:imagedata>` in one `<w:pict>` emit multiple `Image` events in document order. `<v:group>` wrappers are transparent.
- Emits: `StartDocument`, `StartHeading`, `EndHeading`, `StartBlockQuote`, `EndBlockQuote`, `StartParagraph`, `StartPreformatted`, `StartTextStyle`, `Text`, `EndTextStyle`, `LineBreak`, `EndParagraph`, `EndPreformatted`, `StartTable`, `StartTableRow`, `StartTableCell`, `StartTableHeader`, `EndTableHeader`, `EndTableCell`, `EndTableRow`, `EndTable`, `StartLink`, `EndLink`, `StartOrderedListItem`, `EndOrderedListItem`, `StartUnorderedListItem`, `EndUnorderedListItem`, `Image`, `EndDocument`
- Symbol font character normalization for Wingdings, Wingdings 2, Wingdings 3, Webdings, and Symbol fonts — codepoints are mapped to their Unicode equivalents; unmapped codepoints are dropped
- Compression: `Stored` and `Deflated` only
- **Suppressing empty headings, block quotes, and paragraphs**: the reader emits these by design (see `empty_paragraph_still_emits_start_end` and sibling tests in `tests/reader.rs`). To hide empty `StartHeading`/`EndHeading`, `StartBlockQuote`/`EndBlockQuote`, and `StartParagraph`/`EndParagraph` pairs in your pipeline, wrap the reader in [`docspec_core::SkipEmptyBlocks`](https://docs.rs/docspec-core/latest/docspec_core/struct.SkipEmptyBlocks.html). The `docspec` CLI and HTTP server apply this filter by default; library consumers opt in by wrapping their `EventSource`.

### Color and Highlight Precedence

When both `<w:highlight>` and `<w:shd w:fill>` appear in the same `<w:rPr>`, `<w:highlight>` wins. The `<w:shd>` fill is ignored for that run.

### No-Collapse Rule

Adjacent runs with the same color emit separate `StartTextStyle`/`EndTextStyle` pairs. The reader maintains per-run discipline and does not merge consecutive runs, even when their style properties are identical.

## Out of Scope (subtree silently dropped)

The XML elements listed below are the reader's denylist — their entire subtree is silently dropped during parsing. Any element NOT listed (whether a known structural tag like `<w:p>` or an unknown extension) is processed normally; the reader just continues into its children.

- `<w:rStyle>` (run-level style references) — silently dropped
- `<w:pStyle>` values other than the heading, block-quote, and code-style names listed in "Supported" — silently dropped; the paragraph emits as `StartParagraph` with no style metadata
- Run formatting not listed above: `<w:sz>`, `<w:szCs>`, `<w:caps>`, `<w:smallCaps>`, `<w:position>`, `<w:spacing>`, `<w:kern>`, `<w:lang>`, `<w:noProof>`
- `<w:rFonts>` (general font tracking is not exposed as events, *except for symbol font resolution (Wingdings, Wingdings 2, Wingdings 3, Webdings, Symbol) which is used internally to normalize codepoints to Unicode*)
- `themeColor` / `themeTint` / `themeShade` attributes on `<w:color>` and `<w:shd>` — silently dropped. The reader does not parse `styles.xml` or `theme1.xml`, so theme-referenced colors cannot be resolved. Future work.
- Revision tracking (`<w:rPrChange>`, `<w:pPrChange>`)
- Advanced paragraph layout beyond alignment: `<w:ind>`, `<w:tabs>`, `<w:framePr>`, `<w:sectPr>`
- `<w:rPr>` nested inside `<w:pPr>` (paragraph mark / pilcrow run properties)
- BiDi-aware logical alignment (`start`/`end` flipping based on paragraph direction is not tracked)
- Math (`m:rPr`) and DrawingML (`a:rPr`) namespaces
- Vertical cell merging (`<w:vMerge>`) — every cell still emits with `rowspan: None`; covered cells emit as ordinary `StartTableCell` events (visual merge is lost)
- Header rows in nested tables — only the outermost table honors `<w:tblHeader>`
- Table-level property exceptions (`<w:tblPrEx>`) — silently ignored (consistent with `<w:tblPr>`)
- Table, row, and cell visual properties (`<w:tblPr>`, `<w:trPr>` visual fields, `<w:tcPr>` visual fields, `<w:tblGrid>`) — silently dropped
- `<mc:Fallback>` (inside `<mc:AlternateContent>`) — subtree dropped to deduplicate images that appear in both the modern `<w:drawing>` (in `<mc:Choice>`) and the legacy `<w:pict>` (in `<mc:Fallback>`).
- Comments, footnotes, headers, footers
- Document metadata
- Tracked deletions and moves-from (`<w:del>`, `<w:moveFrom>`) — silently dropped (accept-changes semantics). Their text content uses `<w:delText>` which is not part of the reader's text-matching set.
- Structured document tag properties (`<w:sdtPr>`, `<w:sdtEndPr>`) — metadata containers; subtree dropped.
- Field-code hyperlinks (`<w:fldChar>` + `<w:instrText>HYPERLINK ...`): legacy form not currently supported; only the modern `<w:hyperlink>` element is recognized.

### Lists (V1 cuts)

The following list features are intentionally out of scope for V1:
- No `<w:start>` element parsing — `start` is always `Some(1)` on the first item of each list, `None` thereafter
- No `<w:lvlOverride>` resolution — abstractNum's level definitions are authoritative
- No picture bullets (`<w:lvlPicBulletId>`) — picture-bullet levels emit `Disc`
- No style-linked lists (`<w:numStyleLink>`, `<w:styleLink>`) — fall back to `Decimal` defaults
- `<w:multiLevelType>` is ignored — per-level `<w:numFmt>` is authoritative (§17.9.12)
- No per-level marker text (`<w:lvlText>`) — not parsed
- No level-specific font, color, or indent
- No per-level resolution for synthesised phantom levels — when the first authored item appears at `ilvl > 0`, intermediate levels inherit the target item's ordered/unordered classification and use `Decimal`/`Disc` defaults instead of resolving each phantom level's own `numFmt`

## Image Support

`<w:drawing>` elements are parsed and emit `Event::Image`. The `source` field is one of two variants:

- **Embedded image** (`r:embed`): `ImageSource::Asset(handle)` where `handle.asset_id()` returns `"zip://word/media/image1.png"`. The asset ID is the resolved ZIP entry path with a `zip://` scheme prefix. Call `handle.stream_to(&mut buf)` to stream the raw bytes, or `handle.content_type()` to get the MIME type.
- **External image** (`r:link`): `ImageSource::Uri { uri: "<url>" }`. The URL is passed through verbatim from the relationship target.

When both `r:embed` and `r:link` appear on the same blip, `r:embed` wins.

A relationship marked `TargetMode="External"` is honored even when referenced via `r:embed` — the reader emits `ImageSource::Uri` in that case, matching Word and LibreOffice behavior.

If the relationship ID cannot be resolved (missing or malformed rels), the reader emits `ImageSource::Asset(handle)` where `handle.asset_id()` returns the raw relationship ID with no `zip://` prefix. Writers should apply their own missing-asset policy.

`wp:docPr/@descr` maps to `Event::Image.alt`. The `title` field is always `None` in this release.

VML images (`<w:pict>` containing `<v:imagedata>`) are also supported, using the same asset resolution as DrawingML images. V1 limitations: VML positioning attributes (`style`, `w10:wrap`, `w10:anchorlock`), `<v:textbox>` text content, and inline image bytes via `<w:binData>` (referenced from `<v:imagedata src="wordml://..."/>`) are silently dropped.

### Asset Handles

Each `ImageSource::Asset(handle)` carries an `Arc<dyn AssetHandle>` that holds everything needed to resolve the asset. Call methods directly on the handle:

```rust,no_run
use docspec_docx_reader::{DocxReader, EventSource};
use docspec_core::{Event, ImageSource};

let mut reader = DocxReader::from_path("document.docx")?;
while let Some(event) = reader.next_event()? {
    if let Event::Image { source: ImageSource::Asset(handle), .. } = &event {
        let mut buf = Vec::new();
        handle.stream_to(&mut buf)?;
        // buf now contains the raw image bytes
    }
}
# Ok::<(), docspec_core::Error>(())
```

## Streaming Guarantee

Memory use depends on which constructor you call:

- **`from_path`**: streams `word/document.xml` in constant memory — O(1) regardless
  of document size. Use this when processing large files or when memory is constrained.
- **`from_reader`**: buffers `word/document.xml` into memory — O(N) in document size.
  Use `from_path` when constant memory is required.

In both cases, `_rels/.rels` and `word/_rels/document.xml.rels` are fully read into
memory at package-open time (typical combined size < 10 KB even for large documents).
The internal event queue remains bounded regardless of document size or hyperlink count.

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
- [ARCHITECTURE.md](../../ARCHITECTURE.md) — pipeline design, event model decisions, and pointers to the in-code event reference
- [`docspec_core` on docs.rs](https://docs.rs/docspec-core) — every event variant, field, and well-formedness rule
