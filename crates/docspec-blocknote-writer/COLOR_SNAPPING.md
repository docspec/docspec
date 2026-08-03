# Highlight Color: Known Name Divergences

`Mark(Color)` events arriving from any upstream reader (e.g. OOXML `<w:highlight>`, OOXML `<w:shd w:fill>`, HTML `<mark>`) are snapped to BlockNote's 9-entry pastel background palette using squared Euclidean distance in 8-bit sRGB (`src/palette.rs`). The resulting palette name is emitted as the `backgroundColor` key in the inline `styles` object.

When the source carries a *named* color from a different palette, the snap can pick a palette entry whose **name** differs from the source name, even though both palettes share many of the same names. This is deterministic, asserted by tests, and **semantically lossy**: a span highlighted "yellow" in the source can render under a different name in BlockNote.

## Example case: OOXML `yellow` → BlockNote `orange`

OOXML `<w:highlight w:val="yellow"/>` is parsed by the DOCX reader as `Color::Rgb { r: 255, g: 255, b: 0 }` and arrives at this writer as `Mark((255, 255, 0))`.

The two relevant palette entries are:

- `"yellow"` → `(251, 243, 219)`
- `"orange"` → `(246, 233, 217)`

```
distance((255,255,0) → (251,243,219)) = 4² + 12² + 219² = 48121
distance((255,255,0) → (246,233,217)) = 9² + 22² + 217² = 47654   ← shorter
```

`"orange"` wins by 467 squared units. Every yellow highlight in the source is emitted as `"backgroundColor":"orange"`. This is asserted by `palette::tests::r255_g255_b0_matches_orange` in `src/palette.rs`.

## Mechanism

Every entry in `BACKGROUND_PALETTE` has all three channels in the range 217-251 (pastel). Many source-palette colors place at least one channel at 0 or 255, far outside that anchor cluster. The squared-distance sum is then dominated by whichever channels are most extreme, and the snap is decided by small differences in the remaining channels. Those small differences do not track perceptual hue or color name, so the "nearest" pastel under this metric is frequently a *neighbouring* color rather than the same color in pastel form.

The same metric applies to text color (`TextColor` → `textColor`) against `TEXT_PALETTE`. The text palette uses richer tones and a different anchor distribution, so the specific divergences differ.
