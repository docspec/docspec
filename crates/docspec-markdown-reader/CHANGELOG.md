# Changelog

## [0.6.0](https://github.com/docspec/docspec/compare/v0.5.0...v0.6.0) (2026-06-02)


### Features

* **core:** add code, strikethrough, underline text formatting support ([d0a768d](https://github.com/docspec/docspec/commit/d0a768d9edafb944043b471ce78721d4c90bb164))
* **core:** add Event::SoftBreak variant ([be8b485](https://github.com/docspec/docspec/commit/be8b48527368440986934b28df7a4d1e414bc322)), closes [#12](https://github.com/docspec/docspec/issues/12)
* **core:** add StackTrackingSink for event stream normalization ([0555b0a](https://github.com/docspec/docspec/commit/0555b0aab7d278be5152b542fe2bef5905bb8c3a))
* **json:** extract JSON writing primitives to docspec-json crate ([8b51f24](https://github.com/docspec/docspec/commit/8b51f24fda81605db46335b49b823ac871f55d92))
* **markdown-reader,blocknote-writer:** emit and serialize inline links ([9592073](https://github.com/docspec/docspec/commit/9592073313c627257cfc7fd91ad978b3267e24a6))
* **markdown-reader,blocknote-writer:** Markdown reader and BlockNote writer ([8e56b1b](https://github.com/docspec/docspec/commit/8e56b1b2d70633ddc29732d48228f2cb22f24db6))
* **markdown-reader,blocknote-writer:** support block quotes and thematic break dividers ([91f7246](https://github.com/docspec/docspec/commit/91f7246a4e7fbfa97519317d9a4891f679965c9c))
* **markdown-reader:** defer StartParagraph emission to elide empty wrappers ([1606839](https://github.com/docspec/docspec/commit/1606839898e032e654730e8aa1a2cb325465def2)), closes [#37](https://github.com/docspec/docspec/issues/37)
* **markdown-reader:** emit ordered and unordered list events ([7ffec48](https://github.com/docspec/docspec/commit/7ffec480eca1a54cecc5ef20e2ec2abcc0b88664))
* **markdown-reader:** emit table structure events ([4dbef52](https://github.com/docspec/docspec/commit/4dbef52252c25c3e0b1ef683ad5ca5f65fdf0200)), closes [#37](https://github.com/docspec/docspec/issues/37)
* **markdown-reader:** preformatted/code block ([0881081](https://github.com/docspec/docspec/commit/08810818ad9f4c0a7570ba5cdf7dbd02b7d3d7d0)), closes [#37](https://github.com/docspec/docspec/issues/37)


### Bug Fixes

* **blocknote-writer:** blockquote text no longer lost to separate paragraph ([5bde87f](https://github.com/docspec/docspec/commit/5bde87fb6bbfaaef5f68813612f7fc4bcb90cc0a)), closes [#12](https://github.com/docspec/docspec/issues/12)
* **core:** add id's to every event type ([d14a5a6](https://github.com/docspec/docspec/commit/d14a5a6f84a88445e18acd0bfdccfb71991df2d4)), closes [#10](https://github.com/docspec/docspec/issues/10)
* **core:** validate EndDocument and remove Blockquote from content-bearing ([e1377ff](https://github.com/docspec/docspec/commit/e1377ff810a6b67d471add2bae8a3d58d609da9d)), closes [#10](https://github.com/docspec/docspec/issues/10)
* dependency cycle ([7315fd0](https://github.com/docspec/docspec/commit/7315fd0f8912941c7a69a7d5e8065ba258d1f584))
* **markdown-reader:** buffer code block text for proper newline stripping ([718b4c3](https://github.com/docspec/docspec/commit/718b4c3c37c634835a2b2a1826a02814c12f32ec))
* **markdown-reader:** keep parent item open during nested list ([d68e0a2](https://github.com/docspec/docspec/commit/d68e0a2c696e3c6efa9d937e641dbc1b220f06af)), closes [#37](https://github.com/docspec/docspec/issues/37)
* **markdown-reader:** remove redundant code style from preformatted text events ([a749578](https://github.com/docspec/docspec/commit/a749578039a61f0553b5408ef045e2fbc74fb0d1)), closes [#37](https://github.com/docspec/docspec/issues/37)

## [0.5.0](https://github.com/docspec/docspec/compare/v0.4.0...v0.5.0) (2026-06-02)


### Features

* **core:** add code, strikethrough, underline text formatting support ([d0a768d](https://github.com/docspec/docspec/commit/d0a768d9edafb944043b471ce78721d4c90bb164))
* **core:** add Event::SoftBreak variant ([be8b485](https://github.com/docspec/docspec/commit/be8b48527368440986934b28df7a4d1e414bc322)), closes [#12](https://github.com/docspec/docspec/issues/12)
* **core:** add StackTrackingSink for event stream normalization ([0555b0a](https://github.com/docspec/docspec/commit/0555b0aab7d278be5152b542fe2bef5905bb8c3a))
* **json:** extract JSON writing primitives to docspec-json crate ([8b51f24](https://github.com/docspec/docspec/commit/8b51f24fda81605db46335b49b823ac871f55d92))
* **markdown-reader,blocknote-writer:** emit and serialize inline links ([9592073](https://github.com/docspec/docspec/commit/9592073313c627257cfc7fd91ad978b3267e24a6))
* **markdown-reader,blocknote-writer:** Markdown reader and BlockNote writer ([8e56b1b](https://github.com/docspec/docspec/commit/8e56b1b2d70633ddc29732d48228f2cb22f24db6))
* **markdown-reader,blocknote-writer:** support block quotes and thematic break dividers ([91f7246](https://github.com/docspec/docspec/commit/91f7246a4e7fbfa97519317d9a4891f679965c9c))
* **markdown-reader:** defer StartParagraph emission to elide empty wrappers ([1606839](https://github.com/docspec/docspec/commit/1606839898e032e654730e8aa1a2cb325465def2)), closes [#37](https://github.com/docspec/docspec/issues/37)
* **markdown-reader:** emit ordered and unordered list events ([7ffec48](https://github.com/docspec/docspec/commit/7ffec480eca1a54cecc5ef20e2ec2abcc0b88664))
* **markdown-reader:** emit table structure events ([4dbef52](https://github.com/docspec/docspec/commit/4dbef52252c25c3e0b1ef683ad5ca5f65fdf0200)), closes [#37](https://github.com/docspec/docspec/issues/37)
* **markdown-reader:** preformatted/code block ([0881081](https://github.com/docspec/docspec/commit/08810818ad9f4c0a7570ba5cdf7dbd02b7d3d7d0)), closes [#37](https://github.com/docspec/docspec/issues/37)


### Bug Fixes

* **blocknote-writer:** blockquote text no longer lost to separate paragraph ([5bde87f](https://github.com/docspec/docspec/commit/5bde87fb6bbfaaef5f68813612f7fc4bcb90cc0a)), closes [#12](https://github.com/docspec/docspec/issues/12)
* **core:** add id's to every event type ([d14a5a6](https://github.com/docspec/docspec/commit/d14a5a6f84a88445e18acd0bfdccfb71991df2d4)), closes [#10](https://github.com/docspec/docspec/issues/10)
* **core:** validate EndDocument and remove Blockquote from content-bearing ([e1377ff](https://github.com/docspec/docspec/commit/e1377ff810a6b67d471add2bae8a3d58d609da9d)), closes [#10](https://github.com/docspec/docspec/issues/10)
* dependency cycle ([7315fd0](https://github.com/docspec/docspec/commit/7315fd0f8912941c7a69a7d5e8065ba258d1f584))
* **markdown-reader:** buffer code block text for proper newline stripping ([718b4c3](https://github.com/docspec/docspec/commit/718b4c3c37c634835a2b2a1826a02814c12f32ec))
* **markdown-reader:** keep parent item open during nested list ([d68e0a2](https://github.com/docspec/docspec/commit/d68e0a2c696e3c6efa9d937e641dbc1b220f06af)), closes [#37](https://github.com/docspec/docspec/issues/37)
* **markdown-reader:** remove redundant code style from preformatted text events ([a749578](https://github.com/docspec/docspec/commit/a749578039a61f0553b5408ef045e2fbc74fb0d1)), closes [#37](https://github.com/docspec/docspec/issues/37)

## [0.4.0](https://github.com/docspec/docspec/compare/v0.3.0...v0.4.0) (2026-06-01)


### Features

* add code, strikethrough, underline text formatting support ([3aa6b50](https://github.com/docspec/docspec/commit/3aa6b50cfc3d0ea6dceae1387a320e65c0d2d4a4))
* **core:** add Event::SoftBreak variant ([5d7e408](https://github.com/docspec/docspec/commit/5d7e40813a0de18eec58f4893032e3babedc9812))
* **core:** add StackTrackingSink for event stream normalization ([#12](https://github.com/docspec/docspec/issues/12), [#14](https://github.com/docspec/docspec/issues/14), [#16](https://github.com/docspec/docspec/issues/16)) ([2cd6c5c](https://github.com/docspec/docspec/commit/2cd6c5c9143a54b5fcb268ae82ba008ebe3338ce))
* **json:** extract JSON writing primitives to docspec-json crate ([37a2da8](https://github.com/docspec/docspec/commit/37a2da8c6436899cee5d6b45d5a39e153a554ca9))
* Markdown reader and BlockNote writer ([#39](https://github.com/docspec/docspec/issues/39), [#12](https://github.com/docspec/docspec/issues/12), [#10](https://github.com/docspec/docspec/issues/10), [#13](https://github.com/docspec/docspec/issues/13)) ([90c27d3](https://github.com/docspec/docspec/commit/90c27d3689fa99d0bfa1ea59c9383ae9bf754f29))
* **markdown-reader,blocknote-writer:** emit and serialize inline links ([7eea150](https://github.com/docspec/docspec/commit/7eea1500220cff01176d64153a00c0364376937c))
* **markdown-reader:** defer StartParagraph emission to elide empty wrappers ([ec1b157](https://github.com/docspec/docspec/commit/ec1b1576465dc4267416eb194dde1ad90370a014)), closes [#37](https://github.com/docspec/docspec/issues/37)
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
* **markdown-reader:** remove redundant code style from preformatted text events ([a37b70f](https://github.com/docspec/docspec/commit/a37b70fff7aedddf0c5aca3c650033b4bbf896d7)), closes [#37](https://github.com/docspec/docspec/issues/37)
* **reader:** buffer code block text for proper newline stripping ([#37](https://github.com/docspec/docspec/issues/37), [#12](https://github.com/docspec/docspec/issues/12)) ([76c0e4c](https://github.com/docspec/docspec/commit/76c0e4c977a6913d01bc029891a5cc9721d9c516))

## [0.3.0](https://github.com/docspec/docspec/compare/v0.2.0...v0.3.0) (2026-06-01)


### Features

* **markdown-reader:** defer StartParagraph emission to elide empty wrappers ([ec1b157](https://github.com/docspec/docspec/commit/ec1b1576465dc4267416eb194dde1ad90370a014)), closes [#37](https://github.com/docspec/docspec/issues/37)


### Bug Fixes

* **markdown-reader:** remove redundant code style from preformatted text events ([a37b70f](https://github.com/docspec/docspec/commit/a37b70fff7aedddf0c5aca3c650033b4bbf896d7)), closes [#37](https://github.com/docspec/docspec/issues/37)

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
