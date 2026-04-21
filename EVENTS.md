# Streaming Events Specification

DocSpec documents are streams of typed events. Readers emit events. Writers consume them. Events flow one at a time in document order. This specification defines every event type, its semantics, and the rules that govern well-formed streams.

## Design Decisions

**SAX-like structure.** Block elements use Start/End pairs.

**Formatting as attributes.** Inline formatting lives on `Text` as attributes, not wrapper events. Links are the exception: they use Start/End because they carry `href`.

**Semantic fidelity.** Events capture meaning, not appearance. Visual properties (font, color, size) are not represented — except mark color for highlighting.

**Lazy asset references.** Images carry references, not bytes. Writers resolve via `AssetProvider`.

**No warnings.** Errors are out-of-band via `Result`. Events carry content only.

**Flat list model.** List items carry nesting level as a field. No `StartList`/`EndList`. Maps cleanly to DOCX/RTF where list structure is implicit.

---

## Error Handling

```rust
fn next_event(&mut self) -> Result<Option<Event>, Error>
```

| Return            | Meaning                      |
| ----------------- | ---------------------------- |
| `Ok(Some(event))` | Content event                |
| `Ok(None)`        | Stream ended normally        |
| `Err(e)`          | Fatal error, stop processing |

Events never carry errors or warnings. If a reader cannot continue, it returns `Err`. If it can recover silently, it does — no event emitted for the recovery.

**Recoverable:** missing optional attributes, unrecognized elements (skip them), unsupported features.
**Fatal:** malformed structure, truncated stream, invalid encoding.

---

## Asset References

```rust
trait AssetProvider: Send + Sync {
    fn content_type(&self, asset_id: &str) -> Option<Cow<'_, str>>;
    fn stream_to(&self, asset_id: &str, writer: &mut dyn Write) -> Option<io::Result<u64>>;
}
```

Readers register assets as encountered. Writers call `stream_to()` on demand — bytes stream, never buffer. Assets must remain accessible until `EndDocument`.

---

## Type Definitions

```rust
enum TextAlignment { Left, Center, Right, Justify }

enum ListType { Ordered, Unordered }

enum ListStyleType {
    // Ordered list styles
    Decimal, LowerAlpha, UpperAlpha, LowerRoman, UpperRoman,
    // Unordered list styles
    Disc, Circle, Square,
}
// Writers ignore mismatched styles (e.g., Disc on an ordered list).

enum TableHeaderScope { Column, Row }  // Column: header describes cells below; Row: header describes cells to the right

enum Color { Rgb { r: u8, g: u8, b: u8 } }

enum ImageSource {
    Asset { asset_id: String },   // resolved through AssetProvider
    Uri { uri: String },          // external resource
}

struct DocumentMeta {
    title: Option<String>,
    authors: Option<Vec<Author>>,
    description: Option<String>,
}

struct Author {
    name: String,
    email: Option<String>,
}

```

All strings are UTF-8. No normalization form is required; readers preserve source encoding, writers preserve normalization.

---

## Event Reference

All events in the `Event` enum, grouped by category.

**Document structure:**

| Event                  | Fields                                                       | Pair                 |
| ---------------------- | ------------------------------------------------------------ | -------------------- |
| `StartDocument`        | `language: Option<String>`, `metadata: Option<DocumentMeta>` | `EndDocument`        |
| `StartHeading`         | `level: u8`                                                  | `EndHeading`         |
| `StartParagraph`       | `alignment: Option<TextAlignment>`                           | `EndParagraph`       |
| `StartBlockQuote`      | —                                                            | `EndBlockQuote`      |
| `StartPreformatted`    | `syntax: Option<String>`                                     | `EndPreformatted`    |
| `StartFootnote`        | `id: u32`                                                    | `EndFootnote`        |

**Lists:**

| Event            | Fields                                                                                         | Pair           |
| ---------------- | ---------------------------------------------------------------------------------------------- | -------------- |
| `StartListItem`  | `level: u8`, `list_type: ListType`, `start: Option<u32>`, `style_type: Option<ListStyleType>` | `EndListItem`  |

**Tables:**

| Event              | Fields                                                                                                    | Pair             |
| ------------------ | --------------------------------------------------------------------------------------------------------- | ---------------- |
| `StartTable`       | —                                                                                                         | `EndTable`       |
| `StartCaption`     | —                                                                                                         | `EndCaption`     |
| `StartTableRow`    | —                                                                                                         | `EndTableRow`    |
| `StartTableHeader` | `scope: Option<TableHeaderScope>`, `abbr: Option<String>`, `colspan: Option<u32>`, `rowspan: Option<u32>` | `EndTableHeader` |
| `StartTableCell`   | `colspan: Option<u32>`, `rowspan: Option<u32>`                                                            | `EndTableCell`   |

**Definition lists:**

| Event                   | Fields | Pair                  |
| ----------------------- | ------ | --------------------- |
| `StartDefinitionList`   | —      | `EndDefinitionList`   |
| `StartDefinitionTerm`   | —      | `EndDefinitionTerm`   |
| `StartDefinitionDetail` | —      | `EndDefinitionDetail` |

**Inline containers:**

| Event       | Fields                                  | Pair      |
| ----------- | --------------------------------------- | --------- |
| `StartLink` | `href: String`, `title: Option<String>` | `EndLink` |

**Block (self-contained):**

| Event           | Fields |
| --------------- | ------ |
| `ThematicBreak` | —      |

**Inline (self-contained):**

| Event         | Fields                                                                                                                                                                 |
| ------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Text`        | `content: String`, `bold: bool`, `italic: bool`, `code: bool`, `strikethrough: bool`, `underline: bool`, `subscript: bool`, `superscript: bool`, `mark: Option<Color>` |
| `Image`       | `source: ImageSource`, `alt: Option<String>`, `title: Option<String>`, `decorative: bool`                                                                              |
| `FootnoteRef` | `id: u32`                                                                                                                                                              |
| `LineBreak`   | —                                                                                                                                                                      |

---

## Semantics

Every `Start*` has a matching `End*`. They nest but never overlap.

**Document.** The root container. `language` is a BCP 47 tag.

**Heading.** Levels 1–6 are standard (HTML). DOCX/ODT/RTF support 1–9. Writers clamp higher levels. Both heading and list levels are 1-based (range 1–255); no format exceeds 9.

**List items.** Nesting is flat — children follow parents sequentially, distinguished by `level`. No `StartList`/`EndList` exists. The `start` field sets numbering base until another `start` appears. **Boundary rules:** a new list begins when (a) a non-list block intervenes, (b) `list_type` changes at the same level, or (c) level decreases then increases without a parent.

**Table.** `StartCaption` is optional, appears before rows. Header cells carry `scope`/`abbr` for accessibility; data cells omit these. Cells may contain any block element.

**BlockQuote.** May contain any block element.

**Preformatted.** When `syntax` is present, block has code semantics. Formatting attributes on inner `Text` are ignored. Newlines in content are literal.

**Definition list.** Terms contain inline content only. Details can contain any block element.

**Footnote.** Readers emit `StartFootnote` as soon as practical (placement varies by source format). `FootnoteRef` may appear before or after its corresponding `StartFootnote`. Writers decide final placement and must buffer if needed. Footnotes contain paragraphs only; this may relax in future.

**Link.** An inline container (uses Start/End because it carries `href`). Valid inside paragraphs, headings, list items, cells, definition details. Cannot nest.

**Text.** Formatting changes produce new events. Default: all bools `false`, mark `None`. Empty content is valid but meaningless. `subscript` and `superscript` may both be true; writers that can't represent both prefer `superscript`. Whitespace is significant. Outside preformatted blocks, newlines in content are collapsed to whitespace; readers emit `LineBreak` for semantically meaningful line breaks.

**Image.** Asset bytes resolve lazily via `AssetProvider`. `decorative` means purely visual. May appear inline or directly in block containers.

**FootnoteRef.** Inline marker; corresponding `StartFootnote` appears elsewhere.

**LineBreak.** Soft break within a block.

**ThematicBreak.** Horizontal rule / section separator.

---

## Well-Formedness Rules

Readers MUST produce well-formed streams. Writers MAY assume well-formedness.

1. Every `Start*` has exactly one matching `End*`. They nest but never overlap.
2. Exactly one root: `StartDocument`. Empty containers (`Start*` immediately followed by `End*`) are valid.
3. `Text` appears only inside containers, never at root.
4. `StartLink` appears inside inline-accepting blocks (paragraphs, headings, list items, cells, definition details). Links do not nest.
5. `StartListItem` appears inside block containers. List items are flat — distinguished by `level` field.
6. `StartCaption` appears at most once per table, before any rows.
7. Each footnote ID appears in exactly one `FootnoteRef` and one `StartFootnote`.
8. Table structure: `StartTableRow` appears only inside `StartTable`. `StartTableCell`/`StartTableHeader` appear only inside `StartTableRow`.

---

## Future Extensions

Deferred (non-breaking additions): named styles, visual properties (`StartSpan`), table sections/metadata, figure/caption, page/section breaks, anchors, in-stream warnings, embedded objects.
