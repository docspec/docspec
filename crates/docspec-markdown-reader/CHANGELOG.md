# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.0](https://github.com/docspec/docspec/releases/tag/docspec-markdown-reader-v0.5.0) - 2026-06-02

### Added

- [**breaking**] crates.io publishing
- *(markdown-reader)* defer StartParagraph emission to elide empty wrappers
- *(core)* add Event::SoftBreak variant
- *(markdown-reader,blocknote-writer)* emit and serialize inline links
- *(json)* extract JSON writing primitives to docspec-json crate
- *(markdown-reader)* emit ordered and unordered list events
- *(markdown-reader)* emit table structure events
- *(core)* add code, strikethrough, underline text formatting support
- *(core)* add StackTrackingSink for event stream normalization
- *(markdown-reader)* preformatted/code block
- *(markdown-reader,blocknote-writer)* support block quotes and thematic break dividers
- *(markdown-reader,blocknote-writer)* Markdown reader and BlockNote writer

### Fixed

- *(markdown-reader)* remove redundant code style from preformatted text events
- dependency cycle
- *(markdown-reader)* keep parent item open during nested list
- *(markdown-reader)* buffer code block text for proper newline stripping
- *(core)* validate EndDocument and remove Blockquote from content-bearing
- *(blocknote-writer)* blockquote text no longer lost to separate paragraph
- *(core)* add id's to every event type

### Other

- *(ci)* switch to release-plz
- release main
- release main
- release main
- release main
- *(tests)* improve quality
- reduce repetition across crates
- *(markdown-reader)* split tag dispatch into per-tag handlers
- *(blocknote-writer)* document tables-flatten-to-paragraphs limitation
- *(core)* extract TextStyle struct from Event::Text
- *(release)* configure release-please and renovate
- add CI workflow for all tests, fix test quality
