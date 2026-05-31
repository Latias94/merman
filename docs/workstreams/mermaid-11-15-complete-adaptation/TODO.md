# Mermaid 11.15 Complete Adaptation - TODO

Status: Active
Last updated: 2026-05-31

## M0 - Scope And Evidence Freeze

- [x] M15C-010 [owner=planner] [deps=none] [scope=docs/workstreams/mermaid-11-15-complete-adaptation]
  Goal: Open the umbrella lane and freeze the current 11.15 gap model.
  Validation: Workstream docs exist and agree on scope.
  Evidence: `docs/workstreams/mermaid-11-15-complete-adaptation/DESIGN.md`
  Context: `docs/workstreams/mermaid-11-15-complete-adaptation/CONTEXT.jsonl`
  Handoff: DONE. The lane is active and the first executable task is M15C-020.

## M1 - Baseline Evidence And Tooling

- [x] M15C-020 [owner=codex] [deps=M15C-010] [scope=docs/workstreams/mermaid-11-15-complete-adaptation,target/compare,docs/alignment]
  Goal: Capture the current implemented-matrix parity failure inventory and classify stale-baseline
  drift versus likely renderer gaps.
  Validation: `cargo run -p xtask -- check-alignment`; `cargo run -p xtask -- verify-generated`;
  `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`
  recorded in `EVIDENCE_AND_GATES.md`.
  Review: Confirm the inventory is diagram-scoped and does not treat old baselines as renderer bugs.
  Evidence: `docs/workstreams/mermaid-11-15-complete-adaptation/EVIDENCE_AND_GATES.md`
  Context: this workstream plus `docs/rendering/UPSTREAM_SVG_BASELINES.md`.
  Handoff: DONE. `PARITY_FAILURE_INVENTORY.md` records the 525-mismatch inventory and first split.

- [x] M15C-030 [owner=codex] [deps=M15C-020] [scope=crates/xtask/src/cmd/compare,docs/rendering,docs/alignment]
  Goal: Remove or reclassify active 11.12.3 compare/report metadata that conflicts with the 11.15
  baseline claim.
  Validation: `cargo nextest run -p xtask`; `cargo run -p xtask -- check-alignment`;
  `cargo fmt --check`; `git diff --check`.
  Review: Historical docs may keep old version labels; active 11.15 reports must not mislabel the
  current baseline.
  Evidence: `EVIDENCE_AND_GATES.md`
  Context: this workstream plus `docs/adr/0001-upstream-baseline.md`.
  Handoff: DONE. Active compare report headers now say `pinned Mermaid baseline` instead of
  hard-coded Mermaid 11.12.3, and the hardening plan top-level baseline label names 11.15.

- [ ] M15C-040 [owner=codex] [deps=M15C-030] [scope=fixtures/upstream-svgs,tools/mermaid-cli,crates/xtask/src/cmd/generate.rs]
  Goal: Regenerate or check Mermaid 11.15 upstream SVG baselines for marker-ID impacted diagrams
  and split any real renderer mismatches.
  Validation: Targeted `check-upstream-svgs` / `gen-upstream-svgs` commands plus
  `compare-sequence-svgs`, `compare-c4-svgs`, `compare-journey-svgs`, and `compare-timeline-svgs`
  in `parity` mode.
  Review: Stage baseline churn separately from renderer code fixes when possible.
  Evidence: `EVIDENCE_AND_GATES.md`
  Context: this workstream plus `docs/rendering/UPSTREAM_SVG_BASELINES.md`.
  Handoff: IN_PROGRESS. Sequence fresh 11.15 probes are green for `basic` and `central`, but the
  full fresh Sequence corpus still has 121 DOM mismatches, so stored Sequence baselines were not
  refreshed. C4 and Journey fresh 11.15 full-diagram probes are green, and their stored upstream SVG
  baselines have been refreshed. Timeline fresh 11.15 still has broad renderer/model deltas and
  needs a separate convergence slice.

## M2 - Residual Existing-Matrix Parity

- [ ] M15C-050 [owner=codex] [deps=M15C-040] [scope=fixtures/upstream-svgs/sankey,crates/merman-render/src/svg/parity/sankey.rs,crates/merman-render/tests]
  Goal: Close Sankey 11.15 parity after baseline refresh, especially stroke-width/layout deltas.
  Validation: `cargo nextest run -p merman-render sankey`;
  `cargo run -p xtask -- compare-sankey-svgs --check-dom --dom-mode parity --dom-decimals 3`.
  Review: Decide whether remaining drift is baseline refresh, d3-sankey config, or renderer math.
  Evidence: `EVIDENCE_AND_GATES.md`
  Context: this workstream plus `docs/alignment/SANKEY_UPSTREAM_TEST_COVERAGE.md`.
  Handoff: Not started.

- [ ] M15C-060 [owner=codex] [deps=M15C-040] [scope=fixtures/upstream-svgs/class,fixtures/upstream-svgs/xychart,fixtures/upstream-svgs/flowchart,crates/merman-render/src/svg/parity]
  Goal: Close the remaining Class, XYChart, and Flowchart Math parity deltas after 11.15 baselines
  are authoritative.
  Validation: Targeted compare commands for class, xychart, and flowchart in `parity` mode plus
  package tests for any touched renderer.
  Review: Split a child lane if any one diagram turns into a larger renderer convergence effort.
  Evidence: `EVIDENCE_AND_GATES.md`
  Context: this workstream plus diagram-specific alignment docs.
  Handoff: Not started.

## M3 - Full Implemented-Matrix Gates

- [ ] M15C-070 [owner=codex] [deps=M15C-050,M15C-060] [scope=crates,fixtures,docs/workstreams/mermaid-11-15-complete-adaptation]
  Goal: Make the full implemented-matrix parity gate authoritative for Mermaid 11.15.
  Validation: `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`;
  `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3`;
  package/workspace tests as recorded in `EVIDENCE_AND_GATES.md`.
  Review: `parity-root` failures may be split only with fresh evidence and explicit non-goal wording.
  Evidence: `EVIDENCE_AND_GATES.md`
  Context: this workstream plus `docs/alignment/PARITY_HARDENING_PLAN.md`.
  Handoff: Not started.

## M4 - Upstream Family Decisions

- [ ] M15C-080 [owner=planner] [deps=M15C-020] [scope=docs/alignment/STATUS.md,docs/workstreams]
  Goal: Record final 11.15 decisions for upstream families not in the implemented matrix:
  `eventmodeling`, `wardley`, `treeView`, `venn`, `ishikawa`, `cynefin`, and `railroad`.
  Validation: `cargo run -p xtask -- check-alignment`; new child workstreams exist for promoted
  families.
  Review: The main baseline claim must not imply support for deferred families.
  Evidence: `docs/alignment/STATUS.md`
  Context: this workstream plus `repo-ref/mermaid/packages/mermaid/src/diagrams`.
  Handoff: Not started.

## M5 - Closeout

- [ ] M15C-090 [owner=planner] [deps=M15C-070,M15C-080] [scope=docs/workstreams/mermaid-11-15-complete-adaptation,docs/alignment]
  Goal: Close the campaign or split remaining 11.15 work into narrower lanes.
  Validation: Fresh closeout gates recorded in `EVIDENCE_AND_GATES.md`.
  Review: `review-workstream` and `verify-rust-workstream` before completion.
  Evidence: `EVIDENCE_AND_GATES.md`, `WORKSTREAM.json`
  Context: this workstream.
  Handoff: Not started.
