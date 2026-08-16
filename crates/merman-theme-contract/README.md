# merman-theme-contract

`merman-theme-contract` is the pre-freeze owner for Merman's candidate theme authoring version
registry and canonical JSON implementation. It is intentionally independent from the parser,
renderer, bindings, and host runtime crates.

The crate currently exposes only the candidate authoring-version tuple defined by ADR 0082. Its
RFC 8785 JSON Canonicalization Scheme implementation remains crate-private until contract-owned wire
types can enforce the I-JSON input constraints. This crate is not published and makes no
cross-version compatibility promise before the C7a contract gate.

## Version resolution

Resolve the complete tuple rather than accepting authoring and expansion versions independently:

```rust
use merman_theme_contract::resolve_authoring_version;

let version = resolve_authoring_version(1, 1)?;
assert_eq!(version.spec_schema_version(), 1);
# Ok::<(), merman_theme_contract::ThemeContractVersionError>(())
```

This crate has no default Cargo features.
