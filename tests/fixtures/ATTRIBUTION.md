# Attribution — DOCX Test Fixtures

The `tests/fixtures/docx/` tree contains test inputs from two corpora with
different provenance and licensing. Each corpus lives in its own subdirectory.

## DocSpec-Curated Corpus (`docx/docspec/`)

- **Title**: DocSpec DOCX Regression Corpus
- **Author**: DocSpec contributors
- **License**: ODC-By-1.0 (Open Data Commons Attribution License v1.0) — SPDX identifier `ODC-By-1.0`; canonical text at <https://opendatacommons.org/licenses/by/1-0/>
- **Source**: Original work, generated in-tree by DocSpec maintainers to exercise reader behaviour not covered by the Pandoc corpus.

These fixtures are **our own**. You may copy, distribute, and adapt them under
ODC-By-1.0 provided attribution to DocSpec is preserved and any changes are
indicated.

### Fixtures

| Fixture | Primary event coverage |
|---------|------------------------|
| `preformatted-boundaries.docx` | Consecutive code-styled paragraphs consolidated into one `StartPreformatted` / `EndPreformatted` block separated by `LineBreak` events; clean closure before regular paragraphs, headings, block quotes, list items, tables, and table cells; consecutive code paragraphs inside a table cell. |

## Pandoc DOCX Test Suite (`docx/pandoc/`)

- **Title**: Pandoc DOCX Test Suite
- **Author**: John MacFarlane and Pandoc contributors
- **Upstream**: <https://github.com/jgm/pandoc>, `test/docx/`
- **Mirror**: <https://github.com/docspec/documents>, `documents/docx/pandoc/`
- **License**: GPL-2.0-or-later — SPDX identifier `GPL-2.0-or-later`; canonical text at <https://www.gnu.org/licenses/old-licenses/gpl-2.0.txt>
- **Imported**: 2026-06-22

## Apache Tika DOCX Test Documents (`docx/apache-tika/`)

- **Title**: Apache Tika DOCX Test Documents
- **Author**: The Apache Software Foundation and Apache Tika contributors
- **Upstream**: <https://github.com/apache/tika>
- **Mirror**: <https://github.com/docspec/documents>, `documents/docx/apache-tika/`
- **License**: Apache-2.0 — SPDX identifier `Apache-2.0`; canonical text at <https://www.apache.org/licenses/LICENSE-2.0.txt>
- **Imported**: 2026-06-22

Selective import — see `docx/apache-tika/README.md` for the fixture list and
the criterion for adding new ones.

## License Compatibility

DocSpec is licensed MIT. All three fixture corpora are test data only:

- They are NOT compiled into any DocSpec binary or library.
- They are NOT distributed via crates.io (excluded via `Cargo.toml exclude`).
- They are used solely as test inputs, consistent with GPL Section 0 scope for the Pandoc corpus, the Apache-2.0 permission grant for the Apache Tika corpus, and unconditionally permitted for the DocSpec corpus under ODC-By-1.0.
- The verbatim GPL-2.0, Apache-2.0, and ODC-By-1.0 texts are intentionally not vendored; see the canonical URLs above.

## Mirrored Corpus

The `docx/pandoc/` directory mirrors the Pandoc DOCX fixture corpus from the
sibling `documents` repository. See `docx/pandoc/README.md` for scope notes and
expected silent drops.

## Refreshing the Pandoc Corpus

To refresh fixtures from the sibling `documents` repo:

```bash
rsync -a ../documents/documents/docx/pandoc/*.docx tests/fixtures/docx/pandoc/
```

Then run `INSTA_UPDATE=unseen cargo test --test snapshots -p docspec-docx-reader`
to generate any new snapshots, review them with `cargo insta review`, and commit.
Update the **Imported** line above when refreshing en masse.

## Adding a DocSpec Fixture

Drop a new `.docx` in `docx/docspec/`, add a row to the table above, then run
`INSTA_UPDATE=unseen cargo test --test snapshots -p docspec-docx-reader`
to materialise the snapshot, review it with `cargo insta review`, and commit
the fixture, the snapshot, and this attribution update together.
