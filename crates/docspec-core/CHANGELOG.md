# Changelog

## [0.2.0](https://github.com/docspec/docspec/compare/v0.1.0...v0.2.0) (2026-06-01)


### Features

* **core:** add Event::SoftBreak variant ([5d7e408](https://github.com/docspec/docspec/commit/5d7e40813a0de18eec58f4893032e3babedc9812))
* **core:** add StackTrackingSink for event stream normalization ([#12](https://github.com/docspec/docspec/issues/12), [#14](https://github.com/docspec/docspec/issues/14), [#16](https://github.com/docspec/docspec/issues/16)) ([2cd6c5c](https://github.com/docspec/docspec/commit/2cd6c5c9143a54b5fcb268ae82ba008ebe3338ce))
* **core:** implement docspec-core crate with streaming event types ([#10](https://github.com/docspec/docspec/issues/10), [#35](https://github.com/docspec/docspec/issues/35)) ([92b7941](https://github.com/docspec/docspec/commit/92b794188b1642933c2f0019a0222ad74092a0fe))
* **json:** extract JSON writing primitives to docspec-json crate ([37a2da8](https://github.com/docspec/docspec/commit/37a2da8c6436899cee5d6b45d5a39e153a554ca9))
* Markdown reader and BlockNote writer ([#39](https://github.com/docspec/docspec/issues/39), [#12](https://github.com/docspec/docspec/issues/12), [#10](https://github.com/docspec/docspec/issues/10), [#13](https://github.com/docspec/docspec/issues/13)) ([90c27d3](https://github.com/docspec/docspec/commit/90c27d3689fa99d0bfa1ea59c9383ae9bf754f29))
* **markdown-reader:** emit ordered and unordered list events ([2185ca0](https://github.com/docspec/docspec/commit/2185ca001a5dfc10830938519fea188a8703f4f5)), closes [#37](https://github.com/docspec/docspec/issues/37) [#10](https://github.com/docspec/docspec/issues/10)
* **markdown,blocknote:** support block quotes and thematic break dividers ([#10](https://github.com/docspec/docspec/issues/10), [#37](https://github.com/docspec/docspec/issues/37), [#12](https://github.com/docspec/docspec/issues/12)) ([275c1e0](https://github.com/docspec/docspec/commit/275c1e07170651b7519ecb9122542ad8551753b4))


### Bug Fixes

* add id's to every event type ([#10](https://github.com/docspec/docspec/issues/10)) ([84b614a](https://github.com/docspec/docspec/commit/84b614af0b1ff925a7e339eef2a1f9c5ecc94fc7))
* **core:** validate EndDocument and remove Blockquote from content-bearing ([#10](https://github.com/docspec/docspec/issues/10)) ([d8eb71a](https://github.com/docspec/docspec/commit/d8eb71a534e82feb090adf47614c025194c04e59))
* **core:** validate single StartDocument in StackTrackingSink ([#10](https://github.com/docspec/docspec/issues/10), [#12](https://github.com/docspec/docspec/issues/12)) ([2853b14](https://github.com/docspec/docspec/commit/2853b1431142b7924f1c65c49927e73a520896ce))
