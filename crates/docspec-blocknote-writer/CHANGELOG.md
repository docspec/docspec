# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.0](https://github.com/docspec/docspec/releases/tag/docspec-blocknote-writer-v0.5.0) - 2026-06-02

### Added

- [**breaking**] crates.io publishing
- *(core)* add Event::SoftBreak variant
- *(markdown-reader,blocknote-writer)* emit and serialize inline links
- *(blocknote-writer)* list support with nesting
- *(blocknote-writer)* emit native BlockNote table blocks
- *(json)* extract JSON writing primitives to docspec-json crate
- *(markdown-reader)* emit ordered and unordered list events
- *(core)* add code, strikethrough, underline text formatting support
- *(core)* add StackTrackingSink for event stream normalization
- *(blocknote-writer)* preformatted blocks / codeBlocks
- *(markdown-reader,blocknote-writer)* support block quotes and thematic break dividers
- *(cli)* scaffold CLI, improve I/O handling
- *(markdown-reader,blocknote-writer)* Markdown reader and BlockNote writer

### Fixed

- dependency cycle
- *(blocknote-writer)* handle image inside heading without panic
- *(blocknote-writer)* use double newline for paragraph separation in quotes
- *(core)* validate single StartDocument in StackTrackingSink
- *(markdown-reader)* buffer code block text for proper newline stripping
- *(core)* validate EndDocument and remove Blockquote from content-bearing
- *(blocknote-writer)* blockquote text no longer lost to separate paragraph
- *(core)* add id's to every event type

### Other

- *(ci)* switch to release-plz
- release main
- release main
- *(tests)* simplify testing infrastructure
- release main
- release main
- *(tests)* improve quality
- reduce repetition across crates
- *(blocknote-writer)* extract encode_asset_as_data_uri helper
- *(blocknote-writer)* document tables-flatten-to-paragraphs limitation
- *(blocknote-writer)* close JSON scopes via closures
- *(blocknote-writer)* simplify writing JSON
- *(core)* extract TextStyle struct from Event::Text
- *(release)* configure release-please and renovate
- *(tests)* move unit tests from src to tests directories
- add CI workflow for all tests, fix test quality
