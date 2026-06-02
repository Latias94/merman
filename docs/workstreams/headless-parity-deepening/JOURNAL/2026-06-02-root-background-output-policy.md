# HPD-080 - Root Background Output Policy

Date: 2026-06-02
Task: HPD-080 visible rendering defect triage

## Question

Zed's merman 0.6 upgrade feedback included root background and text color differences. We needed to
decide whether root `background-color: white` is Mermaid 11.15 source behavior, a local/capture
artifact, or a host policy seam.

## Source / Capture Finding

- Pinned Mermaid 11.15 `packages/mermaid/src/setupGraphViewbox.js` sets `width="100%"` and
  `style="max-width: ...px;"` when `useMaxWidth` is enabled. It does not emit root
  `background-color`.
- Installed Mermaid 11.15 dist under `tools/mermaid-cli/node_modules/mermaid/dist` has the same
  `calculateSvgSizeAttrs(...)` behavior.
- The local upstream SVG capture script in `crates/xtask/src/cmd/generate.rs` defaults
  `input.background_color || 'white'` and calls `ensureSvgBackgroundColor(...)` before writing the
  fixture SVG.
- Local parity SVG renderers intentionally keep `background-color: white` in many root styles so the
  fixture comparison surface remains stable.

## Change

- Added `RootBackgroundPostprocessor`.
- Exposed it through `merman_render::svg` and `merman::render`.
- Added shared binding `options_json.svg.root_background_color`.
- Added `@merman/web` typing for `root_background_color`.
- Updated binding and pipeline docs.

The postprocessor rewrites only the root `<svg>` inline `background-color` or adds one when missing.
It does not rewrite diagram-owned palette CSS or inline node/edge/label colors.

## Decision

Default SVG output should not change. The white root background is not Mermaid source behavior, but
it is part of the current fixture/capture parity surface. Host canvas color is a valid common need,
so it should be explicit opt-in output policy rather than a silent default rewrite.

## Verification

- `git -C repo-ref/mermaid show 41646dfd43ac83f001b03c70605feb036afae46d:packages/mermaid/src/setupGraphViewbox.js`
- `Get-Content tools/mermaid-cli/node_modules/mermaid/dist/chunks/mermaid.core/chunk-CSCIHK7Q.mjs`
  around `calculateSvgSizeAttrs(...)`
- `rg "ensureSvgBackgroundColor|background_color" crates/xtask/src/cmd/generate.rs`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo test -p merman-render root_background --lib`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo test -p merman-bindings-core root_background --lib`
- `npm run build:ts --prefix platforms/web`
- JSONL validation for `CONTEXT.jsonl`, `TASKS.jsonl`, and `CAMPAIGNS.jsonl`
- `git diff --check`

## Residual

- Zed-style node/edge/tag-label palette cleanup remains host-specific CSS or postprocessing.
- Do not remove the root white background from default parity SVGs unless the fixture/capture policy
  changes.
