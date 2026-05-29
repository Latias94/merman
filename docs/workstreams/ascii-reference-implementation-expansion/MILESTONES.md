# ASCII Reference Implementation Expansion — Milestones

Status: Active
Last updated: 2026-05-29

## M0 — Reference Intake And Provenance

Exit criteria:

- `mermaid-ascii` and `beautiful-mermaid` are both named as reference implementations.
- License copies for any reference source with derived work are tracked.
- Upstream commit pins are recorded outside gitignored `repo-ref/`.
- The no-second-parser boundary is explicit.

Primary evidence:

- `crates/merman-ascii/README.md`
- `crates/merman-ascii/LICENSES/*.txt`
- `tools/upstreams/REPOS.lock.json`

## M1 — Class Diagram ASCII

Exit criteria:

- `RenderSemanticModel::Class` no longer fails for the supported subset.
- Class boxes, members, methods, and core relationship markers have snapshot coverage.
- Unsupported class features return structured diagnostics instead of silent omission.

Primary gates:

- `cargo nextest run -p merman-ascii class`

## M2 — ER Diagram ASCII

Exit criteria:

- `RenderSemanticModel::Er` renders readable entity boxes and common relationships.
- Crow's-foot cardinalities and identifying/non-identifying lines are mapped from typed model data.
- Relationship labels and attributes have focused snapshots.

Primary gates:

- `cargo nextest run -p merman-ascii er`

## M3 — XYChart ASCII

Exit criteria:

- `RenderSemanticModel::XyChart` renders deterministic bars, lines, mixed plots, and horizontal
  orientation for the supported subset.
- Axis scaling behavior is documented and snapshot-tested.
- Plain text output works without color support.

Primary gates:

- `cargo nextest run -p merman-ascii xychart`

## M4 — Flow/State Delta Triage

Exit criteria:

- Any `beautiful-mermaid` graph deltas are classified as port, reject, or defer.
- Shipped deltas have Rust tests and support-matrix updates.
- Parser-only behavior is rejected unless `merman-core` already preserves the semantics.

Primary evidence:

- `crates/merman-ascii/FLOWCHART_SUPPORT.md`
- focused graph tests for shipped deltas

## M5 — Integration And Closeout

Exit criteria:

- Public dispatch and documentation reflect shipped diagram support.
- Fresh focused and broad gates are recorded.
- Remaining work is closed or split into follow-ons.

Primary gates:

- `cargo fmt --all --check`
- `cargo nextest run -p merman-ascii`
- `cargo nextest run -p merman --features ascii`
- `cargo nextest run -p merman-cli --features ascii`
- `git diff --check`
