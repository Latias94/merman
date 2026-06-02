# HPD-080 - Mermaid 11.15 Extended Theme Surface

Date: 2026-06-02
Task: HPD-080 visible rendering defect triage

## External Signal

Zed PR `zed-industries/zed#57967` upgraded its markdown Mermaid renderer from merman `0.4` to
`0.6` and moved to `SvgPipeline::resvg_safe()`. The review feedback reported changed background
and text colors after the upgrade, and the follow-up color cleanup commit kept Zed's existing
markdown-preview appearance by rewriting host-specific colors.

That remains a host policy signal rather than evidence that merman should inject Zed palette rules
by default. The generic reusable contract from that PR is fallback integration: hosts need to
identify fallback groups and may want duplicate native/fallback text cleanup.

## Source Evidence

- `repo-ref/mermaid/packages/mermaid/src/themes/index.js` registers `neo`, `neo-dark`, `redux`,
  `redux-dark`, `redux-color`, and `redux-dark-color` alongside `default`, `base`, `dark`,
  `forest`, and `neutral`.
- `repo-ref/mermaid/packages/mermaid/src/config.type.ts` includes the same names in the public
  `MermaidConfig.theme` union.
- `tools/mermaid-cli/node_modules/mermaid/dist/config.type.d.ts` from the installed Mermaid
  `11.15.0` baseline confirms the same public theme union.
- `theme: null` appears in the config type but is not registered in `themes/index.js`, so it is not
  treated as a supported named theme.

## Finding

The earlier theme-parity lane treated the `neo/redux*` names as snapshot-only variants. That was too
narrow: they are official Mermaid 11.15 config themes and should be exposed through the same public
surface as `default`, `base`, `dark`, `forest`, and `neutral`.

The correction should still avoid pretending to have browser-identical theme calculation. Default
values can be loaded from the generated upstream `theme_variables_11_15_0.json` snapshot, while
exact source-equivalent derived override behavior for extended themes remains a follow-up audit.

## Change

- Core supported theme names now include all 11 Mermaid 11.15 names.
- Extended `neo/redux*` defaults expand from the generated 11.15 theme-variable snapshot.
- Explicit direct `themeVariables` overrides continue to win.
- Unknown theme names still fall back to Mermaid's default theme behavior.
- `@merman/web` and bindings expose the same theme name surface as core.

## Verification

- `cargo fmt -p merman-core -p merman-bindings-core -p merman`
- `cargo test -p merman-core theme`
- `cargo test -p merman-bindings-core supported_themes_exposes_core_theme_surface`
- `cargo test -p merman external_ --features render`
- `npm run build:ts --prefix platforms/web`

## Residual

- Exact `neo/redux*` override derivation is still less deep than the source-backed expansion for
  `default`, `base`, `dark`, `forest`, and `neutral`.
- The Rust API gives hosts enough postprocessor composition for Zed-like integrations, including
  optional duplicate fallback cleanup. The FFI/options JSON surface still only exposes the pipeline
  preset, so non-Rust consumers do not yet have first-class toggles for
  `DropNativeDuplicateFallbacksPostprocessor`, scoped host CSS, or `!important` cleanup policy.
