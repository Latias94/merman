# Architecture Indexed FCoSE - Milestones

Status: Complete
Last updated: 2026-05-28

## M0 - Scope And Evidence Freeze

Exit criteria:

- Problem and target state are explicit.
- Non-goals are explicit.
- Relevant performance and parity docs are linked.
- First executable task is chosen.

Primary evidence:

- `docs/workstreams/architecture-indexed-fcose/DESIGN.md`
- `docs/workstreams/architecture-indexed-fcose/TODO.md`

## M1 - Indexed FCoSE API

Exit criteria:

- FCoSE exposes an indexed layout entry point.
- Existing string-keyed graph entry points remain compatible.
- Compatibility API delegates through indexed internals instead of duplicating the simulation setup.

Primary gates:

- `cargo nextest run -p manatee`

## M2 - Architecture Direct Indexed Layout

Exit criteria:

- Architecture builds indexed FCoSE input directly.
- Transient Architecture-side string graph construction is deleted or reduced to semantic-ID
  bookkeeping that cannot move into FCoSE.
- Architecture parity gates pass without root override growth.

Primary gates:

- `cargo nextest run -p merman-render architecture`
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3`
- `cargo run -p xtask -- report-overrides --check-no-growth`

## M3 - Performance Evidence

Exit criteria:

- Architecture layout stress benchmark is rerun.
- Pipeline benchmark for `architecture_medium` is rerun.
- Evidence records whether the indexed boundary reduced the gap and what remains.

Primary gates:

- `cargo bench -p merman --features render --bench architecture_layout_stress`
- `cargo bench -p merman --features render --bench pipeline -- architecture_medium`

## M4 - Closeout Or Split Follow-ons

Exit criteria:

- Gate set is recorded with fresh command output.
- Remaining work is either completed, deferred, or split into a follow-on workstream.
- `WORKSTREAM.json` status is updated.

Closeout rule:

- Typed dispatch consolidation and text measurement caching are separate lanes unless this
  workstream explicitly proves a direct dependency.

Closeout status: satisfied on 2026-05-28. All implementation, integration, parity, override,
formatting, clippy, package, and performance gates are recorded in `EVIDENCE_AND_GATES.md`.
