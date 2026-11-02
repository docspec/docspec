# docspec-html-writer

**Streaming HTML5 writer for DocSpec events**

HTML, written as it flows. `docspec-html-writer` turns a stream of DocSpec events into
HTML5 without ever assembling the document in memory. Paragraphs become `<p>`, text is
escaped as it passes, and nothing accumulates — streaming, like the rest of DocSpec.
(See the [Manifesto](https://github.com/docspec/docspec/blob/main/MANIFESTO.md) for why.)

## Add it

```toml
[dependencies]
docspec-html-writer = "1"
docspec-core = "1"
```

## Write some HTML

`HtmlWriter` is an `EventSink`: hand it events and it writes HTML5 as they arrive. We open
`<html><body>`, emit a `<p>` for each paragraph, escape every character as it passes, and
close everything when the document ends:

```rust,no_run
use docspec_html_writer::HtmlWriter;
use docspec_core::{Event, EventSink};

let mut html = Vec::new();
let mut writer = HtmlWriter::new(&mut html);
for event in [
    Event::StartDocument { id: None, language: None, metadata: None },
    Event::StartParagraph { alignment: None, id: None },
    Event::Text { content: "Hello, world".into() },
    Event::EndParagraph,
    Event::EndDocument,
] {
    writer.handle_event(event)?;
}
writer.finish()?;

assert_eq!(html, b"<html><body><p>Hello, world</p></body></html>");
# Ok::<(), docspec_core::Error>(())
```

In a real pipeline you feed it from a reader with `docspec_core::pipe(reader, writer)`
rather than hand-writing events.

## What it handles today

Handled: `StartDocument`, `EndDocument`, `StartParagraph`, `EndParagraph`,
`StartPreformatted`, `EndPreformatted`, and `Text` while a paragraph or preformatted
block is open. Text is escaped for `&`, `<`, and `>`.

A preformatted block becomes `<pre><code>…</code></pre>`. When `StartPreformatted`
carries a `syntax`, `<code>` gets `class="language-{syntax}"` (the attribute value is
escaped). Inside the block, text keeps its whitespace and newlines, and `LineBreak` and
`SoftBreak` each become a newline. The code is written as it arrives, never held back.
Block ids are not written, for code blocks as for paragraphs.

Silently ignored: `StartBlockQuote`/`EndBlockQuote`, `StartCaption`/`EndCaption`, every
definition-list event, `StartFootnote`/`EndFootnote`/`FootnoteRef`,
`StartHeading`/`EndHeading`, `StartLink`/`EndLink`, both list-item pairs, every table
event, `StartTextStyle`/`EndTextStyle`, `Image`, `ThematicBreak`, and `LineBreak` and
`SoftBreak` outside a preformatted block. `Text` outside a paragraph or preformatted
block is also ignored, as is a block that starts while another is still open. An open
paragraph or preformatted block is closed automatically on `EndDocument`.

## Related

- [Architecture](https://github.com/docspec/docspec/blob/main/ARCHITECTURE.md) — the streaming pipeline and event model
- [`docspec-html-writer` on docs.rs](https://docs.rs/docspec-html-writer)
