# HPD-050 - C4 Detector Regex Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

While validating retained semantic config projection, an exploratory auto-detect small-stack test
overflowed before semantic parsing. The boundary was the detector path, not the semantic model:
simple `block` input had to pass earlier detectors first, including the C4 detector.

The C4 detector used a lazily compiled static regex for a fixed Mermaid source pattern:
`/^\s*C4Context|C4Container|C4Component|C4Dynamic|C4Deployment/`. Compiling that regex on the first
small-stack detection pass is a fixed implementation cost, not user-authored recursion.

## Changes

- Removed the C4 detector's lazy `Regex` / `OnceLock` helper.
- Replaced it with direct string checks that preserve the upstream ungrouped regex shape:
  - `C4Context` matches only after leading whitespace;
  - `C4Container`, `C4Component`, `C4Dynamic`, and `C4Deployment` still match anywhere in the
    cleaned text, because the upstream regex is not grouped.
- Added a semantic guard test for that ungrouped C4 detector behavior.
- Added a small-stack public metadata parsing regression for common headers (`block`, `sankey`,
  `treemap`, and `C4Context`) with a deep host config.

## Verification

- `cargo +1.95 fmt -p merman-core` - passed.
- `cargo +1.95 nextest run -p merman-core c4_detector_preserves_upstream_ungrouped_regex_shape auto_detect_common_headers_with_deep_config_small_stack` -
  passed, `2` tests run.
- `cargo +1.95 nextest run -p merman-core detect` - passed, `17` tests run.
- `cargo +1.95 fmt --check -p merman-core` - passed.
- `git diff --check` - passed.

## Boundary

No detector order, supported-family profile, parser behavior, SVG output, baseline, or root
viewport behavior changed. This slice only removes a fixed lazy-regex initialization point from
the public detection path while preserving the C4 detector's source-backed matching shape.
