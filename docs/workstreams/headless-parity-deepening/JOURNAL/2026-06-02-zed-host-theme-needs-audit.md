# HPD-080 - Zed Host Theme Needs Audit

Date: 2026-06-02
Task: HPD-080 visible rendering defect triage

## External Signal

- Zed PR: <https://github.com/zed-industries/zed/pull/57967>
- Reviewer color feedback:
  <https://github.com/zed-industries/zed/pull/57967#issuecomment-4598335939>
- Zed follow-up comment:
  <https://github.com/zed-industries/zed/pull/57967#issuecomment-4599604388>
- Zed `color cleanup` commit:
  `c85f29cd2e78ec8a68b20349606d8298eecf37bb`

## Question

Can the current theme surface support common host needs after the 0.6 `resvg_safe` integration
feedback?

## Finding

- Rust consumers can cover the common host theme workflow today:
  - pass Mermaid site config through `HeadlessRenderer::with_site_config(...)`,
  - use Mermaid `theme`, `themeVariables`, and scoped `themeCSS`,
  - pick `SvgPipeline::readable()` or `SvgPipeline::resvg_safe()`,
  - append `ScopedCssPostprocessor`, `CssOverridePostprocessor`, and
    `DropNativeDuplicateFallbacksPostprocessor` when the host owns extra palette or raster policy.
- Binding and `@merman/web` consumers now have the generic raster-safe controls:
  `svg.pipeline`, `svg.diagram_id`, and `svg.drop_native_duplicate_fallbacks`.
- Binding consumers do not yet have a first-class `site_config` or host-scoped CSS option. They can
  still use diagram directives or postprocess the returned SVG, but that is less ergonomic than the
  Rust API.
- Zed's background, edge-label, and tag-label color cleanup remains host palette policy. It should
  not become default merman output because it would replace Mermaid theme semantics with editor
  visual compatibility rules.
- Root white-background replacement remains an explicit open boundary. It should be handled as a
  separate output-policy or host postprocessor decision, not silently changed in `resvg_safe()`.

## Decision

No renderer behavior change is justified by this audit. The current generic gap is API ergonomics
for non-Rust host theme configuration, not a broken default Mermaid theme implementation.

If this becomes a user-facing integration blocker, the next source-compatible API should be a
deliberate shared options design, probably:

- `config` / `site_config` JSON for Mermaid theme selection and `themeVariables`,
- an explicitly scoped host CSS option with documented cascade and raster-safety constraints,
- no default Zed-like palette replacement.

## Verification

- `gh pr view 57967 --repo zed-industries/zed --comments --json title,url,mergeStateStatus,state,body,files,commits,comments,reviews`
- `gh api repos/zed-industries/zed/commits/c85f29cd2e78ec8a68b20349606d8298eecf37bb --jq ...`
- `cargo nextest run -p merman-core theme`
- `cargo nextest run -p merman-bindings-core supported_themes_exposes_core_theme_surface svg_options_can_drop_native_duplicate_fallbacks`
- `cargo nextest run -p merman --features render external_ render_svg_sync_applies_scoped_theme_css_once`
- `npm run build:ts --prefix platforms/web`

## Residual

- Exact `neo/redux*` source-equivalent override derivation remains a narrow follow-up audit.
- Binding-level host CSS / site-config support needs a security and cascade design before it becomes
  a shared JSON option.
- Do not treat host screenshots with editor palette differences as Mermaid parity failures unless a
  Mermaid config/theme variable is demonstrably ignored by merman.
