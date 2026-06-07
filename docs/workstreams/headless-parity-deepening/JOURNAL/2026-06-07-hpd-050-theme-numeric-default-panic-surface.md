# HPD-050 - Theme Numeric Default Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

Base theme Radar defaults used two fixed finite numbers through:

```rust
serde_json::Number::from_f64(...).unwrap()
```

The values are constants and therefore not expected to fail today, but this was still a production
panic-bearing construction site in the public theme/config path. During the same triage pass, the
COSE-Bilkent horizontal y-force diagnostic `panic!` was confirmed to live in a `#[cfg(test)]`
helper, so it should not be tracked as a production runtime panic candidate.

## Changes

- Added `set_finite_number_if_missing(...)` beside the existing base-theme default helpers.
- Replaced the Radar `curveOpacity` and `graticuleOpacity` unwrap-based number construction with
  the finite-number helper.
- Preserved the same default JSON numbers: `curveOpacity = 0.5` and `graticuleOpacity = 0.3`.
- Removed the stale COSE-Bilkent y-force triage bullet from `docs/quality/PANIC_SURFACE.md`.

## Verification

- `cargo +1.95 fmt -p merman-core` - passed.
- `cargo +1.95 nextest run -p merman-core theme` - passed, `23` tests run.
- `rg -n 'from_f64\(0\.5\)\.unwrap\(\)|from_f64\(0\.3\)\.unwrap\(\)' crates/merman-core/src/theme.rs` -
  no matches.
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check - passed, `875`
  lines parsed.
- `git diff --check` - passed.

## Boundary

This is a local theme-default panic-surface cleanup. It does not change theme derivation, supported
theme names, retained config projection, Radar parser behavior, render layout, SVG baselines, root
viewport formulas, or Mermaid parity residual classification.
