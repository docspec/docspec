# Changelog

## [0.3.0](https://github.com/docspec/docspec/compare/v0.2.2...v0.3.0) (2026-06-02)


### Features

* **blocknote-writer:** emit native BlockNote table blocks ([f3b3d31](https://github.com/docspec/docspec/commit/f3b3d3112834fa276e05a4d820647cd0938a1714)), closes [#12](https://github.com/docspec/docspec/issues/12)
* **blocknote-writer:** list support with nesting ([6f7c1a9](https://github.com/docspec/docspec/commit/6f7c1a9283058f27f87f8c7481e34597e3ac5733)), closes [#12](https://github.com/docspec/docspec/issues/12)
* **blocknote-writer:** preformatted blocks / codeBlocks ([e1a95fd](https://github.com/docspec/docspec/commit/e1a95fd17ec702a305d6265f80b401f22893b61e)), closes [#12](https://github.com/docspec/docspec/issues/12)
* **cli:** scaffold CLI, improve I/O handling ([0108fe8](https://github.com/docspec/docspec/commit/0108fe87409662e3c5e127bcd4b7a5790e8cd8e3)), closes [#16](https://github.com/docspec/docspec/issues/16)
* **core:** add code, strikethrough, underline text formatting support ([d0a768d](https://github.com/docspec/docspec/commit/d0a768d9edafb944043b471ce78721d4c90bb164))
* **core:** add Event::SoftBreak variant ([be8b485](https://github.com/docspec/docspec/commit/be8b48527368440986934b28df7a4d1e414bc322)), closes [#12](https://github.com/docspec/docspec/issues/12)
* **core:** add StackTrackingSink for event stream normalization ([0555b0a](https://github.com/docspec/docspec/commit/0555b0aab7d278be5152b542fe2bef5905bb8c3a))
* **json:** extract JSON writing primitives to docspec-json crate ([8b51f24](https://github.com/docspec/docspec/commit/8b51f24fda81605db46335b49b823ac871f55d92))
* **markdown-reader,blocknote-writer:** emit and serialize inline links ([9592073](https://github.com/docspec/docspec/commit/9592073313c627257cfc7fd91ad978b3267e24a6))
* **markdown-reader,blocknote-writer:** Markdown reader and BlockNote writer ([8e56b1b](https://github.com/docspec/docspec/commit/8e56b1b2d70633ddc29732d48228f2cb22f24db6))
* **markdown-reader,blocknote-writer:** support block quotes and thematic break dividers ([91f7246](https://github.com/docspec/docspec/commit/91f7246a4e7fbfa97519317d9a4891f679965c9c))
* **markdown-reader:** emit ordered and unordered list events ([7ffec48](https://github.com/docspec/docspec/commit/7ffec480eca1a54cecc5ef20e2ec2abcc0b88664))


### Bug Fixes

* **blocknote-writer:** blockquote text no longer lost to separate paragraph ([5bde87f](https://github.com/docspec/docspec/commit/5bde87fb6bbfaaef5f68813612f7fc4bcb90cc0a)), closes [#12](https://github.com/docspec/docspec/issues/12)
* **blocknote-writer:** handle image inside heading without panic ([7c9d11c](https://github.com/docspec/docspec/commit/7c9d11c6a501d184930853d6804829aa9a602260)), closes [#12](https://github.com/docspec/docspec/issues/12)
* **blocknote-writer:** use double newline for paragraph separation in quotes ([b6ceae0](https://github.com/docspec/docspec/commit/b6ceae086e2411462c09c5f0d3b508a74d39a001)), closes [#12](https://github.com/docspec/docspec/issues/12)
* **core:** add id's to every event type ([d14a5a6](https://github.com/docspec/docspec/commit/d14a5a6f84a88445e18acd0bfdccfb71991df2d4)), closes [#10](https://github.com/docspec/docspec/issues/10)
* **core:** validate EndDocument and remove Blockquote from content-bearing ([e1377ff](https://github.com/docspec/docspec/commit/e1377ff810a6b67d471add2bae8a3d58d609da9d)), closes [#10](https://github.com/docspec/docspec/issues/10)
* **core:** validate single StartDocument in StackTrackingSink ([ab26465](https://github.com/docspec/docspec/commit/ab2646530ea0593be1f3ff67d5528fa866269443))
* dependency cycle ([7315fd0](https://github.com/docspec/docspec/commit/7315fd0f8912941c7a69a7d5e8065ba258d1f584))
* **markdown-reader:** buffer code block text for proper newline stripping ([718b4c3](https://github.com/docspec/docspec/commit/718b4c3c37c634835a2b2a1826a02814c12f32ec))

## [0.2.2](https://github.com/docspec/docspec/compare/v0.2.1...v0.2.2) (2026-06-01)

## [0.2.1](https://github.com/docspec/docspec/compare/v0.2.0...v0.2.1) (2026-06-01)

## [0.2.0](https://github.com/docspec/docspec/compare/v0.1.0...v0.2.0) (2026-06-01)


### Features

* add code, strikethrough, underline text formatting support ([3aa6b50](https://github.com/docspec/docspec/commit/3aa6b50cfc3d0ea6dceae1387a320e65c0d2d4a4))
* **blocknote-writer:** emit native BlockNote table blocks ([3ac3fb1](https://github.com/docspec/docspec/commit/3ac3fb13bf2c2578329845748e27c6c42d96e307)), closes [#12](https://github.com/docspec/docspec/issues/12)
* **blocknote-writer:** list support with nesting ([3dce1e0](https://github.com/docspec/docspec/commit/3dce1e0812eb479b0944845cb12ccadb41ebd994)), closes [#12](https://github.com/docspec/docspec/issues/12)
* **blocknote:** preformatted blocks / codeBlocks ([#12](https://github.com/docspec/docspec/issues/12)) ([6f09cc5](https://github.com/docspec/docspec/commit/6f09cc58f706d2a47686a31e41df1c70e65823bc))
* **cli:** scaffold CLI, improve I/O handling ([2c52ef2](https://github.com/docspec/docspec/commit/2c52ef2ad94eccfcb1a991bc83877ce62faf9285))
* **core:** add Event::SoftBreak variant ([5d7e408](https://github.com/docspec/docspec/commit/5d7e40813a0de18eec58f4893032e3babedc9812))
* **core:** add StackTrackingSink for event stream normalization ([#12](https://github.com/docspec/docspec/issues/12), [#14](https://github.com/docspec/docspec/issues/14), [#16](https://github.com/docspec/docspec/issues/16)) ([2cd6c5c](https://github.com/docspec/docspec/commit/2cd6c5c9143a54b5fcb268ae82ba008ebe3338ce))
* **json:** extract JSON writing primitives to docspec-json crate ([37a2da8](https://github.com/docspec/docspec/commit/37a2da8c6436899cee5d6b45d5a39e153a554ca9))
* Markdown reader and BlockNote writer ([#39](https://github.com/docspec/docspec/issues/39), [#12](https://github.com/docspec/docspec/issues/12), [#10](https://github.com/docspec/docspec/issues/10), [#13](https://github.com/docspec/docspec/issues/13)) ([90c27d3](https://github.com/docspec/docspec/commit/90c27d3689fa99d0bfa1ea59c9383ae9bf754f29))
* **markdown-reader,blocknote-writer:** emit and serialize inline links ([7eea150](https://github.com/docspec/docspec/commit/7eea1500220cff01176d64153a00c0364376937c))
* **markdown-reader:** emit ordered and unordered list events ([2185ca0](https://github.com/docspec/docspec/commit/2185ca001a5dfc10830938519fea188a8703f4f5)), closes [#37](https://github.com/docspec/docspec/issues/37) [#10](https://github.com/docspec/docspec/issues/10)
* **markdown,blocknote:** support block quotes and thematic break dividers ([#10](https://github.com/docspec/docspec/issues/10), [#37](https://github.com/docspec/docspec/issues/37), [#12](https://github.com/docspec/docspec/issues/12)) ([275c1e0](https://github.com/docspec/docspec/commit/275c1e07170651b7519ecb9122542ad8551753b4))


### Bug Fixes

* add id's to every event type ([#10](https://github.com/docspec/docspec/issues/10)) ([84b614a](https://github.com/docspec/docspec/commit/84b614af0b1ff925a7e339eef2a1f9c5ecc94fc7))
* **blocknote-writer:** handle image inside heading without panic ([34ef6d9](https://github.com/docspec/docspec/commit/34ef6d9b09b84b5239c69c538eb2aadf33cceeb2)), closes [#12](https://github.com/docspec/docspec/issues/12)
* **blocknote:** blockquote text no longer lost to separate paragraph ([#12](https://github.com/docspec/docspec/issues/12)) ([a753e97](https://github.com/docspec/docspec/commit/a753e9715592d7b4e8a52c924ef84dc262c74206))
* **blocknote:** use double newline for paragraph separation in quotes ([#12](https://github.com/docspec/docspec/issues/12)) ([fbeb44d](https://github.com/docspec/docspec/commit/fbeb44d5fb600219d8556b6446d4e396c53fc032))
* **core:** validate EndDocument and remove Blockquote from content-bearing ([#10](https://github.com/docspec/docspec/issues/10)) ([d8eb71a](https://github.com/docspec/docspec/commit/d8eb71a534e82feb090adf47614c025194c04e59))
* **core:** validate single StartDocument in StackTrackingSink ([#10](https://github.com/docspec/docspec/issues/10), [#12](https://github.com/docspec/docspec/issues/12)) ([2853b14](https://github.com/docspec/docspec/commit/2853b1431142b7924f1c65c49927e73a520896ce))
* dependency cycle ([363a4c5](https://github.com/docspec/docspec/commit/363a4c511aa41167b864fa46816a957e57e3b4bb))
* **reader:** buffer code block text for proper newline stripping ([#37](https://github.com/docspec/docspec/issues/37), [#12](https://github.com/docspec/docspec/issues/12)) ([76c0e4c](https://github.com/docspec/docspec/commit/76c0e4c977a6913d01bc029891a5cc9721d9c516))
