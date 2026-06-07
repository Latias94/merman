# HPD-050 - SVG Attribute Sanitize Regex Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

`SanitizeSvgAttributesPostprocessor` compiled the same static regex in two places:

```rust
Regex::new(r#"\s+([A-Za-z_:][-A-Za-z0-9_:.]*)\s*=\s*"([^"]*)""#)
```

The first use rewrote or removed matching double-quoted attributes across a tag. The second use
read `width` / `height` for the bad-`rect` guard before attribute rewriting.

## Changes

- Removed `regex::Regex` and `OnceLock` from
  `crates/merman-render/src/svg/pipeline/builtin/attr_sanitize.rs`.
- Added a shared `SvgAttrMatch` scanner for the previous local regex shape: leading whitespace,
  ASCII SVG-like attribute names, optional whitespace around `=`, and double-quoted values.
- Routed both `sanitize_tag_attributes(...)` and `attr_value(...)` through the shared scanner.
- Preserved current behavior for single-quoted, unquoted, or otherwise non-matching attributes by
  leaving them outside this local cleanup rule.
- Added focused tests for unchanged attribute formatting, px normalization, empty guarded
  attribute dropping, style sanitization, and bad-`rect` detection with spaced uppercase
  attributes.

## Verification

- `cargo +1.95 fmt -p merman-render` - passed.
- `cargo +1.95 nextest run -p merman-render attr_sanitize resvg_safe` - passed, `6` tests run.
- `cargo +1.95 fmt --check -p merman-render` - passed.
- `rg -n 'Regex|regex::|OnceLock' crates/merman-render/src/svg/pipeline/builtin/attr_sanitize.rs crates/merman-render/src/svg/pipeline/builtin/css_sanitize.rs crates/merman-render/src/svg/pipeline/builtin/css_override.rs` -
  no regex dependency matches in those builtin SVG sanitizer files.
- `rg -n "regex::Regex|Regex::new|OnceLock<regex::Regex>|OnceLock\s*<\s*Regex|regex::Captures|Captures<'" crates/merman-render/src -g '*.rs'` -
  reports only `crates/merman-render/src/svg/parity/er.rs`.
- `git diff --check` - passed.
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check - passed.

## Boundary

This is a local SVG pipeline panic-surface cleanup for raster-safe attribute sanitization. It does
not change guarded attribute policy, invalid value policy, style declaration filtering, CSS
override policy, scoped CSS injection, core parsing, sanitizer policy, SVG baselines, root
viewport formulas, or Architecture residual classification. After this slice, the remaining
precise `regex::Regex` / `Regex::new` render hit is the ER parity decimal-normalization helper in
`crates/merman-render/src/svg/parity/er.rs`.
