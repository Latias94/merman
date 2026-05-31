# Mermaid 11.15 Complete Adaptation - Handoff

Status: Active
Last updated: 2026-05-31

## Current State

The umbrella campaign is open. The repo baseline points at Mermaid `11.15.0`, generated artifacts
verify, and the Pie 11.15 lane is closed. M15C-030 removed active 11.12.3 report labels. M15C-040
has partially landed the sequence slice: fresh 11.15 `sequence/basic` and `sequence/central`
probes are green in DOM parity after fixing central connections and 11.15 sequence SVG metadata.
Full implemented-matrix SVG DOM parity is still red until the remaining stale upstream SVG baseline
batches and residual renderer deltas are closed.

## Active Task

- Task ID: M15C-040
- Owner: codex
- Files: `fixtures/upstream-svgs`, `tools/mermaid-cli`, `crates/xtask/src/cmd/generate.rs`
- Validation: targeted `check-upstream-svgs` / `gen-upstream-svgs` commands plus marker-ID
  impacted diagram compares
- Status: IN_PROGRESS
- Review: Stage baseline churn separately from renderer fixes where possible.
- Evidence: `docs/workstreams/mermaid-11-15-complete-adaptation/EVIDENCE_AND_GATES.md`

## Decisions Since Last Update

- Use this lane as an umbrella campaign, not as a monolithic implementation workstream.
- Make `parity` authoritative before `parity-root`.
- Treat new upstream diagram families as child-lane candidates.
- M15C-020 classified the current 525 DOM mismatches in `PARITY_FAILURE_INVENTORY.md`.
- M15C-030 removed active compare/report metadata that hard-coded Mermaid 11.12.3.
- M15C-040 sequence probe found one real renderer/model gap beyond stale SVG baselines:
  Mermaid 11.12.3+ central connections. The Rust parser/model now emits normalized actors,
  `centralConnection`, and type 59/60 internal control messages; fresh 11.15 sequence basic and
  central probes now pass DOM parity.
- Sequence 11.15 metadata was also updated: scoped marker/icon defs plus participant, lifeline,
  message, and note `data-*` attributes.

## Known Risks

- Regenerating all upstream SVG baselines at once may produce very large fixture churn. Prefer
  diagram-scoped batches.
- Marker-ID mismatches are widespread and likely baseline/tooling drift; avoid undoing local 11.15
  prefix behavior until fresh upstream 11.15 generation proves otherwise.
- Full stored upstream SVG refresh was intentionally not done in one batch. `fixtures/upstream-svgs`
  may still contain stale 11.12-era marker ids until diagram-scoped refreshes land.

## Next Recommended Action

Continue M15C-040 with C4, Journey, and Timeline marker-ID batches. For sequence, the next useful
step is a diagram-scoped upstream SVG baseline refresh/check now that fresh 11.15 `basic` and
`central` probes are green.
