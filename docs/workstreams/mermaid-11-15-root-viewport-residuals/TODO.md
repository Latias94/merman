# Mermaid 11.15 Root Viewport Residuals - TODO

Status: Active
Last updated: 2026-06-01

## M0 - Baseline Split

- [x] M15RV-010 [owner=codex] [deps=none] [scope=crates/xtask/src/cmd/compare/all.rs,target/compare,docs/workstreams/mermaid-11-15-root-viewport-residuals]
  Goal: Split root-only residual work out of the Mermaid 11.15 complete-adaptation campaign and
  make the full `parity-root` gate produce bounded, auditable failure summaries.
  Validation: `cargo nextest run -p xtask root_parity`;
  `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3`.
  Review: The command may fail while residuals remain, but it must fail normally and point to
  per-diagram reports instead of crashing.
  Evidence: `EVIDENCE_AND_GATES.md`
  Handoff: DONE. Fresh full root evidence reports 309 unaccepted residuals after existing accepted
  root policy entries are applied.

## M1 - Largest Residual Buckets

- [x] M15RV-020 [owner=codex] [deps=M15RV-010] [scope=crates/merman-render/src/svg/parity/sequence,crates/merman-render/src/text,fixtures/upstream-svgs/sequence,target/compare/sequence_report_parity_root.md]
  Goal: Classify the Sequence root residual bucket and split source-derived lifecycle/frame/text
  rules from browser/root lattice tails.
  Validation: focused `compare-sequence-svgs` checks for any fixed bucket plus full structural
  `compare-all-svgs --dom-mode parity`.
  Review: Do not add broad root pins or per-string constants unless generated browser evidence
  proves a reusable measurement fact.
  Evidence: `EVIDENCE_AND_GATES.md`
  Handoff: DONE. Sequence now has a `--no-root-overrides` diagnostic path; 3 stale root pins were
  deleted. Fresh full root evidence has 168 raw Sequence mismatches and 167 unaccepted after the
  existing `zed_pr_57644_sequence` accepted residual. The central-connection source rules match
  Mermaid 11.15; remaining central rows are root-bounds/text-measurement residuals.

- [x] M15RV-030 [owner=codex] [deps=M15RV-010] [scope=crates/merman-render/src/flowchart,crates/merman-render/src/svg/parity/flowchart,target/compare/flowchart_report_parity_root.md]
  Goal: Classify the remaining Flowchart root residual bucket after the 11.15 shape-source slices.
  Validation: focused `compare-flowchart-svgs --dom-mode parity-root` checks, with
  `--no-root-overrides` where stale-pin diagnosis is relevant.
  Review: Source-derived Mermaid rules are allowed; new exact browser text constants should be
  generated or rejected as diagnostic residuals.
  Evidence: `EVIDENCE_AND_GATES.md`
  Handoff: DONE. Fresh report still has 61 Flowchart root mismatches after deleting 1 stale root
  pin. Disabling Flowchart root overrides increases mismatches to 96, so retained pins are mostly
  still useful. The remaining rows are small root/text-measurement tails: max absolute root width
  delta is about 2.24px, with 60 style mismatches and 1 viewBox mismatch.

- [x] M15RV-040 [owner=codex] [deps=M15RV-010] [scope=crates/merman-render/src/svg/parity/architecture,crates/merman-render/src/svg/parity/class,crates/merman-render/src/svg/parity/c4,target/compare]
  Goal: Classify Architecture, Class, and C4 root residuals into source-rule, root-pin, and
  diagnostic browser-root buckets.
  Validation: focused diagram root compares and `report-overrides --check-no-growth`.
  Review: Architecture has large group/port layout-root drifts; do not collapse them into broad
  root tolerances.
  Evidence: `EVIDENCE_AND_GATES.md`
  Handoff: DONE. C4 is green after refreshing 15 existing fixture-derived root pins to the
  Mermaid 11.15 upstream root values. Architecture remains at 32 unaccepted root residuals
  dominated by group/port/disconnected-component layout-root drift; disabling Architecture root
  pins increases raw mismatches from 32 to 63, so retained pins are still useful. Class has 18
  unaccepted root residuals after 2 existing accepted policy rows; there is no Class root pin table,
  and the largest rows are namespace/layout-width residuals rather than stale root pins.

## M2 - Smaller Residual Buckets

- [x] M15RV-050 [owner=codex] [deps=M15RV-010] [scope=crates/merman-render/src/svg/parity/er,crates/merman-render/src/svg/parity/sankey,crates/merman-render/src/svg/parity/timeline,crates/merman-render/src/svg/parity/journey,target/compare]
  Goal: Classify the smaller ER, Sankey, Timeline, and Journey residuals and close source-derived
  rows when cheap and defensible.
  Validation: focused diagram root compares and full structural parity.
  Review: Prefer deleting stale pins or accepting tiny browser-root residuals over adding new
  fixture-like string constants.
  Evidence: `EVIDENCE_AND_GATES.md`
  Handoff: DONE. ER and Sankey are root-green after refreshing existing fixture-derived root pins
  to Mermaid 11.15 upstream root values. Timeline was reduced from 7 to 3 by refreshing 4 existing
  root pins; the remaining rows are unpinned 0.5-1px root-width measurement tails. Journey remains
  at 2 unpinned 1.25-2px root-width measurement tails and has no root pin table.

## M3 - Policy Closeout

- [ ] M15RV-090 [owner=planner] [deps=M15RV-020,M15RV-030,M15RV-040,M15RV-050] [scope=docs/workstreams/mermaid-11-15-root-viewport-residuals,crates/xtask/src/cmd/compare/all.rs]
  Goal: Close the root residual lane by either making `parity-root` green or accepting only
  documented diagnostic residuals with fresh evidence.
  Validation: `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`;
  `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3`;
  `cargo run -p xtask -- report-overrides --check-no-growth`; `cargo fmt --check`;
  `git diff --check`.
  Review: Run workstream review and verification before closing.
  Evidence: `EVIDENCE_AND_GATES.md`
  Handoff: Not started.
