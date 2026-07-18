# Fearless Refactor Status

Snapshot: 2026-07-17

The original fearless-refactor workstream is historical context.

| Concern | Current authority |
| --- | --- |
| Architecture ownership | `docs/adr/0073-family-owned-diagram-architecture.md` |
| Implementation | `docs/plans/2026-07-14-001-refactor-family-owned-architecture-plan.md` |
| Parity policy and status | `docs/workstreams/PARITY_BOUNDARY.md` and `docs/alignment/STATUS.md` |

Architecture completion does not imply that the latest worktree passes every parity gate. ER,
Mindmap, and State root parity are still converging; the alignment dashboard owns the current
verification read.

## Current State

- Successful parsing constructs family semantics once; compatibility JSON, typed rendering, and
  editor facts are projections.
- Built-in rendering uses the canonical typed headless operation and family-owned artifacts.
- Render dependencies, including host/system text measurement, are selected by the operation-owned
  render environment.
- Every family emits its SVG root through the shared Root Viewport module.
- Generated root tables, complete-label text tables, fixture-keyed calibration branches, and their
  generation/audit commands have been removed.
- Browser-probed vendored font profiles contain reusable glyph, kerning, scale, and endpoint facts;
  fixtures are transient probe inputs and verification evidence, never runtime keys.

## Active Rules

- Fix parser, semantic model, layout, DOM structure, or root-bound ownership at its source.
- Use a host text measurer when exact installed-font behavior is required.
- Treat browser-only differences as documented residuals when no source-backed headless model can
  derive them robustly.
- Do not add literal-label, fixture-id, topology-signature, or old-viewBox branches to production.
- Run focused family tests first, then workspace, clippy, strict verification, and parity gates.

## Historical Documents

`MILESTONES.md`, `TODO.md`, `CHANGELOG.md`, and `COMPLETION_AUDIT.md` describe earlier states. They
may mention mechanisms that no longer exist and are retained as historical evidence, not current
implementation guidance.
