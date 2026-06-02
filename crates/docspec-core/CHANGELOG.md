# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.0](https://github.com/docspec/docspec/releases/tag/docspec-core-v0.5.0) - 2026-06-02

### Added

- [**breaking**] crates.io publishing
- *(core)* add Event::SoftBreak variant
- *(json)* extract JSON writing primitives to docspec-json crate
- *(markdown-reader)* emit ordered and unordered list events
- *(core)* add StackTrackingSink for event stream normalization
- *(markdown-reader,blocknote-writer)* support block quotes and thematic break dividers
- *(markdown-reader,blocknote-writer)* Markdown reader and BlockNote writer
- *(core)* implement docspec-core crate with streaming event types

### Fixed

- *(core)* validate single StartDocument in StackTrackingSink
- *(core)* validate EndDocument and remove Blockquote from content-bearing
- *(core)* add id's to every event type

### Other

- release main
- *(tests)* improve quality
- reduce repetition across crates
- *(core)* simplify block-kind lookup and StackTrackingSink dispatch
- *(core)* extract TextStyle struct from Event::Text
- *(release)* configure release-please and renovate
- *(tests)* move unit tests from src to tests directories
- add CI workflow for all tests, fix test quality
- *(coverage)* lower coverage threshold to 98% and exclude wasm/cli
- *(core)* replace event listing with doc reference
- *(core)* event type reference
- *(hooks)* add comprehensive pre-commit hooks and expand clippy lints to maximum strict
