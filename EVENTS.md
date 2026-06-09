# Streaming Events Specification

DocSpec documents are streams of typed events. Readers emit events. Writers consume them. Events flow one at a time in document order. This specification defines every event type, its semantics, and the rules that govern well-formed streams.

## Design Decisions

**SAX-like structure.** Block elements use Start/End pairs.

**Formatting as wrappers.** Inline formatting is expressed via `StartTextStyle { kind: TextStyleKind, id: Option<String> }` and `EndTextStyle` wrapper events around `Text` content. This matches the Start/End uniformity of every other inline and block container.

**Semantic fidelity.** Events capture meaning, not appearance. Visual properties (font, size) are not represented. Color is represented in two places: `TextStyleKind::Mark(Color)` for highlight/background, and `TextStyleKind::TextColor(Color)` for foreground text color.

**Lazy asset references.** Images carry references, not bytes. Writers resolve via `AssetProvider`.

**No warnings.** Errors are out-of-band via `Result`. Events carry content only.

**Nested list items with level hints.** `StartOrderedListItem`/`StartUnorderedListItem` carry nesting level as a field and may nest in the event stream. No `StartList`/`EndList` exist. `level` is the authoritative indent depth — writers that do not build a tree may rely on it alone. Nesting in events lets continuation content (e.g., CommonMark paragraphs) sit naturally inside the owning parent item.

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

enum ListStyleType {
    // Ordered list styles
    Decimal, LowerAlpha, UpperAlpha, LowerRoman, UpperRoman,
    // Unordered list styles
    Disc, Circle, Square,
}
// Writers ignore mismatched styles (e.g., Disc on an ordered list).

enum TableHeaderScope { Column, Row }  // Column: header describes cells below; Row: header describes cells to the right

enum Color { Rgb { r: u8, g: u8, b: u8 } }

enum TextStyleKind {
    Bold, Italic, Code, Strikethrough, Underline, Subscript, Superscript,
    Mark(Color),  // highlight color
    TextColor(Color),  // foreground text color
}

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

| Event                  | Fields                                                                 | Pair                   |
| ---------------------- | ---------------------------------------------------------------------- | ---------------------- |
| `StartOrderedListItem` | `start: Option<u64>`, `style_type: ListStyleType`, `level: u32`, `id: Option<String>` | `EndOrderedListItem`   |
| `StartUnorderedListItem` | `style_type: ListStyleType`, `level: u32`, `id: Option<String>`                       | `EndUnorderedListItem` |

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

| Event             | Fields                                                  | Pair              |
| ----------------- | ------------------------------------------------------- | ----------------- |
| `StartLink`       | `href: String`, `title: Option<String>`                 | `EndLink`         |
| `StartTextStyle`  | `kind: TextStyleKind`, `id: Option<String>`             | `EndTextStyle`    |

**Block (self-contained):**

| Event           | Fields |
| --------------- | ------ |
| `ThematicBreak` | —      |

**Inline (self-contained):**

| Event         | Fields                                                                           |
| ------------- | -------------------------------------------------------------------------------- |
| `Text`        | `content: String`                                                                |
| `Image`       | `source: ImageSource`, `alt: Option<String>`, `title: Option<String>`, `decorative: bool` |
| `FootnoteRef` | `id: u32`                                                                        |
| `LineBreak`   | —                                                                                |
| `SoftBreak`   | —                                                                                |

---

## Semantics

Every `Start*` has a matching `End*`. They nest but never overlap.

**Document.** The root container. `language` is a BCP 47 tag.

**Heading.** Levels 1–6 are standard (HTML). DOCX/ODT/RTF support 1–9. Writers clamp higher levels. Heading levels are 1-based (range 1–9); list item `level` is 0-indexed (0 = top-level list).

**List items.** Child items may nest inside their parent's `Start*`/`End*` pair. The parent's `End*ListItem` appears AFTER all child items AND any continuation content (paragraphs, line breaks) that semantically belongs to the parent. `level` is 0-indexed (0 = top-level) and is the authoritative indent depth; writers that do not build a tree may rely on `level` alone. `StartOrderedListItem` carries `start: Option<u64>` populated only on the first item of each ordered list (subsequent items: `None`). **Boundary rules** (for sibling items at the same level): a new list begins when (a) a non-list block intervenes, (b) ordered vs. unordered changes at the same level, or (c) level decreases then increases without a parent.

**Table.** `StartCaption` is optional, appears before rows. Header cells carry `scope`/`abbr` for accessibility; data cells omit these. Cells may contain any block element.

**BlockQuote.** May contain any block element.

**Preformatted.** Inside `StartPreformatted`/`EndPreformatted`, no `StartTextStyle` events appear (Rule 11). When `syntax` is present, block has code semantics. Newlines in content are literal.

**Definition list.** Terms contain inline content only. Details can contain any block element.

**Footnote.** Readers emit `StartFootnote` as soon as practical (placement varies by source format). `FootnoteRef` may appear before or after its corresponding `StartFootnote`. Writers decide final placement and must buffer if needed. Footnotes contain paragraphs only; this may relax in future.

**Link.** An inline container (uses Start/End because it carries `href`). Valid inside paragraphs, headings, list items, cells, definition details. Cannot nest.

**StartTextStyle / EndTextStyle.** An inline-container pair carrying a single `TextStyleKind`. Valid inside paragraphs, headings, list items, cells, definition details. Style spans nest but never overlap (per Rule 1); readers MUST close-and-reopen to express overlapping source styles. `Subscript` and `Superscript` MAY both be active simultaneously by nesting; writers that cannot represent both prefer `Superscript`. The `Mark(Color)` variant carries the highlight/background color; the `TextColor(Color)` variant carries the foreground text color.

**Text.** Whitespace is significant. Outside preformatted blocks, newlines in content are collapsed to whitespace; readers emit `LineBreak` for explicit hard breaks (e.g., markdown two-space-newline, HTML `<br>`) and `SoftBreak` for soft breaks (e.g., source line wraps within a paragraph). Inline formatting is expressed via surrounding `StartTextStyle`/`EndTextStyle` wrapper events; the `Text` event itself carries content only.

**Image.** Asset bytes resolve lazily via `AssetProvider`. `decorative` means purely visual. May appear inline or directly in block containers.

**FootnoteRef.** Inline marker; corresponding `StartFootnote` appears elsewhere.

**LineBreak.** Explicit hard break within a paragraph (e.g., markdown two-space-newline, HTML `<br>`).

**SoftBreak.** Soft line break in source markup, such as a markdown line wrap. Writers choose rendering policy (space, newline, `<br>`, etc.).

**ThematicBreak.** Horizontal rule / section separator.

---

## Well-Formedness Rules

Readers MUST produce well-formed streams. Writers MAY assume well-formedness.

1. Every `Start*` has exactly one matching `End*`. They nest but never overlap.
2. Exactly one root: `StartDocument`. Empty containers (`Start*` immediately followed by `End*`) are valid.
3. `Text` appears only inside containers, never at root.
4. `StartLink` appears inside inline-accepting blocks (paragraphs, headings, list items, cells, definition details). Links do not nest.
5. `StartOrderedListItem` and `StartUnorderedListItem` appear inside block containers. List items may nest (child items inside parent `Start*`/`End*` pairs); the `level` field indicates indentation depth and is 0-indexed.
6. `StartCaption` appears at most once per table, before any rows.
7. Each footnote ID appears in exactly one `FootnoteRef` and one `StartFootnote`.
8. Table structure: `StartTableRow` appears only inside `StartTable`. `StartTableCell`/`StartTableHeader` appear only inside `StartTableRow`.
9. `StartTextStyle` and `EndTextStyle` nest but never overlap (subsumes Rule 1 but mentioned explicitly because of the close-and-reopen normalization requirement). Readers MUST normalize overlapping source styles into nested spans via close-and-reopen.
10. All open `StartTextStyle` spans MUST be closed before the enclosing block-end event (`EndParagraph`, `EndHeading`, `EndOrderedListItem`, `EndUnorderedListItem`, `EndTableCell`, `EndTableHeader`, `EndCaption`, `EndDefinitionTerm`, `EndDefinitionDetail`).
11. `StartTextStyle` MUST NOT appear inside `StartPreformatted`/`EndPreformatted`. Conversely, `StartPreformatted` MUST NOT appear inside `StartTextStyle`/`EndTextStyle` (block elements may not nest inside inline style spans).
12. When styled text appears inside a link, readers SHOULD emit `StartLink` as the outer container and `StartTextStyle` as the inner. The reverse nesting (style outside link) is well-formed but discouraged.
13. Readers MUST NOT emit empty style spans. A `StartTextStyle` MUST be followed by at least one `Text` event before its matching `EndTextStyle`. (Defers emission until non-empty content is confirmed.)

---

## Future Extensions

Deferred (non-breaking additions): named styles, visual properties (`StartSpan`), table sections/metadata, figure/caption, page/section breaks, anchors, in-stream warnings, embedded objects.
