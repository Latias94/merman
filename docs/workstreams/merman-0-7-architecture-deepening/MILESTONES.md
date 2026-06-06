# Merman 0.7 Architecture Deepening — Milestones

Status: Closed
Last updated: 2026-06-06

## M0 — Workstream Opened

Exit criteria:

- DESIGN, TODO, TASKS, CAMPAIGNS, gates, context manifest, and handoff exist.
- `CONTEXT.md` records the new lane terms.
- First executable task is ready and bounded.

## M1 — Canonical Render Operation

Exit criteria:

- One behavior-bearing Headless Render Operation module owns parse/layout/SVG/pipeline ordering.
- Existing Rust facade render helpers route through it without public behavior changes.
- Adapter migration plan is backed by tests and evidence.

## M2 — Public Adapter And Raster Adoption

Exit criteria:

- bindings-core, CLI, raster, and FFI-facing code use canonical operations where appropriate.
- Adapter-specific policy remains local to adapters.
- Shallow pre-1.0 convenience methods are deleted, demoted, or explicitly retained with reason.

## M3 — Diagram Family Facts And Admission

Exit criteria:

- Detector/parser/render metadata projections derive from a deeper family facts module.
- Supported diagram metadata is no longer a hand-maintained divergent list.
- Fixture admission has one inventory source for status, skip/defer reason, and gate projection.

Status: Complete on 2026-06-06 via M07A-050 and M07A-060.

## M4 — SVG Root, Viewport, And Theme

Exit criteria:

- Generic root SVG and viewport behavior lives under the SVG parity layer.
- At least the highest-duplication theme/config surfaces consume `PresentationTheme` roles.
- SVG/root/theme changes have targeted compare evidence.

Status: Complete on 2026-06-06. M07A-070 completed the shared root viewport proof with `treeView`;
M07A-080 migrated XyChart and QuadrantChart visible role bundles to `PresentationTheme`.

Adapter determinism side-slice: M07A-077 exposed fixed Gantt/local-time controls through the CLI and
aligned typed render-model parsing with semantic JSON fixed-time behavior. M07A-078 exposed the
same controls through Rust headless renderer facades and shared binding `options_json`.

Flowchart parity side-slice: M07A-079 aligned `nodeSpacing=0` and `rankSpacing=0` with Mermaid's
`|| 50` dagre source behavior while preserving `diagramPadding=0` as a valid explicit value.

## M5 — Typed Semantic Ownership

Exit criteria:

- Engine-level typed semantic sanitization no longer owns family field details.
- Flowchart has one semantic source, or the remaining split has a load-bearing documented reason.
- JSON fallback is fenced as an adapter or preserved with explicit admission evidence.

Status: Complete on 2026-06-06. M07A-090 moved typed common DB sanitization ownership to families,
M07A-100 collapsed Flowchart JSON plus typed render parsing around one internal semantic source,
and M07A-110 fenced JSON render fallback to the built-in `error` diagram plus custom adapters.

## M6 — Closeout

Exit criteria:

- `cargo fmt --all --check` passes.
- Focused package gates for touched crates pass.
- `cargo run -p xtask -- check-alignment` passes.
- SVG parity gate is either full green or narrowed with documented reason matching touched surface.
- Remaining architecture risks are split into follow-on workstreams or documented as accepted
  residuals.

Status: Complete on 2026-06-06 via M07A-120. Workspace tests, alignment, structural SVG parity,
selected root SVG parity, override no-growth, JSON ledger parsing, formatting, and documentation
whitespace gates passed. Full `parity-root` is still a root-only residual diagnostic surface and is
owned by `docs/workstreams/mermaid-11-15-root-viewport-residuals`.
