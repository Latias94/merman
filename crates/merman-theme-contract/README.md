# merman-theme-contract

`merman-theme-contract` owns Merman's unreleased version 1 theme documents, typed authoring values,
materialized results, and canonical JSON. It has no parser, renderer, bindings, or host dependency.
The first-release schema remains 1 while its unreleased design evolves; there is no legacy decoder.

## Save and share a theme

`ThemeRecipeV1` is the exchange envelope accepted by binding `options.theme`, CLI `--theme-file`,
and Rust `DiagramThemeCompiler::compile_recipe`. `design` retains editable values and bindings;
`complete_spec` carries a concrete specification for precise low-level authoring.

```json
{
  "schema_version": 1,
  "kind": "design",
  "design": {
    "tokens": {
      "brand": { "type": "color", "value": "#60a5fa", "label": "Brand", "group": "Colors" }
    },
    "styles": {
      "node": { "stroke": { "paint": { "kind": "solid", "color": { "ref": "brand" } } } }
    },
    "canvas": { "base": "#0f172a" }
  }
}
```

Each object has a closed wire shape. Unknown fields, mixed payload kinds, unknown versions, and
non-finite numbers are rejected. Omitted collections are empty; explicit `null` is permitted only
at clearable style fields. Direct Serde decoding is appropriate for trusted or externally bounded
input. Transport entry points must enforce byte and collection limits before decoding.

## Author typed values

Direct values require no tokens. A token is an optional named value with a declared type and
editor metadata. Values can reference exact document-local IDs with `{ "ref": "brand" }`; dots
in an ID are literal, and types are never implicitly converted. Colors, numbers, strings, lengths,
font stacks, palettes, paints, strokes, text styles, effects, insets, and line heights are supported.

```rust
use merman_theme_contract::{ThemeDesignTokenV1, ThemeDesignTokenValueV1, ThemeDesignV1, ThemeValueV1};

let mut design = ThemeDesignV1::default();
design.tokens.insert("brand".into(), ThemeDesignTokenV1 {
    value: ThemeDesignTokenValueV1::Color(ThemeValueV1::Literal("#60a5fa".into())),
    label: Some("Brand".into()),
    description: None,
    group: Some("Colors".into()),
});
let readable = serde_json::to_string_pretty(&design)?;
let restored: ThemeDesignV1 = serde_json::from_str(&readable)?;
assert_eq!(design.canonical_json_bytes()?, restored.canonical_json_bytes()?);
# Ok::<(), Box<dyn std::error::Error>>(())
```

`styles` contains semantic defaults keyed by target. Ordered `rules` provide family, variant, or
ordinal exceptions after defaults; `palettes` then supplies ordinal color cycles. Nested paints,
gradient stops, shadows, typography, and canvas layers can retain their own references. Editing a
shared value affects only explicit consumers; equal literal colors remain independent.

The Rust renderer owns bounded reference resolution, cycle detection, consumer domains, and
compilation. This contract crate describes values without duplicating those semantics.

## Materialized results

`MaterializedThemeWireV1` contains the concrete `spec` and resolved version tuple. Its optional
`references` list supplies authored locations and consumer domains for editor controls. These
locations are not execution evidence, portability claims, preset qualification, or render receipts.

```rust
use merman_theme_contract::{DiagramThemeSpecWireV1, MaterializedThemeWireV1};

let result = MaterializedThemeWireV1::try_new(DiagramThemeSpecWireV1::default())?;
assert_eq!(result.schema_version(), 1);
assert_eq!(result.spec_schema_version(), 1);
assert!(result.references().is_empty());
# Ok::<(), merman_theme_contract::CanonicalJsonError>(())
```

## Canonical JSON

Canonical JSON uses RFC 8785 property ordering and number serialization. It is available only for
contract-owned typed values, without a public arbitrary-`Serialize` canonicalizer.

```rust
use merman_theme_contract::ThemeDesignV1;
assert_eq!(ThemeDesignV1::default().canonical_json_bytes()?, b"{}");
# Ok::<(), merman_theme_contract::CanonicalJsonError>(())
```

Saving a document does not embed host fonts or establish support for every diagram family and
output. Use renderer support and execution results for those decisions. Asset redistribution
requirements belong in the accompanying README and LICENSE files.

This crate has no default Cargo features.
