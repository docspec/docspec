# Pandoc DOCX Test Fixtures

This directory contains a curated subset of 5 DOCX test fixtures from the
[Pandoc](https://github.com/jgm/pandoc) test suite (`test/docx/`),
tracked via Git LFS. See `ATTRIBUTION.md` for license and source details.

## Curated Subset

The starter set is chosen to maximise event-type coverage of `DocxReader`
while keeping the corpus small. Add more fixtures from the upstream Pandoc
suite as reader features land.

| Fixture | Primary event coverage |
|---------|------------------------|
| `headers.docx` | `StartDocument` / `StartParagraph` / `Text` / `EndParagraph` / `EndDocument` |
| `inline_formatting.docx` | `StartTextStyle` / `EndTextStyle` (bold, italic, underline, strikethrough, color, highlight) |
| `tables.docx` | `StartTable` / `StartTableRow` / `StartTableCell` / `StartTableHeader` and pairs |
| `lists.docx` | `StartOrderedListItem` / `StartUnorderedListItem` with `level` and `start` |
| `image.docx` | `Image` event with `AssetDescriptor` + SHA-256 of asset bytes |

## Expected Silent Drops (when adding more fixtures)

Some Pandoc fixtures produce near-empty snapshots because the current
`DocxReader` intentionally drops:

- **VML images** (`<w:pict>`) — subtree silently dropped; only DrawingML images
  (`<w:drawing>`) are supported.
- **Comments** — dropped entirely.
- **Track-changes deletions** (`<w:del>`, `<w:moveFrom>`) — dropped (accept-changes
  semantics).
- **Document metadata** (headers, footers, footnotes) — dropped.

This is expected behaviour, not a bug. When the reader gains support for these
features, snapshot diffs will surface the new output for review.
