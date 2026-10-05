---
type: "Work Progress"
title: "Mermaid 12 prepared routes and polygon intersections verified"
description: "Work Progress for Mermaid 12 prepared routes and polygon intersections verified."
timestamp: 2026-09-23T15:48:35Z
record_id: "f7a8b19231a94b3dbf08bca308e0e731"
producer_id: "codex-mermaid12-recovery"
run_id: "session-01a0ce25"
---

# Verified changes

- `96a6b9b45` fixes Class ELK painting: the renderer now consumes prepared edges instead of
  reloading original layout routes. The shared terminal straightener had already produced the
  correct channels, but its output and terminal-label updates were discarded. The new source/
  target regression failed before the change and passed afterward; explicit opt-out and fixed
  ports are checked.
- `92443baca` ports Mermaid 12 polygon intersection without the old half-pixel rounding bias.
  Hourglass now uses the same source-backed polygon implementation instead of a duplicated
  special path. The pinned source's asymmetric second-side epsilon test is retained. Direct
  execution of the selected upstream JavaScript confirmed the regression expectations.
- Independent read-only reviews found no correctness issue in either fix. No push, publication,
  PR, or merge occurred. Inherited worktree changes remain intact.

# Executed verification

- Class ELK focused SVG tests: 4/4 passed.
- New polygon intersection tests: 3/3 passed.
- Public facade `flowchart_elk_render`: 9/9 passed, including the corrected stadium assertion.
- Full Class/Flowchart/stacked-rectangle SVG suites with `layout-elk`: 123/123 passed.
- Fresh `compare-er-svgs --check-dom --dom-mode parity`: 101/101 rendered, no skips, passed.
- Fresh `compare-requirement-svgs --check-dom --dom-mode parity`: 47/47 rendered, no skips, passed.
- Fresh Class parity: 251 selected, 249 rendered, 2 pre-existing exclusions; one mismatch remains
  in `stress_class_svg_font_size_px_string_precedence_026`. Its split positions and line count
  vary with browser text measurements; the existing exact browser-layout receipt still targets
  Mermaid 11.17.2 and has not been migrated. Do not claim the full Class gate passed.
- Class's eight semantic-label receipts and ER's two receipts were individually compared with
  target-generated SVGs and migrated in the working tree. Class's extra terminal jogs were fixed
  before acceptance. Both canaries now preserve directed route topology; the remaining widths,
  anchors, path lengths, and Neo masks follow source-owned browser measurements. The normal
  comparator accepted all eight Class and both ER receipts without candidate mode.
- Fresh Flowchart parity reports 118 mismatching fixtures (87 first differences in path commands,
  18 child-count differences, 13 other). Fresh State parity reports 38 mismatching fixtures,
  all first differences in path commands. These remain unadmitted; no topology normalization
  or blanket receipt refresh was applied.

Local logs live under `target/mermaid12-*.log`; ordinary comparison reports are
`target/compare/<family>_report.md`. Historical `*_report_structure_parity.md` files are stale
and must not be mistaken for the current run.

# Integration state and next work

The prior September 23 strict run terminated at nextest with exit 100: 673 passed, one failed,
seven skipped, and 7,918 not run. The failing stadium test was the obsolete expectation that
Mermaid 12 must retain a half-pixel cutter jog. That failure is now fixed. Earlier stages of that
run passed formatting/check/Clippy, the 117 feature combinations, reference/generated/alignment
checks, RustSec and license checks, and 382 legal projections. Its downstream doctest, full DOM,
Web, Playground, and VS Code stages were not reached.

A new `cargo nextest run --workspace --no-fail-fast --test-threads 1` with
`CARGO_BUILD_JOBS=1` is running as this checkpoint is written; compilation is recorded in
`target/mermaid12-workspace-nextest-resumed.log`. Poll the actual process/session before any
restart; this paragraph is not a completion receipt.

Continue source-backed Flowchart/State route convergence. Two Flowchart examples show swapped
symmetric ELK routes before jump painting, so changing arc ownership alone would hide the cause.
The browser-layout catalog still has 96 historical exact receipts and requires reviewed migration;
it must not be bulk re-signed. Class/ER documentation and label catalog migration are uncommitted.

The five Web WASM artifacts date from September 21 and predate later renderer/version changes.
Analysis previously exceeded compressed budgets (gzip 1,415,591 > 1,375,000; Brotli 1,090,288 >
1,050,000). Binaryen `-Oz` made compressed sizes worse. No budget increase or capability removal
is authorized. Final fresh WASM, consumer/browser checks, package size measurement, strict
integration, documentation, and focused commits remain required by the full plan.

# Citations

- [Full Mermaid 12 plan](../../../../plans/2026-09-20-1250-refactor-mermaid-12-alignment-plan.md)
- [Class contract](../../../../alignment/CLASS_MINIMUM.md)
- [Default ELK product boundary](../../../../adr/0089-mermaid-12-default-elk-products.md)
