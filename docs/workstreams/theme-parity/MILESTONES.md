# Theme Parity Refactor - Milestones

Status: Complete
Last updated: 2026-06-02

## M0 - Scope Freeze

Exit criteria:

- Workstream artifacts exist and agree.
- Mermaid source comparison is captured in DESIGN.md.
- First executable task is ready.

## M1 - Core Theme Expansion

Exit criteria:

- `theme: default` populates common Mermaid theme variables in core.
- User overrides keep precedence.
- Existing `base/dark/forest/neutral` assertions still pass.
- Narrow core nextest gate passes.

## M2 - Render Resolver Cleanup

Exit criteria:

- Class, block, and flowchart SVG CSS read common theme values through a shared resolver.
- Redundant fallback code covered by core defaults is removed.
- Text color remains scoped inside SVG output under hostile host-page CSS.

## M3 - API And Playground Theme Surface

Exit criteria:

- WASM, TypeScript wrapper, playground store, toolbar, history, share links, and Mermaid compare mode
  agree on the supported theme list.
- Unknown theme values degrade to `default`.
- Frontend build gates pass.

## M4 - Follow-Up Split

Exit criteria:

- Broad theme fixture expansion is explicitly split from this lane.
- Remaining diagram-specific resolver migration is documented as follow-up work.
- Exact `neo/redux` override derivation is deferred into a separate audit.

## M5 - Closeout

Exit criteria:

- CHANGELOG records theme refactor and user-visible theme behavior changes.
- Exact `neo/redux` override derivation is either deferred explicitly or moved to a follow-on
  workstream.
- Final verification evidence is recorded.

## M6 - Post-11.15 Theme Surface Hardening

Exit criteria:

- Core, bindings, WASM, `@merman/web`, playground, and compare mode expose only Mermaid 11.15
  config themes:
  `default/base/dark/forest/neutral/neo/neo-dark/redux/redux-dark/redux-color/redux-dark-color`.
- Unknown theme names fall back to default.
- Flowchart neutral `edgeLabelBackground: white` produces Mermaid-compatible white label
  backgrounds.
- Representative ordinary-source/theme-selector parity coverage exists before broadening fixture
  claims; current coverage uses high-level `HeadlessRenderer::with_site_config` tests for plain
  source external themes.
- Remaining diagram resolver migrations are evaluated after fixture evidence, not before it.
