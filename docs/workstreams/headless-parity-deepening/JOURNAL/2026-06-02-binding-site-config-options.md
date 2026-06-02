# HPD-080 - Binding Site Config Options

Date: 2026-06-02
Task: HPD-080 Visible rendering defect triage / host theme integration

## Context

The Zed PR 57967 audit split host theme needs into two categories:

- reusable Mermaid configuration that merman should expose across bindings,
- host-owned palette cleanup that belongs in product code or explicit postprocessors.

Rust already supports the first category through `HeadlessRenderer::with_site_config(...)`, but the
shared binding `options_json` surface did not. Non-Rust hosts had to either embed init directives in
diagram text or postprocess output.

## Implementation

- Added top-level `options_json.site_config` in `crates/merman-bindings-core/src/lib.rs`.
- Validated `site_config` as a JSON object and returned `MERMAN_INVALID_ARGUMENT` for non-object
  values.
- Routed `site_config` through both `HeadlessRenderer::with_site_config(...)` and
  `HeadlessAsciiRenderer::with_site_config(...)`.
- Updated `@merman/web` typed `BindingOptions` with `site_config?: Record<string, unknown>`.
- Updated `docs/bindings/OPTIONS_JSON.md`,
  `docs/rendering/SVG_OUTPUT_PIPELINE.md`, and `THEME_RENDERING_COVERAGE.md`.

## Boundary

This is not a host palette CSS option. `site_config.themeCSS` is Mermaid-owned custom CSS and is
scoped by the parity renderer like the Rust API path. Zed-style background replacement, edge-label
palette cleanup, and `!important` policy still need explicit host postprocessing or a future
security/cascade/raster-safety design.

## Verification

- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo test -p merman-bindings-core site_config --lib`
- `cargo fmt --check -p merman-bindings-core`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo test -p merman-bindings-core --lib`
- `npm run build:ts --prefix platforms/web`
- JSONL validation for `CONTEXT.jsonl`, `TASKS.jsonl`, and `CAMPAIGNS.jsonl`
- `git diff --check`

Note: this shell did not have MSVC `link.exe` on PATH, so the Rust verification used the
toolchain-provided `rust-lld` linker.
