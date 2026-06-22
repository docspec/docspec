# Apache Tika DOCX Test Fixtures

This directory contains a **selective** import of DOCX fixtures from the
[Apache Tika](https://github.com/apache/tika) test suite, tracked via Git LFS.
See `../../ATTRIBUTION.md` for license and source details.

Unlike the `pandoc/` corpus, this directory is NOT a full mirror — only the
fixtures needed to exercise reader behaviour not represented elsewhere are
imported. Adding a new fixture requires:

1. Copying it from `documents/docx/apache-tika/` in the sibling
   [documents](https://github.com/docspec/documents) repo.
2. Recording it below.
3. Running `INSTA_UPDATE=unseen cargo test --test snapshots -p docspec-docx-reader`
   to materialise the snapshot, reviewing it with `cargo insta review`, and
   committing the fixture, the snapshot, and this README update together.

## Fixtures

| Fixture | Primary event coverage |
|---------|------------------------|
| `017097.docx` | VML `<v:shape>` parents for `<v:imagedata>` with non-empty `alt` attributes, exercising shape alt-text extraction and `shape_depth` cleanup. |
| `testInstrLink.docx` | `<v:imagedata o:title=""/>` with no `r:id` / `r:embed` / `r:link` attribute, exercising the no-rId early-return in `emit_pict_imagedata`. |
