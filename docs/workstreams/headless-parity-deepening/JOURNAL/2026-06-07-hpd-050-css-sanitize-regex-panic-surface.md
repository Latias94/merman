# HPD-050 - CSS Sanitize Regex Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

`SanitizeCssPostprocessor` compiled two static regexes on the SVG pipeline path used by the
`resvg-safe` preset:

```rust
Regex::new(r"(?i)(^|[;{])\s*animation(?:-[a-z-]+)?\s*:[^;}]*;?")
Regex::new(r"(?i)(-?\d+(?:\.\d+)?)deg\b")
```

These helpers are local raster-safety cleanup rules: one removes CSS animation declarations, and
the other strips `deg` units before downstream resvg processing. `attr_sanitize.rs` also calls
`strip_css_deg_units(...)` when cleaning style attributes.

## Changes

- Removed `regex::Regex` and `OnceLock` from
  `crates/merman-render/src/svg/pipeline/builtin/css_sanitize.rs`.
- Replaced animation declaration replacement with a direct scanner that preserves the previous
  delimiter behavior: matches are allowed at string start or after `;` / `{`, the delimiter is kept
  in output, and declarations stop before `}` or after an optional semicolon.
- Replaced CSS degree-unit replacement with a direct scanner that preserves optional negative
  numbers, decimal fractions, case-insensitive `deg`, `.5deg` substring matching, and trailing
  word-boundary behavior.
- Added focused tests for animation suffix boundaries, delimiter preservation, hyphen-followed
  `deg` stripping, and non-ASCII word-boundary non-matches.

## Verification

- `cargo +1.95 fmt --check -p merman-render` - passed.
- `cargo +1.95 nextest run -p merman-render css_sanitize resvg_safe` - passed, `4` tests run.
- `rg -n 'Regex|regex::|OnceLock' crates/merman-render/src/svg/pipeline/builtin/css_sanitize.rs` -
  no regex dependency matches in `css_sanitize.rs`.
- `git diff --check` - passed.
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check - passed.

## Boundary

This is a local SVG pipeline panic-surface cleanup for raster-safe CSS sanitization. It does not
change unsupported-rule filtering, style-element scanning, attribute sanitization, CSS override
policy, scoped CSS injection, core parsing, sanitizer policy, SVG baselines, root viewport
formulas, or Architecture residual classification. The remaining production render regex cluster
is in `crates/merman-render/src/svg/pipeline/builtin/attr_sanitize.rs`.
