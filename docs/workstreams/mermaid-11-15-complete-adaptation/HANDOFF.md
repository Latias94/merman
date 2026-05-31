# Mermaid 11.15 Complete Adaptation - Handoff

Status: Active
Last updated: 2026-06-01

## Current State

The umbrella campaign is open. The repo baseline points at Mermaid `11.15.0`, generated artifacts
verify, and the Pie 11.15 lane is closed. M15C-030 removed active 11.12.3 report labels. M15C-040
has landed renderer fixes for Sequence central connections, Sequence 11.15 metadata, C4 scoped
symbols/type labels, Journey scoped task-line ids, and the remaining full Sequence 11.15 DOM
differences. Sequence, C4, and Journey stored upstream SVG baselines have been refreshed and now
pass their stored-fixture compares. Full implemented matrix SVG DOM parity is still red, but
Sequence is no longer in the failure set: current split is timeline=91, sankey=24, class=9,
flowchart=1, xychart=1.

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
- Fresh C4 11.15 output scopes base symbol ids (`computer`, `database`, `clock`) by SVG id and
  uses updated type-label text lengths for `system`, `system_db`, `system_queue`, and
  `external_person`. Local C4 now matches the fresh 11.15 full-diagram target, and stored C4 SVG
  baselines have been refreshed.
- Fresh Journey 11.15 output scopes task-line ids by SVG id. Local Journey now matches the fresh
  11.15 full-diagram target, and stored Journey SVG baselines have been refreshed.
- Full fresh Sequence 11.15 generation produced 322 SVGs, but full fresh compare still failed with
  121 mismatches. Do not refresh stored Sequence baselines until Sequence residuals are closed or
  explicitly split.
- Sequence residuals were closed after the full fresh probe. Stored Sequence baselines were
  refreshed and both `compare-sequence-svgs` and `compare-svg-xml --diagram sequence` pass in
  `parity` mode. `stress_end_keyword_016` is intentionally skipped in upstream SVG gates because
  Mermaid 11.15 rejects `(end)` as a participant id; keep it for local parser coverage.
- Fresh Timeline 11.15 output still does not match local output: representative deltas include
  scoped node ids such as `<svg-id>-node-0` versus local `node-undefined`, wrapper class/DOM shape
  differences, and multiline/tspan differences.

## Known Risks

- Regenerating all upstream SVG baselines at once may produce very large fixture churn. Prefer
  diagram-scoped batches.
- Marker-ID mismatches are widespread in stored baselines. Fresh C4 and Journey have proven the
  local scoped-id direction is correct there; Timeline needs renderer convergence rather than an
  unqualified baseline refresh.
- Full stored upstream SVG refresh was intentionally not done in one batch. `fixtures/upstream-svgs`
  still contains stale 11.12-era Timeline marker ids until Timeline converges.

## Next Recommended Action

Continue M15C-040 with Timeline convergence against `target/upstream-svgs-11-15-timeline`, then
move to M15C-050/M15C-060 for Sankey/Class/XYChart/Flowchart residuals. Sankey, Class, and XYChart
still need fresh 11.15 baseline checks before code changes; Flowchart has a single MathML
`columnalign` normalizer/renderer delta.
