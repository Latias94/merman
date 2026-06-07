# HPD-050 - CSS Important Regex Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

`CssOverridePostprocessor` compiled a static regex on the public SVG postprocessing path:

```rust
Regex::new(r"(?i)\s*!important\b")
```

The helper is local to `merman-render`: it strips existing `!important` declarations before
injecting caller-provided CSS overrides, and is also reused by scoped CSS injection.

## Changes

- Removed `regex::Regex` and `OnceLock` from
  `crates/merman-render/src/svg/pipeline/builtin/css_override.rs`.
- Replaced regex replacement with a direct scanner anchored on `!`, removing contiguous whitespace
  immediately before case-insensitive `!important`.
- Preserved the previous word-boundary behavior after `important`, so `!importantfoo` and
  `!importanté` remain untouched while `!important-border` strips the marker and leaves `-border`.
- Kept the existing `CssOverridePolicy::Preserve` behavior unchanged.
- Added focused tests for uppercase markers, tab whitespace, word-boundary non-matches, and
  hyphen-boundary removal.

## Verification

- `cargo +1.95 fmt -p merman-render` - passed.
- `cargo +1.95 nextest run -p merman-render important` - passed, `3` tests run.
- `rg -n 'Regex|regex::|OnceLock|css_important|strip_css_important' crates/merman-render/src/svg/pipeline/builtin/css_override.rs crates/merman-render/src/svg/pipeline/builtin/scoped_css.rs` -
  no regex dependency matches in `css_override.rs`; scanner and call sites were the only relevant
  hits.
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check - passed, `846`
  lines parsed.

## Boundary

This is a local SVG pipeline panic-surface cleanup. It does not change CSS override policy
selection, scoped CSS injection syntax, SVG baseline content outside existing-important stripping,
core parsing, sanitizer policy, root viewport formulas, or Architecture residual classification.
