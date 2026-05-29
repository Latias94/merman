# ASCII Graph Junction Routing - Milestones

Status: Active
Last updated: 2026-05-29

## M0 - Scope And Evidence Freeze

Exit criteria:

- Workstream docs exist and agree on scope.
- The first implementation task is bounded and executable.

## M1 - Graph Module Split

Exit criteria:

- `graph/mod.rs` is reduced to module wiring and render entrypoint.
- Charset/layout/drawing/routing responsibilities live in separate private modules.
- Existing graph fixture allowlist remains green.

## M2 - Junction Merge Routing

Exit criteria:

- Shared LR edge routes merge compatible line segments instead of blindly overwriting them.
- At least one named crossing fixture moves to the exact allowlist, or evidence explains why it was split.
- No existing exact fixture regresses.

## M3 - Closeout

Exit criteria:

- Focused and broad Rust gates pass with fresh evidence.
- Docs and gap inventory reflect the final exact count.
- Lane is committed or explicitly handed off.
