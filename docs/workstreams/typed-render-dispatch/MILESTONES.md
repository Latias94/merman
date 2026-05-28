# Typed Render Dispatch - Milestones

Status: Complete
Last updated: 2026-05-28

## M0 - Scope And Evidence Freeze

Status: Done

Exit criteria:

- Problem and non-goals are explicit.
- First bounded task is selected.

Primary evidence:

- `docs/workstreams/typed-render-dispatch/DESIGN.md`
- `docs/workstreams/typed-render-dispatch/TODO.md`

## M1 - Model-Owned Metadata

Status: Done

Exit criteria:

- `RenderSemanticModel` owns canonical kind strings.
- `RenderSemanticModel` owns alias compatibility.
- Focused tests cover canonical and alias diagram types.

Primary gate:

- `cargo nextest run -p merman-core render_semantic_model`

## M2 - Variant-Only Layout Dispatch

Status: Done

Exit criteria:

- `layout_parsed_render_layout_only` validates compatibility once.
- Typed layout dispatch matches on `RenderSemanticModel` variants without repeated alias patterns.
- JSON fallback remains diagram-type based.

Primary gates:

- `cargo nextest run -p merman-render render_model`
- `cargo nextest run -p merman-render`

## M3 - Closeout Or Follow-On

Status: Done

Exit criteria:

- Fresh evidence is recorded.
- Generated dispatch work is either deferred or split.
- `WORKSTREAM.json` status is updated.
