# merman-theme-contract

`merman-theme-contract` compiles native Theme v2 JSON into immutable visual settings shared across diagram families. The crate contains the built-in presets and has no diagram parser, renderer, or platform dependency.

```rust
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
```

Use the [Theme v2 JSON Schema](https://merman.dev/schemas/theme-v2.schema.json) to author documents. See the [theme guide](https://github.com/Latias94/merman/blob/main/docs/rendering/custom-diagram-themes.md) for CLI and SDK entry points.
