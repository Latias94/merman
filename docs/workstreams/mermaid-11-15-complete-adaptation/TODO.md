# Mermaid 11.15 Complete Adaptation - TODO

Status: Active
Last updated: 2026-06-01

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

- [x] M15C-040 [owner=codex] [deps=M15C-030] [scope=fixtures/upstream-svgs,tools/mermaid-cli,crates/xtask/src/cmd/generate.rs]
  Goal: Regenerate or check Mermaid 11.15 upstream SVG baselines for marker-ID impacted diagrams
  and split any real renderer mismatches.
  Validation: Targeted `check-upstream-svgs` / `gen-upstream-svgs` commands plus
  `compare-sequence-svgs`, `compare-c4-svgs`, `compare-journey-svgs`, and `compare-timeline-svgs`
  in `parity` mode.
  Review: Stage baseline churn separately from renderer code fixes when possible.
  Evidence: `EVIDENCE_AND_GATES.md`
  Context: this workstream plus `docs/rendering/UPSTREAM_SVG_BASELINES.md`.
  Handoff: DONE. Sequence, C4, Journey, and Timeline are green against stored Mermaid 11.15
  upstream SVG baselines. Sequence keeps `stress_end_keyword_016` as local parser coverage but
  skips its stale SVG baseline in upstream gates because Mermaid 11.15 rejects `(end)` as a
  participant id. Timeline needed one renderer convergence fix for 11.15 scoped node ids before its
  stored baselines could be refreshed. At M15C-040 close, the full `parity` gate was red only for
  sankey=24, class=9, flowchart=1, xychart=1.

## M2 - Residual Existing-Matrix Parity

- [x] M15C-050 [owner=codex] [deps=M15C-040] [scope=fixtures/upstream-svgs/sankey,crates/merman-render/src/svg/parity/sankey.rs,crates/merman-render/tests]
  Goal: Close Sankey 11.15 parity after baseline refresh, especially stroke-width/layout deltas.
  Validation: `cargo nextest run -p merman-render sankey`;
  `cargo run -p xtask -- compare-sankey-svgs --check-dom --dom-mode parity --dom-decimals 3`.
  Review: Decide whether remaining drift is baseline refresh, d3-sankey config, or renderer math.
  Evidence: `EVIDENCE_AND_GATES.md`
  Context: this workstream plus `docs/alignment/SANKEY_UPSTREAM_TEST_COVERAGE.md`.
  Handoff: DONE. Fresh Mermaid 11.15 Sankey SVG baselines matched local output, proving the 24
  stored-fixture `stroke-width` mismatches were stale baseline drift rather than renderer math.
  Stored Sankey SVG baselines were refreshed and the Sankey stored gate now passes. The current full
  `parity` gate is red only for class=9, flowchart=1, xychart=1.

- [x] M15C-060 [owner=codex] [deps=M15C-040] [scope=fixtures/upstream-svgs/class,fixtures/upstream-svgs/xychart,fixtures/upstream-svgs/flowchart,fixtures/upstream-svgs/er,crates/merman-render/src/svg/parity,docs/workstreams/flowchart-11-15-svg-convergence]
  Goal: Close the remaining Class, XYChart, Flowchart, and ER parity deltas after 11.15 baselines
  are authoritative.
  Validation: Targeted compare commands for class, xychart, flowchart, and er in `parity` mode plus
  package tests for any touched renderer.
  Review: Split a child lane if any one diagram turns into a larger renderer convergence effort.
  Evidence: `EVIDENCE_AND_GATES.md`
  Context: this workstream plus diagram-specific alignment docs.
  Handoff: DONE. XYChart was stale baseline drift and has a refreshed targeted 11.15
  baseline. Class expanded from 14 stored DOM mismatches to a 245-SVG fresh 11.15 renderer
  convergence slice, then was fixed and refreshed. Flowchart was not a single
  MathML baseline issue: fresh Mermaid 11.15 output exposed 594 flowchart DOM mismatches plus one
  unsupported `flowchart-elk` fixture, so Flowchart is split to
  `docs/workstreams/flowchart-11-15-svg-convergence`. The Flowchart child lane later made the
  supported Flowchart matrix green and refreshed stored Flowchart baselines; the remaining
  `flowchart-elk` fixture is a documented out-of-matrix skip. ER also expanded from a stale-looking
  single stored mismatch into a full 101-fixture 11.15 renderer envelope refresh; ER is now green
  against stored Mermaid 11.15 SVGs. Class is now green against fresh and stored Mermaid 11.15 SVGs;
  `upstream_parser_class_spec` remains a documented upstream render artifact skip.

## M3 - Full Implemented-Matrix Gates

- [ ] M15C-070 [owner=codex] [deps=M15C-050,M15C-060] [scope=crates,fixtures,docs/workstreams/mermaid-11-15-complete-adaptation]
  Goal: Make the full implemented-matrix parity gate authoritative for Mermaid 11.15.
  Validation: `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`;
  `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3`;
  package/workspace tests as recorded in `EVIDENCE_AND_GATES.md`.
  Review: `parity-root` failures may be split only with fresh evidence and explicit non-goal wording.
  Evidence: `EVIDENCE_AND_GATES.md`
  Context: this workstream plus `docs/alignment/PARITY_HARDENING_PLAN.md`.
  Handoff: IN_PROGRESS. `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`
  now passes for the implemented matrix. `parity-root` is still red for root/viewBox/max-width
  residuals only. Fresh report triage shows a broader 11.15 root viewport recalibration set:
  flowchart=229, sequence=168, architecture=32, class=20, c4=15, timeline=7, mindmap=4,
  sankey=3, journey=2, and er=4 table `dom ok = no` rows. The stale expected
  `flowchart/upstream_docs_math_flowcharts_001` residual policy entry has been removed. The
  Flowchart FontAwesome 11.15 root slice reduced the Flowchart diagram-specific `parity-root`
  mismatch count to 205 and root override inventory to 281 total entries. The SVG-like markdown
  shape-layout slice then fixed the large Flowchart new/old shape geometry drift by using wrapped
  SVG markdown metrics for Dagre sizing; the representative new-shape fixture moved from a
  `+929.090px` root delta to `-1.340px`. The long-name C1/mojibake slice corrected shared
  Flowchart HTML label fallback metrics for preserved C1 controls and updated two 11.15 root pins
  for the remaining browser-font/root serialization delta. The KaTeX CSS math slice then fixed the
  browser measurement probe so `upstream_docs_math_flowcharts_001` passes strict `parity-root` with
  root overrides disabled; Flowchart `parity-root` then reported 202 strict root-only mismatches.
  The Flowchart shape-alias geometry slice aligned `hex`/`prepare`, `lined-cylinder`,
  `paper-tape`/`flag`, and `docs`/`stacked-document` sizing/rendering to Mermaid 11.15 source
  formulas. Targeted strict-root checks for the representative alias fixtures now pass with root
  overrides disabled, implemented-matrix structural `parity` remains green, and Flowchart
  `parity-root` reported 160 strict root-only mismatches. The following plain `Car` text-metric
  slice proved that the leading handdrawn/demo hex-looking bucket was actually an ordinary text
  label beside the hex shape being measured with the icon-like vendored width. That slice now keeps
  plain `Car` at the Mermaid 11.15 DOM text width while preserving FontAwesome icon metrics,
  removes the representative handdrawn/demo rows, keeps structural `parity` green, and leaves
  Flowchart `parity-root` at 148 strict root-only mismatches. The top residuals are now demo
  flowchart 016/052, small shape-alias buckets (`36`, `27`, `20`, `21`, `12`), delay/root rounding,
  markdown-subgraph, and shape-family geometry/root buckets. The demo flowchart 016/052 slice then
  proved those two rows were stale root pins, not renderer geometry: with root overrides disabled
  they were only `+0.922px` off, while the active pin still forced the old `622.921875px` root.
  The existing pins now match the Mermaid 11.15 baseline root (`640.921875px`), both targeted
  fixtures pass, override growth remains within budget, and Flowchart `parity-root` reports 146
  strict root-only mismatches. The leading residuals are now the remaining shape-alias and
  shape-family/root buckets. The bow-tie/stored-data shape slice then aligned
  `bow-rect`/`stored-data`/`bow-tie-rectangle` sizing, rendering, and edge intersection with
  Mermaid 11.15 `bowTieRect.ts`: classic mode uses `2 * nodePadding` horizontal label padding
  rather than the old `nodePadding + 20px`. The representative alias fixture and docs stored-data
  bucket are now closed, implemented-matrix structural `parity` remains green, and Flowchart
  `parity-root` reports 144 strict root-only mismatches. The window-pane/internal-storage slice
  then aligned `win-pane`/`internal-storage`/`window-pane` with Mermaid 11.15 `windowPane.ts`:
  `rectOffset` is `10`, not the old local `5`. Alias set 27 now passes strict-root with root
  overrides disabled, structural `parity` remains green, and Flowchart `parity-root` reports 129
  strict root-only mismatches. The document/delay geometry slice then aligned `doc`/`document`
  with Mermaid 11.15 `waveEdgedRectangle.ts` (`minWidth=14`) and
  `delay`/`half-rounded-rectangle` with `halfRoundedRectangle.ts` (`minWidth=15`,
  `minHeight=10`). Alias sets 20 and 21 plus the docs single-delay fixture now pass strict-root
  with root overrides disabled. A narrow upstream SVG text-width override closes the remaining
  `half-rounded-rectangle` 1/16px browser metric delta. Structural `parity` remains green, and
  Flowchart `parity-root` reports 124 strict root-only mismatches. The double-circle geometry
  slice then aligned `dbl-circ`/`double-circle` with Mermaid 11.15 `doubleCircle.ts`: outer radius
  is `bbox.width / 2 + padding`, so Dagre diameter is `label width + 2 * padding`, not the old
  `label width + padding + 10`. Alias set 12 now passes strict-root with root overrides disabled,
  structural `parity` remains green, and Flowchart `parity-root` reports 101 strict root-only
  mismatches. The lined/tagged document slice then aligned `lin-doc`/`lined-document` with
  Mermaid 11.15 `linedWaveEdgedRect.ts` and `tag-doc`/`tagged-document` with
  `taggedWaveEdgedRectangle.ts`: classic lined documents use `h / 8` wave amplitude, and tagged
  documents use the 11.15 tag sine constants. Alias sets 29 and 38 now pass strict-root with root
  overrides disabled. Alias set 34 was a 1/64px upstream SVG/browser text metric residual for the
  `stacked-rectangle` label, so it is captured as a narrow Flowchart HTML width override. The full
  `shape_alias` strict-root sweep now passes with root overrides disabled, structural `parity`
  remains green, and Flowchart `parity-root` reports 71 strict root-only mismatches. The
  curved-trapezoid/display slice then aligned local `curv-trap`/`display`/`curved-trapezoid`
  minimum geometry constants with Mermaid 11.15 `curvedTrapezoid.ts` (`minWidth=20`,
  `minHeight=5`), closing the no-label `newshapesset3` LR/TB root bucket with root overrides
  disabled. Structural `parity` remains green, and Flowchart `parity-root` reports 69 strict
  root-only mismatches. The Unicode text-metric slice then closed the two largest text residuals
  by anchoring CJK/Hangul, emoji, and Windows-path HTML min-content widths to the Mermaid 11.15 SVG
  browser metrics. Structural `parity` remains green, and Flowchart `parity-root` reports 67
  strict root-only mismatches. The icon root-pin refresh then updated an existing stale
  `upstream_cypress_flowchart_icon_spec_example_002` root override from `92.046875px` to the
  Mermaid 11.15 `98.046875px` root after proving the renderer passes with root overrides disabled.
  Flowchart `parity-root` reports 66 strict root-only mismatches.

## M4 - Upstream Family Decisions

- [x] M15C-080 [owner=planner] [deps=M15C-020] [scope=docs/alignment/STATUS.md,docs/workstreams]
  Goal: Record final 11.15 decisions for upstream families not in the implemented matrix:
  `eventmodeling`, `wardley`, `treeView`, `venn`, `ishikawa`, `cynefin`, and `railroad`.
  Validation: `cargo run -p xtask -- check-alignment`; new child workstreams exist for promoted
  families.
  Review: The main baseline claim must not imply support for deferred families.
  Evidence: `docs/alignment/STATUS.md`
  Context: this workstream plus `repo-ref/mermaid/packages/mermaid/src/diagrams`.
  Handoff: DONE. `docs/alignment/STATUS.md` now records the 2026-06-01 final decision check
  against `repo-ref/mermaid/packages/mermaid/src/diagrams`. No family was promoted in this
  campaign, so no child workstreams are required yet. `eventmodeling`, `wardley`, `treeView`,
  `venn`, and `ishikawa` are deferred follow-on diagram-family lanes; `cynefin` and `railroad`
  remain out of scope unless explicitly promoted later.

## M5 - Closeout

- [ ] M15C-090 [owner=planner] [deps=M15C-070,M15C-080] [scope=docs/workstreams/mermaid-11-15-complete-adaptation,docs/alignment]
  Goal: Close the campaign or split remaining 11.15 work into narrower lanes.
  Validation: Fresh closeout gates recorded in `EVIDENCE_AND_GATES.md`.
  Review: `review-workstream` and `verify-rust-workstream` before completion.
  Evidence: `EVIDENCE_AND_GATES.md`, `WORKSTREAM.json`
  Context: this workstream.
  Handoff: Not started.
