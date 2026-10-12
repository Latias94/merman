# Test Fonts

Cousine Regular, Bold, and Italic are test-only fixtures under the SIL Open Font
License 1.1 in `OFL.txt`. They are not bundled into production binaries.

Source: Google Fonts, `ofl/cousine`, commit
`62e55e586fc0c29d428ee47b7c359cd33143f93d`.
The unmodified `.ttf` files are used to test explicit font loading, style
selection, reproducible face identities, and text geometry without host fonts.

## Primitive Test Subsets

All derived fixtures below are test-only and are not included in production
binaries. They were generated with fontTools 4.66.0; Unicode subsets retain
layout features and font naming/license records.

| Fixture | Source at the same pinned Google Fonts commit | Derivation | License |
| --- | --- | --- | --- |
| `NotoSans-Latin-subset.ttf` | `ofl/notosans/NotoSans[wdth,wght].ttf` | Instantiate `wght=400 wdth=100`, then subset U+0020, U+0041, U+0056, U+0066, U+0069, U+00A0 | `NotoSans-OFL.txt` |
| `NotoSansSC-subset.ttf` | `ofl/notosanssc/NotoSansSC[wght].ttf` | Subset U+0020, U+4E2D, then instantiate `wght=400` | `NotoSansSC-OFL.txt` |
| `Cousine-test-collection.ttc` | Existing Cousine Regular and Italic fixtures | Subset each to U+0020, U+0041, U+0056, then combine in that order with `fontTools.ttLib.TTCollection` | `OFL.txt` |

Noto Sans tests real pair kerning and trailing nonbreaking-space advance. The
small Simplified Chinese face supplies a genuine CJK fallback glyph absent from
Cousine. The collection exercises face index 1 without relying on host fonts.

Subset commands use `--layout-features=* --name-IDs=* --name-languages=*`.
Variable-font instances use `python -m fontTools.varLib.instancer`; subsets use
`pyftsubset`. Collection construction uses `TTCollection.fonts = [TTFont(regular),
TTFont(italic)]` followed by `save`. No font-building code is part of the runtime.
