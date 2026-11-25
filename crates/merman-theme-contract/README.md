# merman-theme-contract

`merman-theme-contract` provides immutable native theme values shared across diagram families. Enable `compile` to compile standalone Theme v2 JSON. Enable `presets` to compile or extend the built-in designs. Both features are off by default.

```rust
# #[cfg(feature = "presets")]
# fn example() -> Result<(), merman_theme_contract::v2::ThemeError> {
use merman_theme_contract::v2::{Appearance, BuiltinPreset, ThemeCompiler};

let compiler = ThemeCompiler::new();
let preset = compiler.compile_preset(BuiltinPreset::Studio, Some(Appearance::Dark))?;
let document = br##"{
  "schema_version": 2,
  "extends": "studio",
  "seed": { "accent": "#0052CC" },
  "form": { "radius": 8 }
}"##;
let theme = compiler.compile_json(document, Some(Appearance::Dark))?;
assert_eq!(theme.form().radius, 8.0);
# Ok::<(), merman_theme_contract::v2::ThemeError>(())
# }
```

Use the [Theme v2 JSON Schema](https://merman.dev/schemas/theme-v2.schema.json) to author documents. See the [theme guide](https://github.com/Latias94/merman/blob/main/docs/rendering/custom-diagram-themes.md) for CLI and SDK entry points.
