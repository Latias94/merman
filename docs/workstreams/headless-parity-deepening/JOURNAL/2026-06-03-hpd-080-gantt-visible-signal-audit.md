# HPD-080 Gantt Visible Signal Audit

Date: 2026-06-03

## Summary

Re-audited Gantt public theme renderability after the visible-signal tightening work found a
measurement-quality risk: the compact smoke source rendered only a `done` task while expecting
ordinary task colors, and it did not prove outside-label DOM before counting `taskTextOutsideColor`.

This was not a production renderer defect. Local Gantt CSS already emits the Mermaid 11.15
ordinary-task, done-task, and outside-label state selectors. The public smoke source was too narrow.

## Source Evidence

- `repo-ref/mermaid/packages/mermaid/src/diagrams/gantt/styles.js`
- Local rendered evidence in `target/compare/gantt_visible_audit3.svg` shows the calibrated compact
  source emits:
  - `class="task task0"` for an ordinary task;
  - `class="taskTextOutsideRight taskTextOutside0 ..."` for an outside label;
  - `class="task done0"` for a done task.

## Changes

- Updated `crates/merman/tests/theme_renderability_smoke.rs` so the public Gantt smoke includes a
  wide ordinary task, a narrow long-label ordinary task, and a done task.
- Added `gantt_theme_smoke_counts_normal_and_done_task_dom_as_visible` to pin the DOM-counting
  boundary for Gantt visible colors.
- Updated the HPD-080 evidence and coverage docs to make this a visible-signal calibration, not a
  renderer parity claim.

## Verification

- `cargo nextest run -p merman --features render --test theme_renderability_smoke gantt_theme_smoke_counts_normal_and_done_task_dom_as_visible`
- `cargo nextest run -p merman --features render --test theme_renderability_smoke`
- `cargo run -p xtask -- compare-gantt-svgs --check-dom --dom-mode parity --dom-decimals 3`

## Residual

Gantt remains covered for the current implemented renderer shape. Future smoke additions should
avoid counting task-state or outside-label colors unless the sample renders matching state/label
DOM.
