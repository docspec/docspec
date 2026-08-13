# Changelog

All notable changes to DocSpec are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and DocSpec uses **ecosystem-level Semantic Versioning**: all 12 crates share a
single version, and any breaking change in any crate increments the ecosystem
major version. See [`RELEASING.md`](RELEASING.md) for the full policy.

For pre-1.5.0 history of individual crates, see git tags matching
`{crate}-v{version}` (e.g., `docspec-cli-v1.3.1`).

## [Unreleased]

## [1.21.9](https://github.com/docspec/docspec/compare/v1.21.8...v1.21.9) - 2026-08-13

### Fixed

- *(deps)* update rust crate posthog-rs to 0.23.0

### Other

- *(html-writer)* cover orphan end document
- update Cargo.lock dependencies

## [1.21.8](https://github.com/docspec/docspec/compare/v1.21.7...v1.21.8) - 2026-08-04

### Fixed

- *(docspec)* compile the reader doc examples only with their format
- *(docspec)* gate the Send assertions on a reader being compiled in
- *(docspec)* derive the reader and writer cfgs in build.rs
- *(docspec)* compile under every reader and writer feature selection

### Other

- *(docspec)* trim the comments around the derived cfgs
- *(docx-reader)* split document parsing into text, props, and media modules
- *(http)* extract conversion analytics into a private module

## [1.21.7](https://github.com/docspec/docspec/compare/v1.21.6...v1.21.7) - 2026-08-04

### Other

- *(docx-reader)* use a where clause for the extend trait bound
- *(docx-reader)* extract XmlCursor, EmitState, and PackageContext

## [1.21.6](https://github.com/docspec/docspec/compare/v1.21.5...v1.21.6) - 2026-08-03

### Fixed

- *(blocknote-writer)* close open child paragraph before emitting sibling block
- *(http)* adapt Sentry client options to 0.49 builder API
- *(deps)* update rust crate sentry to 0.49

### Other

- assert JSON output as readable documents
- *(memory)* resolve memtest child binaries via Cargo's target directory
- distinguish supported from experimental formats

## [1.21.5](https://github.com/docspec/docspec/compare/v1.21.4...v1.21.5) - 2026-08-03

### Fixed

- *(docspec)* stream DOCX in AnyReader::from_path instead of buffering
- *(http)* avoid panic when serializing error responses

### Other

- correct invalid commands, nonexistent APIs, and false capability claims
- *(markdown-reader)* drop dead_code allow and unused classify_start_tag
- *(http)* remove redundant inline allows already set workspace-wide

## [1.21.4](https://github.com/docspec/docspec/compare/v1.21.3...v1.21.4) - 2026-07-31

### Fixed

- *(deps)* update rust crate base64 to 0.23

### Other

- improve documentation

## [1.21.3](https://github.com/docspec/docspec/compare/v1.21.2...v1.21.3) - 2026-07-21

### Fixed

- *(deps)* update rust crate posthog-rs to 0.21.0
- *(deps)* update rust crate posthog-rs to 0.20.0

### Other

- fix clippy warnings

## [1.21.2](https://github.com/docspec/docspec/compare/v1.21.1...v1.21.2) - 2026-07-08

### Fixed

- *(docspec-http)* enable posthog Cargo feature by default

## [1.21.1](https://github.com/docspec/docspec/compare/v1.21.0...v1.21.1) - 2026-07-08

### Fixed

- *(docspec-http)* capture usable stacktraces on Sentry errors

## [1.21.0](https://github.com/docspec/docspec/compare/v1.20.1...v1.21.0) - 2026-07-08

### Added

- *(docspec-http)* add optional posthog analytics integration
- *(docspec-http)* propagate internal error source to Sentry

### Fixed

- *(deps)* update rust crate posthog-rs to 0.17.0

### Other

- add posthog feature flags and cargo-deny config

## [1.20.1](https://github.com/docspec/docspec/compare/v1.20.0...v1.20.1) - 2026-07-03

### Fixed

- *(blocknote-writer)* drop id field on list items

### Other

- update Cargo.lock dependencies

## [1.20.0](https://github.com/docspec/docspec/compare/v1.19.0...v1.20.0) - 2026-06-30

### Added

- *(docx-reader)* emit effective counter as start value on list resumption

## [1.19.0](https://github.com/docspec/docspec/compare/v1.18.3...v1.19.0) - 2026-06-30

### Added

- *(blocknote-writer)* unified lift policy for block nesting and cell content

### Fixed

- *(docx-reader)* ignore w:bCs and w:iCs in non-complex-script bold/italic detection

## [1.18.3](https://github.com/docspec/docspec/compare/v1.18.2...v1.18.3) - 2026-06-30

### Fixed

- *(docx-reader)* eagerly close lists on non-numbered paragraphs
- *(deps)* update rust crate quick-xml to 0.41

### Other

- update Cargo.lock dependencies

## [1.18.2](https://github.com/docspec/docspec/compare/v1.18.1...v1.18.2) - 2026-06-24

### Fixed

- *(http)* remove default 2 MiB request body cap

## [1.18.1](https://github.com/docspec/docspec/compare/v1.18.0...v1.18.1) - 2026-06-24

### Fixed

- *(docx-reader)* migrate paragraph-level parsing to recursive descent
- *(docx-reader)* migrate run-level parsing to recursive descent

### Other

- *(docx-reader)* replace denied state machine with read_to_end_into and remove dead dispatch
- *(docx-reader)* migrate table parsing to recursive descent
- *(docx-reader)* migrate hyperlink parsing to recursive descent

## [1.18.0](https://github.com/docspec/docspec/compare/v1.17.0...v1.18.0) - 2026-06-23

### Added

- *(blocknote-writer)* propagate colspan and rowspan to tableCell props

## [1.17.0](https://github.com/docspec/docspec/compare/v1.16.1...v1.17.0) - 2026-06-23

### Added

- *(blocknote-writer)* lift in-cell images to siblings after enclosing table

## [1.16.1](https://github.com/docspec/docspec/compare/v1.16.0...v1.16.1) - 2026-06-23

### Fixed

- *(docx-reader,blocknote-writer)* drop spurious gray background from no-op OOXML shading

## [1.16.0](https://github.com/docspec/docspec/compare/v1.15.0...v1.16.0) - 2026-06-23

### Added

- *(cli,http)* suppress empty headings, block quotes, and paragraphs by default
- *(core)* add SkipEmptyBlocks source adapter for empty Heading/BlockQuote/Paragraph suppression

### Other

- *(core)* make SkipEmptyBlocks::next_event iterative to guarantee O(1) stack
- *(docx-reader)* document StartHeading/StartBlockQuote emission and fix Out-of-Scope list

## [1.15.0](https://github.com/docspec/docspec/compare/v1.14.0...v1.15.0) - 2026-06-23

### Added

- *(docx-reader)* parse w:pict VML images

### Other

- update Cargo.lock dependencies

## [1.14.0](https://github.com/docspec/docspec/compare/v1.13.1...v1.14.0) - 2026-06-23

### Added

- *(docx-reader)* parse w:pict VML images

## [1.13.1](https://github.com/docspec/docspec/compare/v1.13.0...v1.13.1) - 2026-06-22

### Fixed

- *(docx-reader)* narrow preformatted style list and scope Verbatim Char to character styles

## [1.13.0](https://github.com/docspec/docspec/compare/v1.12.0...v1.13.0) - 2026-06-22

### Added

- *(docx-reader)* consolidate consecutive preformatted paragraphs into one block

### Other

- *(docx-reader)* unify DOCX corpus snapshot harness into snapshots.rs
- *(docx-reader)* exercise both DocxReader constructors in pandoc corpus, drop redundant synthetic tests
- *(docx-reader)* mirror pandoc DOCX corpus
- update Cargo.lock dependencies

## [1.12.0](https://github.com/docspec/docspec/compare/v1.11.0...v1.12.0) - 2026-06-22

### Added

- *(docx-reader)* consolidate consecutive preformatted paragraphs into one block

### Other

- *(docx-reader)* unify DOCX corpus snapshot harness into snapshots.rs
- *(docx-reader)* exercise both DocxReader constructors in pandoc corpus, drop redundant synthetic tests
- *(docx-reader)* mirror pandoc DOCX corpus

## [1.11.0](https://github.com/docspec/docspec/compare/v1.10.1...v1.11.0) - 2026-06-22

### Added

- *(docx-reader)* stream document XML from path

### Fixed

- *(deps)* update rust crate tower-http to 0.7

### Other

- correct DOCX capability claims
- *(core)* promote StyleStack from markdown-reader::html to docspec-core (no behavior change yet)
- simplify docx run state and copy value types
- *(blocknote-writer)* document highlight color name divergences
- *(blocknote-writer)* switch handler params to Option<&str>
- *(docx-reader)* introduce pandoc DOCX corpus snapshots
- *(docx-reader)* extract ensure_inline_context and emit_list_item_* helpers
- *(docx-reader)* extract shared paragraph-property-leaf and inline-atom helpers

## [1.10.1](https://github.com/docspec/docspec/compare/v1.10.0...v1.10.1) - 2026-06-12

### Fixed

- embed asset access in ImageSource::Asset via AssetHandle trait

## [1.10.0](https://github.com/docspec/docspec/compare/v1.9.0...v1.10.0) - 2026-06-11

### Added

- *(markdown-writer)* minimal markdown writer
- *(docx-reader)* image support
- *(markdown-reader)* translate raw HTML to DocSpec events

### Fixed

- *(blocknote-writer)* lift embedded elements/blocks
- *(docspec-docx-reader)* continued lists

### Other

- *(tests)* share event builders via docspec-test-utils
- *(tests)* centralize synth_docx fixture in docspec-test-utils
- *(tests)* share FailingWriter mock via docspec-test-utils
- *(tests)* share collect_events harness via docspec-test-utils
- *(tests)* share writer drive harness via docspec-test-utils
- *(docx-reader)* consolidate test collect_events helpers
- *(http)* centralize MIME constants

## [1.9.0](https://github.com/docspec/docspec/compare/v1.8.0...v1.9.0) - 2026-06-11

### Added

- *(docx-reader)* minimal list support
- *(docx-reader)* StartLink/EndLink for w:hyperlink
- *(pandoc-native-writer)* CodeBlock, Code

## [1.8.0](https://github.com/docspec/docspec/compare/v1.7.1...v1.8.0) - 2026-06-10

### Added

- *(docx-reader,blocknote-writer)* text color and highlight support
- *(core)* replace TextStyle with wrapper events
- *(docx-reader)* replace opaque-subtree counter with typed enum stack; shrink denylist
- *(docx-reader)* gridSpan, colspan, tblHeader, StartTableHeader support
- *(docx-reader)* symbol font character normalization (Wingdings/Webdings/Symbol)
- *(docx-reader)* Heading, BlockQuote, Preformatted
- *(pandoc-native-writer)* Headings
- *(pandoc-native-writer)* HorizontalRule, LineBreak, SoftBreak

### Fixed

- *(pandoc-native-writer)* prevent cross-close between paragraph and heading

### Other

- *(docx-reader)* split DOCX reader modules

## [1.7.1](https://github.com/docspec/docspec/compare/v1.7.0...v1.7.1) - 2026-06-09

### Fixed

- *(blocknote-writer)* skip default color props
- *(blocknote-writer)* skip default text alignment

## [1.7.0](https://github.com/docspec/docspec/compare/v1.6.0...v1.7.0) - 2026-06-09

### Added

- *(docx-reader)* honour w:rPr and w:pPr properties

### Other

- update Cargo.lock dependencies

## [1.6.0](https://github.com/docspec/docspec/compare/v1.5.1...v1.6.0) - 2026-06-09

### Added

- add DOCX input support across facade, CLI, and HTTP
- unify reader constructors
- make public enums non-exhaustive
- *(pandoc-native-writer)* Pandoc native writer
- *(docx-reader)* emit table events
- *(docx-reader)* emit Text("\t") for <w:tab>
- *(docx-reader)* emit LineBreak for <w:br>
- *(cli)* unify docspec-http into docspec as http subcommand

### Other

- *(docspec)* append InputFormat::Docx to preserve discriminants
- enforce dead_code lint and drop unused CliError variants

## [1.5.1](https://github.com/docspec/docspec/compare/v1.5.0...v1.5.1) - 2026-06-08

### Fixed

- *(docspec)* correctly organize features

### Other

- loosen semver violation checks
