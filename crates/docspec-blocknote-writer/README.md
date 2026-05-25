# docspec-blocknote-writer

DocSpec event stream to BlockNote JSON writer.

See the [main DocSpec repository](https://github.com/docspec/docspec) for documentation.

## Known Limitations

### GFM tables flatten to paragraph blocks

`MarkdownReader` emits structured table events (`StartTable`, `StartTableRow`,
`StartTableHeader`, `StartTableCell`, etc.) per [EVENTS.md](../../EVENTS.md).
`BlockNoteWriter` silently ignores those table-structure events. When the
writer is wrapped in `StackTrackingSink` (the intended use), text inside each
cell triggers automatic paragraph insertion, so each cell's content ends up as
an individual `paragraph` block rather than a BlockNote `table` block.

**Impact**: Cell *content* (including inline formatting such as bold, italic,
code, and links) is preserved. Table *structure* (rows, columns, headers,
captions, colspan/rowspan, header scope) is lost.

**Status**: Implementing the BlockNote `table` block is deferred future work.
The current behavior is the documented expectation; see
`tests/fixtures/blocknote/tables.json` in the repository root.
