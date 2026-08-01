# Error Upstream Test Coverage (Mermaid@11.16.0)

Scope: Mermaid tag `@11.16.0`.

## Upstream source coverage

| Upstream behavior | Source | Local evidence |
| --- | --- | --- |
| Registered `error` diagram with a no-op parser | `packages/mermaid/src/diagrams/error/errorDiagram.ts` | `fixtures/error/basic.mmd` and its semantic/layout goldens |
| Fixed `2412 x 512` viewBox and responsive `512px` maximum width | `packages/mermaid/src/diagrams/error/errorRenderer.ts` | All four layout goldens and upstream SVG baselines |
| Six error-icon paths | `packages/mermaid/src/diagrams/error/errorRenderer.ts` | DOM comparison for all four upstream SVG baselines |
| Syntax-error and Mermaid-version labels | `packages/mermaid/src/diagrams/error/errorRenderer.ts` | DOM comparison and public resvg-safe fixture smoke |
| Suppressed legacy State parser failure | `packages/mermaid/src/diagrams/state/stateDiagram.spec.js`, commented arrow-direction sample | `upstream_pkgtests_statediagram_spec_024.mmd` |
| Suppressed State v2 parser failure | `packages/mermaid/src/diagrams/state/stateDiagram-v2.spec.js`, commented arrow-direction sample | `upstream_pkgtests_statediagram_v2_spec_024.mmd` |
| Suppressed unsupported State `if` syntax | `packages/mermaid/src/diagrams/state/stateDiagram.spec.js`, skipped `if` sample | `upstream_pkgtests_statediagram_spec_030.mmd` |

The three State-derived fixtures intentionally preserve the upstream test strings verbatim. They
exercise the host-visible `suppressErrors` transition into the Error family; they are not claimed
as supported State syntax.

## Committed evidence

- Semantic and layout snapshots: `fixtures/error/*.golden.json` and
  `fixtures/error/*.layout.golden.json`.
- Pinned Mermaid SVGs: `fixtures/upstream-svgs/error/*.svg`.
- Complete generated provenance: `fixtures/upstream-svgs/error/_baseline-manifest.json`.
- Public raster-safe coverage file: `crates/merman/tests/resvg_safe_fixture_smoke.rs`.
  Test name: boundary_fixtures_render_headless_resvg_safe.
- Primary matrix adapter: `compare-error-svgs`.

## Verification

```bash
cargo run -p xtask -- update-snapshots --diagram error
cargo run -p xtask -- update-layout-snapshots --diagram error
cargo run -p xtask -- check-upstream-svgs --diagram error
cargo run -p xtask -- compare-error-svgs --check-dom --dom-mode parity
cargo run -p xtask -- compare-error-svgs --check-dom --dom-mode parity-root
cargo run -p xtask -- check-alignment
```
