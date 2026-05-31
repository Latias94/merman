# Mermaid 11.15 Baseline Upgrade - Milestones

Status: Active
Last updated: 2026-05-31

## M0 - Scope And Evidence Freeze

Exit criteria:

- Workstream scope, non-goals, and release-delta sources are explicit.
- First executable compatibility slice is chosen.
- Task ledger and validation gates are available.

Primary evidence:

- `docs/workstreams/mermaid-11-15-baseline-upgrade/DESIGN.md`
- `docs/workstreams/mermaid-11-15-baseline-upgrade/TODO.md`

## M1 - Existing Diagram Compatibility

Exit criteria:

- Each selected existing-diagram delta lands with targeted tests.
- Backward-compatible defaults are preserved unless the upstream baseline changed them.
- Fixture churn is explained before broad baseline regeneration.

Primary gates:

- Targeted `cargo nextest` commands per task.
- Package gates for touched crates.

## M2 - Scope Decisions And Baseline Metadata

Exit criteria:

- New diagram family support is explicitly accepted, deferred, or split.
- README, ADRs, lock files, and alignment docs describe the real shipped state.
- Upstream SVG baselines are regenerated only for implemented scope.

Primary gates:

- Targeted render/core package gates.
- Broader closeout gate chosen from `EVIDENCE_AND_GATES.md`.

## M3 - Closeout

Exit criteria:

- Fresh verification evidence is recorded.
- Remaining work is either done or split into follow-on workstreams.
- `WORKSTREAM.json` status is updated.
