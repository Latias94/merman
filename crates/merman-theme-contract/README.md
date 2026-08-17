# merman-theme-contract

`merman-theme-contract` is the pre-freeze owner for Merman's candidate theme authoring version
registry, version 1 authoring wire, and canonical JSON implementation. It is intentionally
independent from the parser, renderer, bindings, and host runtime crates.

The wire is closed at every object boundary. Persisted definitions must carry the exact supported
`authoring_schema_version` and `expansion_version` tuple, must contain a non-null `tokens` object,
and may omit `styles` to select an empty authored rule set. Optional token fields retain omission;
JSON `null` is rejected outside clearable style-patch facets. The crate does not apply defaults,
expand tokens, validate renderer semantics, or decode the wire into renderer types.

This crate is configured as a publishable pre-freeze alpha contract and makes no cross-version
compatibility promise before the C7a contract gate.

## Authoring wire

Rust callers can construct the current ephemeral envelope without spelling version numbers:

```rust
use merman_theme_contract::{ThemeDefinitionV1, ThemeTokensV1};

let definition = ThemeDefinitionV1::new(
    ThemeTokensV1::default()
        .with_canvas("#0f172a")
        .with_surface("#111827")
        .with_text("#e5e7eb")
        .with_border("#475569")
        .with_accent("#60a5fa"),
);
assert_eq!(definition.authoring_schema_version(), 1);
assert_eq!(definition.expansion_version(), 1);
```

Serde decoding remains strict. Call it directly only for trusted or externally size-limited JSON;
untrusted transport entry points must apply their encoded-byte and collection ceilings first:

```rust
use merman_theme_contract::ThemeDefinitionV1;

let definition: ThemeDefinitionV1 = serde_json::from_str(
    r#"{"authoring_schema_version":1,"expansion_version":1,"tokens":{}}"#,
)?;
assert!(definition.styles().is_empty());
# Ok::<(), serde_json::Error>(())
```

Readable JSON, canonical JSON, Rust construction, and generated SDK construction are projections of
the same `ThemeDefinitionV1` value. A theme can be shared as one ordinary JSON file without a
manifest, package, registry, or installation step:

```rust
use merman_theme_contract::{ThemeDefinitionV1, ThemeTokensV1};

let typed = ThemeDefinitionV1::new(
    ThemeTokensV1::default()
        .with_canvas("#0f172a")
        .with_text("#e5e7eb")
        .with_accent("#60a5fa"),
);
let readable = serde_json::to_string_pretty(&typed)?;
let decoded: ThemeDefinitionV1 = serde_json::from_str(&readable)?;
assert_eq!(typed.canonical_json_bytes()?, decoded.canonical_json_bytes()?);
# Ok::<(), Box<dyn std::error::Error>>(())
```

`ThemeRuleSetWireV1` preserves the existing flat `kind: "rule" | "ordinal-palette"` JSON shape.
Clearable facets use `SpecifiedWireV1`: omission is `Unspecified`, JSON `null` is `Clear`, and a
non-null payload is `Value`. Semantic target IDs, colors, numeric domains, empty palettes, and
effect references are intentionally left for the materializer to validate.

## Canonical JSON

Canonical serialization is available only from a contract-owned typed definition:

```rust
use merman_theme_contract::{ThemeDefinitionV1, ThemeTokensV1};

let bytes = ThemeDefinitionV1::new(ThemeTokensV1::default()).canonical_json_bytes()?;
assert_eq!(
    bytes,
    br#"{"authoring_schema_version":1,"expansion_version":1,"tokens":{}}"#
);
# Ok::<(), merman_theme_contract::CanonicalJsonError>(())
```

The encoder applies RFC 8785 property ordering and number serialization, including negative-zero
normalization. Rust-constructed non-finite numbers fail closed. There is deliberately no public
canonicalizer for arbitrary `Serialize` values.

## Version resolution

Resolve the complete tuple rather than accepting authoring and expansion versions independently:

```rust
use merman_theme_contract::resolve_authoring_version;

let version = resolve_authoring_version(1, 1)?;
assert_eq!(version.spec_schema_version(), 1);
# Ok::<(), merman_theme_contract::ThemeContractVersionError>(())
```

This crate has no default Cargo features.
