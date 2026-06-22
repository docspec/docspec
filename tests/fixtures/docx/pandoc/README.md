# Pandoc DOCX Test Fixtures

This directory mirrors the DOCX fixtures from the
[Pandoc](https://github.com/jgm/pandoc) test suite (`test/docx/`), tracked via
Git LFS. See `../../ATTRIBUTION.md` for license and source details.

## Corpus Scope

The corpus is copied from the sibling `documents` mirror at
`documents/docx/pandoc/`. It intentionally preserves the upstream fixture names
so the `pandoc` module in `crates/docspec-docx-reader/tests/snapshots.rs` can
emit one test per `.docx` via `build.rs`.

The fixtures cover headings, inline formatting, links, lists, tables, images,
notes, metadata, tracked changes, structured document tags, and other WordprocessingML
edge cases represented in Pandoc's regression suite.

## Expected Silent Drops

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

## Refreshing the Corpus

To refresh from the sibling `documents` repo:

```bash
rsync -a ../documents/documents/docx/pandoc/*.docx tests/fixtures/docx/pandoc/
```

Then run `INSTA_UPDATE=unseen cargo test --test snapshots -p docspec-docx-reader`
to generate any new snapshots, review them with `cargo insta review`, and commit.
Update the **Imported** line in `../../ATTRIBUTION.md` when refreshing en masse.
