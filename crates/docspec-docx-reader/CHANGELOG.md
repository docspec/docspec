# Changelog

## 1.0.0 — 2026-06-03

- Initial release.
- Streaming DOCX reader implementing `EventSource`.
- Emits `StartDocument`, `StartParagraph`, `Text`, `EndParagraph`, `EndDocument` only.
- Supports `Stored` and `Deflated` ZIP compression.
- Discovers document target via `_rels/.rels` relationship file.
- Silently drops tables, tracked changes, hyperlinks, drawings, and all styling.
- Constant memory streaming of `document.xml` regardless of file size.
