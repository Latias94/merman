# HPD-080 - Host Theme Common Needs Audit

Date: 2026-06-02
Task: HPD-080 visible rendering defect triage

## External Signal

- Zed PR: <https://github.com/zed-industries/zed/pull/57967>
- Reviewer color feedback:
  <https://github.com/zed-industries/zed/pull/57967#issuecomment-4598335939>
- Follow-up color cleanup:
  <https://github.com/zed-industries/zed/pull/57967#issuecomment-4599604388>
- Zed `color cleanup` commit:
  `c85f29cd2e78ec8a68b20349606d8298eecf37bb`

## Finding

Current 0.7 theme integration covers the common product-neutral host needs:

- official Mermaid theme selection through Rust `with_site_config(...)` or binding
  `options_json.site_config`,
- direct `themeVariables` and Mermaid diagram-owned `themeCSS`,
- host-owned scoped CSS through Rust postprocessors or binding `svg.scoped_css`,
- `resvg-safe` output for rasterizers,
- optional duplicate native/fallback label cleanup,
- optional root canvas color replacement.

Zed-style exact palette cleanup remains host policy. It includes editor-specific background,
edge-label, tag-label, and accent rules that should not become default merman output.

## Documentation Tightening

- Added `svg.drop_native_duplicate_fallbacks` to the full binding JSON shape in
  `docs/bindings/OPTIONS_JSON.md`.
- Updated `THEME_RENDERING_COVERAGE.md` to state that arbitrary element or inline-style rewrites are
  a host boundary. Rust hosts can implement a custom `SvgPostprocessor`; shared bindings intentionally
  expose only product-neutral controls.
- Clarified that duplicate fallback cleanup is exact-text based and optional, not a geometric or
  semantic de-duplication oracle.

## Verification

- `gh pr view 57967 --repo zed-industries/zed --comments --json title,url,state,mergeStateStatus,body,files,commits,comments,reviews`
- `gh api repos/zed-industries/zed/commits/c85f29cd2e78ec8a68b20349606d8298eecf37bb --jq '.files[] | {filename,patch}'`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-bindings-core`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-render drop_native_duplicate_fallbacks`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-render root_background`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render external_`

## Residual

- Exact `neo/redux*` override derivation remains a follow-up only if a fixture or consumer proves
  direct overrides plus generated snapshots are insufficient.
- Binding consumers do not get arbitrary XML/attribute rewrite controls. That is intentional until
  a common, product-neutral API shape is justified.
