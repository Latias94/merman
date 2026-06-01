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
11.15 stored baselines. M15C-080 upstream-family decisions are recorded in
`docs/alignment/STATUS.md`; no unsupported 11.15 family was promoted into this campaign. Full
implemented-matrix SVG DOM `parity` now passes. The active remainder is M15C-070 `parity-root`:
root/viewBox/max-width residuals remain across the implemented matrix.
Fresh report triage shows flowchart=229, sequence=168, architecture=32, class=20, c4=15,
timeline=7, mindmap=4, sankey=3, journey=2, and er=4 table `dom ok = no` rows. The stale
`flowchart/upstream_docs_math_flowcharts_001` accepted residual policy entry has been removed.
The Flowchart FontAwesome 11.15 root slice reduced Flowchart diagram-specific `parity-root`
mismatches to 205 and root viewport override inventory to 281 total entries. The following
Flowchart SVG-markdown shape-layout slice fixed a real Dagre sizing error where `htmlLabels=false`
markdown node labels were measured unwrapped while the renderer emitted wrapped SVG markdown rows.
The representative new-shape fixture
`upstream_cypress_newshapes_spec_newshapessets_newshapesset1_tb_md_html_false_006` moved from a
`+929.090px` root max-width delta to `-1.340px`. The long-name C1 slice then closed the old
long-name/mojibake bucket and updated the two remaining 11.15 root pins. The Flowchart KaTeX CSS
slice closed `upstream_docs_math_flowcharts_001` without a root pin by loading `katex/dist/katex.css`
in the browser measurement probe; that fixture now reports `+0.000px` with root overrides disabled.
The Flowchart shape-alias geometry slice then aligned `hex`/`prepare`, `lined-cylinder`,
`paper-tape`/`flag`, and `docs`/`stacked-document` to Mermaid 11.15 source formulas. Targeted
strict-root checks for alias sets 7, 23, 35, and 33 pass with root overrides disabled. The plain
`Car` text-metric slice then proved the leading handdrawn/demo hex-looking bucket was not hex
geometry: local plain `Car` labels were retaining a vendored icon-like width instead of the Mermaid
11.15 browser DOM text width. The representative handdrawn/demo rows now pass with root overrides
disabled, structural implemented-matrix `parity` remains green, and Flowchart `parity-root` is
still red with 148 strict root-only mismatches. The demo flowchart 016/052 slice then identified
two stale root pins: unpinned renderer output was only `+0.922px` away, but the active pins still
forced the old `622.921875px` root. The existing pins now match the Mermaid 11.15 root
`640.921875px`, both targeted fixtures pass, override growth remains within budget, and Flowchart
`parity-root` is down to 146 strict root-only mismatches. The top residuals are now the remaining
smaller shape-alias buckets, delay/root rounding, markdown-subgraph, and shape-family geometry/root
buckets. The bow-tie rectangle geometry slice then aligned
`bow-rect`/`stored-data`/`bow-tie-rectangle` with Mermaid 11.15 `bowTieRect.ts`: classic mode uses
`2 * nodePadding` horizontal label padding instead of the old `nodePadding + 20px`. The
representative alias set 36 and docs stored-data root bucket are closed, structural parity remains
green, and Flowchart `parity-root` is down to 144 strict root-only mismatches. The window-pane
geometry slice then aligned `win-pane`/`internal-storage`/`window-pane` with Mermaid 11.15
`windowPane.ts`: `rectOffset` is `10`, not the old local `5`. Alias set 27 now passes strict-root
with root overrides disabled, structural parity remains green, and Flowchart `parity-root` is down
to 129 strict root-only mismatches.

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
- M15C-080 confirmed the unsupported Mermaid 11.15 family decisions:
  `eventmodeling`, `wardley`, `treeView`, `venn`, and `ishikawa` are deferred follow-on lanes;
  `cynefin` and `railroad` remain out of scope unless explicitly promoted later.
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
- M15C-070 Flowchart FontAwesome root triage found the Mermaid 11.15 inline icon measurement rule:
  standard and documented custom-pack `fa*` icon tokens use a `1.25em` inline box for layout. The
  slice updated Flowchart root pins for the remaining icon serialization gaps and deleted obsolete
  icon pins now covered by renderer output.
- M15C-070 Flowchart SVG-markdown shape-layout triage found that the remaining new-shape
  `htmlLabels=false` markdown fixtures were not root-override candidates. They exposed a shared
  layout/render metric split: Dagre used unwrapped markdown widths while render emitted wrapped SVG
  markdown tspans. Local layout now uses the wrapped SVG markdown metric path for shape sizing.
- M15C-070 Flowchart long-name triage found that preserved mojibake C1 controls were still using
  an old near-full-em HTML label fallback. The shared fallback is now near `0.6em`, which collapses
  the long-name courier/default root drift from `+96.050px`/`+98.490px` to small root-only
  residuals. The two long-name Flowchart root pins now match the Mermaid 11.15 roots, text lookup
  overrides stayed at `490`, and `report-overrides --check-no-growth` passes with root overrides at
  `282`.
- M15C-070 Flowchart math triage found that the Node/Puppeteer KaTeX probe was missing
  `katex/dist/katex.css`. Loading that stylesheet before browser measurement aligns Mermaid 11.15
  MathML label metrics; `upstream_docs_math_flowcharts_001` now passes strict `parity-root` with
  root overrides disabled, and Flowchart strict-root mismatch count is down to 202.
- M15C-070 Flowchart shape-alias geometry triage found multiple old shape formulas. The Rust
  sizing/rendering paths now follow Mermaid 11.15 `hexagon.ts`, `linedCylinder.ts`,
  `waveRectangle.ts`, and `multiWaveEdgedRectangle.ts` for hex/prepare, lined-cylinder,
  paper-tape/flag, and stacked-document. Flowchart strict-root mismatch count is down to 160 and
  structural implemented-matrix `parity` remains green.
- M15C-070 Flowchart plain `Car` triage found that the largest handdrawn/demo hex-looking bucket
  was actually text measurement drift: plain `Car` measured as `45.015625px` locally instead of
  the Mermaid 11.15 browser DOM text width `24.203125px`. This is baseline/browser-metric anchored,
  not a Mermaid shape-source formula. The guard excludes `fa:` and inline `<i>` icon labels, the
  FontAwesome boundary tests still pass, and Flowchart strict-root mismatch count is down to 148.
- M15C-070 Flowchart demo 016/052 triage found stale root pins rather than a renderer geometry
  problem. With root overrides disabled, both fixtures were within `+0.922px` of the 11.15 root;
  the active old pins forced `622.921875px`. The existing pins now use the pinned 11.15 root
  `640.921875px`, no override growth was introduced, and Flowchart strict-root mismatch count is
  down to 146.
- M15C-070 Flowchart bow-tie rectangle triage found an old width formula. The Rust layout,
  rendering, and edge intersection paths now follow Mermaid 11.15 `bowTieRect.ts` for
  `bow-rect`/`stored-data`/`bow-tie-rectangle`; alias set 36 passes strict-root with root overrides
  disabled, the docs stored-data bucket is closed, and Flowchart strict-root mismatch count is down
  to 144.
- M15C-070 Flowchart window-pane triage found an old `rectOffset` constant. The Rust layout,
  rendering, and edge intersection paths now follow Mermaid 11.15 `windowPane.ts` for
  `win-pane`/`internal-storage`/`window-pane`; alias set 27 passes strict-root with root overrides
  disabled, and Flowchart strict-root mismatch count is down to 129.

## Known Risks

- Regenerating all upstream SVG baselines at once may produce very large fixture churn. Prefer
  diagram-scoped batches.
- `parity-root` has a broad root-only residual set. Treat it as viewBox/max-width policy and root
  geometry work, not as structural DOM parity failure.
- `flowchart-elk` is not supported by the local layout path; it needs either an explicit skip
  policy or a separate ELK layout support lane.

## Next Recommended Action

Continue M15C-070. The Flowchart SVG-markdown shape-layout, long-name C1, shape-alias source
formula, plain `Car` text-metric, demo 016/052 stale-root-pin, and bow-tie rectangle buckets are
closed, and window-pane/internal-storage buckets are closed, but strict Flowchart `parity-root`
still reports 129 root-only mismatches. The next executable step is to sample the new top
Flowchart residuals: remaining shape-alias buckets (`20`, `21`, `12`, `29`, `38`, plus unpinned
`34`), delay half-rounded rectangle, Unicode punctuation/text-metric stress, markdown subgraph
root size, and shape-family layout/root clusters. Check whether each is a shared Mermaid 11.15
root geometry rule, a text metric rule, or only then a scoped root override. After Flowchart stops
exposing large shared buckets, compare Sequence/Class/C4/Architecture for a cross-family 11.15
root viewport rule change before adding broad fixture-scoped root pins.
