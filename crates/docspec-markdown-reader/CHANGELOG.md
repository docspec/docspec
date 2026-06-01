# Changelog

## [0.2.0](https://github.com/docspec/docspec/compare/v0.1.0...v0.2.0) (2026-06-01)


### Features

* add code, strikethrough, underline text formatting support ([3aa6b50](https://github.com/docspec/docspec/commit/3aa6b50cfc3d0ea6dceae1387a320e65c0d2d4a4))
* **core:** add Event::SoftBreak variant ([5d7e408](https://github.com/docspec/docspec/commit/5d7e40813a0de18eec58f4893032e3babedc9812))
* **core:** add StackTrackingSink for event stream normalization ([#12](https://github.com/docspec/docspec/issues/12), [#14](https://github.com/docspec/docspec/issues/14), [#16](https://github.com/docspec/docspec/issues/16)) ([2cd6c5c](https://github.com/docspec/docspec/commit/2cd6c5c9143a54b5fcb268ae82ba008ebe3338ce))
* **json:** extract JSON writing primitives to docspec-json crate ([37a2da8](https://github.com/docspec/docspec/commit/37a2da8c6436899cee5d6b45d5a39e153a554ca9))
* Markdown reader and BlockNote writer ([#39](https://github.com/docspec/docspec/issues/39), [#12](https://github.com/docspec/docspec/issues/12), [#10](https://github.com/docspec/docspec/issues/10), [#13](https://github.com/docspec/docspec/issues/13)) ([90c27d3](https://github.com/docspec/docspec/commit/90c27d3689fa99d0bfa1ea59c9383ae9bf754f29))
* **markdown-reader,blocknote-writer:** emit and serialize inline links ([7eea150](https://github.com/docspec/docspec/commit/7eea1500220cff01176d64153a00c0364376937c))
* **markdown-reader:** emit ordered and unordered list events ([2185ca0](https://github.com/docspec/docspec/commit/2185ca001a5dfc10830938519fea188a8703f4f5)), closes [#37](https://github.com/docspec/docspec/issues/37) [#10](https://github.com/docspec/docspec/issues/10)
* **markdown-reader:** emit table structure events ([9f03fe8](https://github.com/docspec/docspec/commit/9f03fe8ee865758b1e35a20e4d26ac945215c47f)), closes [#37](https://github.com/docspec/docspec/issues/37)
* **markdown,blocknote:** support block quotes and thematic break dividers ([#10](https://github.com/docspec/docspec/issues/10), [#37](https://github.com/docspec/docspec/issues/37), [#12](https://github.com/docspec/docspec/issues/12)) ([275c1e0](https://github.com/docspec/docspec/commit/275c1e07170651b7519ecb9122542ad8551753b4))
* **markdown:** preformatted/code block ([#37](https://github.com/docspec/docspec/issues/37)) ([de1855c](https://github.com/docspec/docspec/commit/de1855cfa681c7ec50a7c713b6cb9002dbc4a9cb))


### Bug Fixes

* add id's to every event type ([#10](https://github.com/docspec/docspec/issues/10)) ([84b614a](https://github.com/docspec/docspec/commit/84b614af0b1ff925a7e339eef2a1f9c5ecc94fc7))
* **blocknote:** blockquote text no longer lost to separate paragraph ([#12](https://github.com/docspec/docspec/issues/12)) ([a753e97](https://github.com/docspec/docspec/commit/a753e9715592d7b4e8a52c924ef84dc262c74206))
* **core:** validate EndDocument and remove Blockquote from content-bearing ([#10](https://github.com/docspec/docspec/issues/10)) ([d8eb71a](https://github.com/docspec/docspec/commit/d8eb71a534e82feb090adf47614c025194c04e59))
* dependency cycle ([363a4c5](https://github.com/docspec/docspec/commit/363a4c511aa41167b864fa46816a957e57e3b4bb))
* **markdown-reader:** keep parent item open during nested list ([09216c7](https://github.com/docspec/docspec/commit/09216c75502de235146c049a79d7d2b3009048f1))
* **reader:** buffer code block text for proper newline stripping ([#37](https://github.com/docspec/docspec/issues/37), [#12](https://github.com/docspec/docspec/issues/12)) ([76c0e4c](https://github.com/docspec/docspec/commit/76c0e4c977a6913d01bc029891a5cc9721d9c516))
