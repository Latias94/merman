# HPD-080 - Binding Scoped CSS Options

Date: 2026-06-02
Task: HPD-080 Visible rendering defect triage / host theme integration

## Context

After `options_json.site_config`, non-Rust hosts could pass Mermaid-owned `theme`,
`themeVariables`, diagram config, and `themeCSS`. The remaining generic host-theme gap was
host-owned palette CSS: Rust consumers can compose `ScopedCssPostprocessor`, but binding consumers
were still expected to postprocess the SVG string manually.

## Pipeline Decision

`SvgPipeline` applies its preset before custom postprocessors. Therefore, appending host CSS to a
`resvg-safe` pipeline after the preset would bypass the preset's CSS sanitizer unless the binding
explicitly sanitizes again.

The binding implementation now uses this order:

1. built-in preset (`parity`, `readable`, or `resvg-safe`),
2. optional duplicate fallback cleanup,
3. optional host `svg.scoped_css` injection,
4. optional `SanitizeCssPostprocessor` after host CSS when the preset is `resvg-safe`.

## Implementation

- Added `svg.scoped_css` and `svg.css_override_policy` to shared binding options.
- Mapped `svg.scoped_css` to `ScopedCssPostprocessor`, preserving normal host cascade order by
  injecting after Mermaid CSS.
- Accepted `css_override_policy` values:
  - `preserve`,
  - `strip-existing-important`,
  - `strip_existing_important`.
- Returned `MERMAN_INVALID_ARGUMENT` for unsupported policy values.
- Updated `@merman/web` typed options and binding/rendering docs.

## Boundary

This is an explicit host-owned CSS option. It does not add Zed-specific palette defaults, does not
strip root backgrounds, and does not change default Mermaid parity output. Hosts still own CSS trust
and product policy.

## Verification

- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo test -p merman-bindings-core scoped_css --lib`
- `cargo fmt -p merman-bindings-core`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo test -p merman-bindings-core --lib`
- `npm run build:ts --prefix platforms/web`
- JSONL validation for `CONTEXT.jsonl`, `TASKS.jsonl`, and `CAMPAIGNS.jsonl`
- `git diff --check`
