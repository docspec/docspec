# Attribution — Pandoc DOCX Test Fixtures

## Source

- **Title**: Pandoc DOCX Test Suite
- **Author**: John MacFarlane and Pandoc contributors
- **Upstream**: <https://github.com/jgm/pandoc>, `test/docx/`
- **Mirror**: <https://github.com/docspec/documents>, `documents/docx/pandoc/`
- **License**: GPL-2.0-or-later — SPDX identifier `GPL-2.0-or-later`; canonical text at <https://www.gnu.org/licenses/old-licenses/gpl-2.0.txt>
- **Imported**: 2026-06-16 @ 50c5c831

## License Compatibility

DocSpec is licensed MIT. These `.docx` fixtures are GPL-2.0-or-later test data:

- They are NOT compiled into any DocSpec binary or library.
- They are NOT distributed via crates.io (excluded via `Cargo.toml exclude`).
- They are used solely as test inputs, consistent with GPL Section 0 scope.
- The verbatim GPL-2.0 text is intentionally not vendored; see the canonical URL above.

## Curated Subset

This directory contains a curated 5-fixture starter set, not the full upstream
corpus. See `README.md` for the selection rationale and event-coverage table.

## Refreshing or Expanding the Corpus

To copy additional fixtures from the sibling `documents` repo:

```bash
rsync -a /path/to/documents/documents/docx/pandoc/<fixture>.docx tests/fixtures/docx/pandoc/
```

Then run `INSTA_UPDATE=unseen cargo test --test pandoc_corpus -p docspec-docx-reader`
to generate the new snapshot, review it with `cargo insta review`, and commit.
Update the **Imported** line above when refreshing en masse.
