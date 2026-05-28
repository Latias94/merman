# ASCII Renderer Compatibility Expansion - Milestones

Status: Complete
Last updated: 2026-05-28

## M0 - Scope And Compatibility Policy

Exit criteria:

- The lane has an explicit terminal approximation policy.
- Support matrix distinguishes supported, approximated, and still-unsupported features.
- `WORKSTREAM.json` points to the authoritative docs.

## M1 - Flowchart Edge Semantics

Exit criteria:

- Edge labels render visibly for simple LR/TD edges.
- Common open, dotted, and length-modified edges no longer fail when they can be represented
  honestly in terminal output.
- Unsupported edge variants still produce structured diagnostics.

Focused gates:

- `cargo nextest run -p merman-ascii flowchart`
- `cargo nextest run -p merman-ascii graph::`

## M2 - Flowchart Shape Approximations

Exit criteria:

- High-frequency non-rectangular shapes render deterministically.
- Shape mappings are documented as terminal approximations.
- Existing rectangular output remains stable unless intentionally updated.

Focused gates:

- `cargo nextest run -p merman-ascii flowchart`
- `cargo nextest run -p merman-ascii graph::`

## M3 - Flowchart Subgraphs

Exit criteria:

- Simple subgraphs render as titled group boxes.
- Containment is visible and deterministic for supported LR/TD diagrams.
- Nested or complex routing cases are either covered or explicitly split as follow-ons.

Focused gates:

- `cargo nextest run -p merman-ascii flowchart`
- `cargo nextest run -p merman-ascii graph::`

## M4 - Product Examples And CLI Smoke

Exit criteria:

- README examples show expanded flowchart support.
- CLI ASCII smoke coverage exercises one expanded flowchart case.
- Existing CLI format defaults and raster/SVG behavior remain unchanged.

Focused gates:

- `cargo nextest run -p merman-cli --features ascii ascii`
- `cargo check -p merman-cli --features ascii`

## M5 - Verification And Closeout

Exit criteria:

- Focused and broader gates are recorded in `EVIDENCE_AND_GATES.md`.
- Remaining unsupported constructs are documented as follow-ons.
- `WORKSTREAM.json` is marked complete or points to the next pending task.

Status: Done. Closeout gates passed on 2026-05-28 and `WORKSTREAM.json` is complete.
