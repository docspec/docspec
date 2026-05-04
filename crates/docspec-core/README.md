# docspec-core

Core types and traits for the DocSpec streaming document pipeline. Documents are streams of typed events flowing from `EventSource` readers to `EventSink` writers.

## Events

The `Event` enum defines all atomic units of document structure. Events come in three categories: Start/End pairs for containers, self-contained block events, and self-contained inline events.

### Document Structure

- `StartDocument` / `EndDocument` — `language: Option<String>`, `metadata: Option<DocumentMeta>`
- `StartHeading` / `EndHeading` — `level: u8`, `id: Option<String>`
- `StartParagraph` / `EndParagraph` — `alignment: Option<TextAlignment>`, `id: Option<String>`
- `StartBlockQuote` / `EndBlockQuote`
- `StartPreformatted` / `EndPreformatted` — `syntax: Option<String>`
- `StartFootnote` / `EndFootnote` — `id: u32`

### Lists

- `StartListItem` / `EndListItem` — `level: u8`, `list_type: ListType`, `start: Option<u32>`, `style_type: Option<ListStyleType>`

### Tables

- `StartTable` / `EndTable`
- `StartCaption` / `EndCaption`
- `StartTableRow` / `EndTableRow`
- `StartTableHeader` / `EndTableHeader` — `scope: Option<TableHeaderScope>`, `abbr: Option<String>`, `colspan: Option<u32>`, `rowspan: Option<u32>`
- `StartTableCell` / `EndTableCell` — `colspan: Option<u32>`, `rowspan: Option<u32>`

### Definition Lists

- `StartDefinitionList` / `EndDefinitionList`
- `StartDefinitionTerm` / `EndDefinitionTerm`
- `StartDefinitionDetail` / `EndDefinitionDetail`

### Inline Containers

- `StartLink` / `EndLink` — `href: String`, `title: Option<String>`

### Self-Contained Block Events

- `ThematicBreak`

### Self-Contained Inline Events

- `Text` — `content: String`, `bold: bool`, `italic: bool`, `code: bool`, `strikethrough: bool`, `underline: bool`, `subscript: bool`, `superscript: bool`, `mark: Option<Color>`
- `Image` — `source: ImageSource`, `alt: Option<String>`, `title: Option<String>`, `decorative: bool`, `id: Option<String>`
- `FootnoteRef` — `id: u32`
- `LineBreak`
