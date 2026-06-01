# Mermaid 11.15 Complete Adaptation - Handoff

Status: Active
Last updated: 2026-06-01

## Current State

The umbrella campaign is open. The repo baseline points at Mermaid `11.15.0`, generated artifacts
verify, and the Pie 11.15 lane is closed. M15C-030 removed active 11.12.3 report labels. M15C-040
has landed renderer fixes for Sequence central connections, Sequence 11.15 metadata, C4 scoped
symbols/type labels, Journey scoped task-line ids, the remaining full Sequence 11.15 DOM
differences, Timeline scoped node ids, and the Sankey 11.15 baseline refresh. M15C-060 is now
closed: XYChart, Flowchart, ER, and Class have all been refreshed or converged against Mermaid
11.15 stored baselines. Full implemented-matrix SVG DOM `parity` now passes. The active remainder is
M15C-070 `parity-root`: root/viewBox/max-width residuals remain across the implemented matrix.
Fresh report triage shows flowchart=229, sequence=168, architecture=32, class=20, c4=15,
timeline=7, mindmap=4, sankey=3, journey=2, and er=4 table `dom ok = no` rows. The stale
`flowchart/upstream_docs_math_flowcharts_001` accepted residual policy entry has been removed.

## Active Task

- Task ID: M15C-070
- Owner: codex
- Files: `crates/xtask/src/cmd/compare`, `crates/merman-render/src/svg/parity`,
  `docs/workstreams/mermaid-11-15-complete-adaptation`
- Validation: full implemented-matrix `parity` and `parity-root` gates plus targeted renderer tests
  for any root-geometry fixes.
- Status: IN_PROGRESS
- Review: Structural `parity` is green. `parity-root` is red for root-only residuals and should be
  triaged separately from Class 11.15 structural convergence. Flowchart remains delegated to
  `docs/workstreams/flowchart-11-15-svg-convergence`; its supported structural matrix is green,
  while `flowchart-elk` remains a documented out-of-matrix skip until a dedicated ELK lane.
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
- The initial fresh Timeline 11.15 probe exposed scoped node ids such as `<svg-id>-node-0` versus
  local `node-undefined`; later raw SVG inspection narrowed the actionable DOM delta to node
  background ids.
- Timeline is now green after matching Mermaid 11.15 scoped node ids (`<svg-id>-node-N`) while
  preserving the upstream `node-undefined` class string. Stored Timeline SVG baselines were
  refreshed and both `compare-timeline-svgs` and `compare-svg-xml --diagram timeline` pass in
  `parity` mode.
- Fresh Sankey 11.15 output matched local output without renderer changes, proving the stored
  `stroke-width` failures were stale baseline drift. Stored Sankey SVG baselines were refreshed and
  both `compare-sankey-svgs` and `compare-svg-xml --diagram sankey` pass in `parity` mode.
- Fresh XYChart 11.15 output matched local output for
  `upstream_cypress_xychart_spec_should_use_all_the_config_from_yaml_013`, so its stored baseline
  was refreshed and the targeted XYChart parity gate passes.
- Fresh Class 11.15 output still fails for the 9 known stored failures; treat these as real Class
  11.15 namespace/DOM renderer gaps.
- Fresh Flowchart 11.15 output exposes 594 canonical XML mismatches plus one unsupported
  `flowchart-elk` local layout failure. Flowchart is split into a child workstream instead of
  staying as a targeted MathML `columnalign` cleanup. The first child-lane slice reduced the fresh
  Flowchart count to 359 mismatches and kept `flowchart-elk` as the remaining layout-policy
  failure.
- Later Flowchart child-lane slices made the supported Flowchart matrix green against fresh
  Mermaid 11.15 output, refreshed stored Flowchart SVG baselines, and documented `flowchart-elk`
  as out of the current headless support matrix.
- Fresh ER 11.15 stored-baseline refresh exposed 101 renderer DOM mismatches. ER is now green after
  matching the 11.15 unified-renderer envelope: root drop-shadow defs, scoped ids, `data-look`,
  no-attribute entity `markdown-node-label`, centered SVG relationship labels, attribute-table
  thin-rectangle dividers, theme gradients, and ELK edge ids without `_0`.
- Fresh Class 11.15 generation produced 245 SVGs and timed out for
  `upstream_parser_class_spec`, a documented upstream prototype-key artifact skip. Class fresh
  canonical XML parity was driven from 245 mismatches to zero, stored Class baselines were refreshed
  from the verified fresh output, and `compare-class-svgs`, `compare-svg-xml --diagram class`, and
  full implemented-matrix `parity` now pass.

## Known Risks

- Regenerating all upstream SVG baselines at once may produce very large fixture churn. Prefer
  diagram-scoped batches.
- `parity-root` has a broad root-only residual set. Treat it as viewBox/max-width policy and root
  geometry work, not as structural DOM parity failure.
- `flowchart-elk` is not supported by the local layout path; it needs either an explicit skip
  policy or a separate ELK layout support lane.

## Next Recommended Action

Continue M15C-070. Start by classifying the latest `parity-root` failures into true root-geometry
gaps versus residual-policy maintenance. The stale Flowchart Math policy entry has already been
removed; the next executable step is to identify whether the large Flowchart/Sequence/Class/C4/
Architecture buckets share a Mermaid 11.15 root viewport rule change before adding any new
fixture-scoped root overrides.
