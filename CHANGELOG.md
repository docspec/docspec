# Changelog

All notable changes to DocSpec are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and DocSpec uses **ecosystem-level Semantic Versioning**: all 12 crates share a
single version, and any breaking change in any crate increments the ecosystem
major version. See [`RELEASING.md`](RELEASING.md) for the full policy.

For pre-1.5.0 history of individual crates, see git tags matching
`{crate}-v{version}` (e.g., `docspec-cli-v1.3.1`).

## [Unreleased]

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
