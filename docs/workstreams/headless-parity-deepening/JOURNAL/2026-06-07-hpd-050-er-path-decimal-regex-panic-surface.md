# HPD-050 - ER Path Decimal Regex Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

`is_label_coordinate_in_path(...)` compiled a cached regex inside the ER SVG renderer:

```rust
regex::Regex::new(r"(\d+\.\d+)")
```

The helper is a local Mermaid-compatibility heuristic. It rounds decimal substrings in an SVG path
`d` attribute, then checks whether a rounded label midpoint coordinate appears in the rounded path
string.

## Changes

- Removed the cached `regex::Regex` / `OnceLock` helper from
  `crates/merman-render/src/svg/parity/er.rs`.
- Replaced decimal replacement with a direct scanner for the local `\d+\.\d+` path substring shape.
- Preserved non-overlapping replacement behavior: signs remain outside the match, `.5` and `10.`
  do not match, and `3.4.5` becomes `3.5` after the first decimal substring is rounded.
- Moved `regex` in `crates/merman-render/Cargo.toml` from normal dependencies to
  `dev-dependencies`, because remaining `merman-render` uses are test-only.

## Verification

- `cargo +1.95 fmt -p merman-render` - passed.
- `cargo +1.95 nextest run -p merman-render er` - passed, `279` tests run.
- `cargo +1.95 nextest run -p merman-render er_label_coordinate_path_decimal_rounding_without_regex` -
  passed, `1` test run after the dependency move.
- `cargo +1.95 nextest run -p merman-render --test er_svg_test` - passed, `7` tests run after
  the dependency move.
- `cargo +1.95 fmt --check -p merman-render` - passed.
- `rg -n 'Regex|regex::|OnceLock' crates/merman-render/src/svg/parity/er.rs` - no regex
  dependency matches in `er.rs`.
- `rg -n "regex::Regex|Regex::new|OnceLock<regex::Regex>|OnceLock\s*<\s*Regex|regex::Captures|Captures<'" crates/merman-core/src crates/merman-render/src -g '*.rs'` -
  no precise production core/render regex compile/cache matches.
- `git diff --check` - passed.
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check - passed.

Non-gating note: `cargo +1.95 nextest run -p merman-core --test snapshots` compiled after a
temporary core dependency move, then failed on the existing
`zed_50558_class_inheritance.mmd` class snapshot mismatch (`+ move()` versus `+move()`). That
core dependency move was reverted and is not part of this slice.

## Boundary

This is a local ER renderer panic-surface cleanup for a path-coordinate heuristic. It does not
change ER layout, relationship routing, label placement fallback semantics, SVG baselines, root
viewport formulas, parser behavior, or sanitizer policy. Test-only regex use remains in integration
tests; production `merman-core/src` and `merman-render/src` now have no precise regex compile/cache
matches.
