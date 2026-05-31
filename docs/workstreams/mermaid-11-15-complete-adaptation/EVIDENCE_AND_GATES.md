# Mermaid 11.15 Complete Adaptation - Evidence And Gates

Status: Active
Last updated: 2026-06-01

## Smallest Current Repro

```bash
cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3
```

As of 2026-06-01 this fails only for the current ER/Class remainder. Flowchart has been removed
from the full-gate failure set after the stored Mermaid 11.15 baseline refresh.

## Gate Set

### Baseline And Artifact Gates

```bash
cargo run -p xtask -- check-alignment
cargo run -p xtask -- verify-generated
```

### Full SVG Parity Gates

```bash
cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3
cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3
```

`parity` is the first closeout gate. `parity-root` should be run after structural parity is green or
after a scoped root-viewport task needs evidence.

### Package And Workspace Gates

Use targeted tests for touched packages, then the workspace gate near closeout:

```bash
cargo nextest run -p xtask
cargo nextest run -p merman-core
cargo nextest run -p merman-render
$env:CARGO_PROFILE_TEST_DEBUG='0'; $env:CARGO_BUILD_JOBS='2'; cargo nextest run --workspace
```

### Formatting And Diff Gates

```bash
cargo fmt --check
git diff --check
```

## Evidence Log

- 2026-05-31 lane opening:
  - Goal: close Mermaid 11.15 complete-adaptation campaign by making the implemented diagram matrix
    green against Mermaid 11.15 SVG baselines and recording all remaining family decisions.
  - `git status --short`: clean before opening docs.
  - `cargo run -p xtask -- check-alignment`: passed.
  - `cargo run -p xtask -- verify-generated`: passed.
  - `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`:
    failed.
  - Failure count from generated parity reports: 525 DOM mismatches across 8 diagram groups:
    sequence=322, timeline=91, c4=51, journey=26, sankey=24, class=9, flowchart=1, xychart=1.
  - Dominant failure family: marker ID prefix drift where stored upstream SVG baselines use bare
    IDs such as `arrowhead`, while local 11.15-oriented output uses `<svg-id>-arrowhead`.
  - Additional likely residuals: Sankey stroke-width/layout deltas, Class hierarchical namespace
    DOM deltas, Flowchart MathML `columnalign`, and XYChart data-label color.
  - Active compare reports still label upstream SVG baselines as Mermaid 11.12.3, so baseline
    metadata must be fixed before treating all mismatches as renderer defects.
- 2026-05-31 M15C-020 inventory:
  - `docs/workstreams/mermaid-11-15-complete-adaptation/PARITY_FAILURE_INVENTORY.md` records the
    current 525-mismatch split.
  - Stale-baseline dominated batch: sequence, timeline, c4, journey (490 mismatches).
  - Fresh-baseline-before-code batch: sankey, class, xychart.
  - Targeted residual candidate: flowchart MathML `columnalign`.
- 2026-05-31 M15C-030 active metadata cleanup:
  - Compare report headers in `crates/xtask/src/cmd/compare/diagrams/*.rs` now refer to the
    `pinned Mermaid baseline` instead of hard-coded Mermaid 11.12.3.
  - `docs/rendering/SVG_CANONICAL_XML.md` now points to the baseline pinned in
    `tools/upstreams/REPOS.lock.json`.
  - `docs/alignment/PARITY_HARDENING_PLAN.md` now names Mermaid `@11.15.0` as the current baseline
    and keeps 11.12.3 labels as historical snapshots.
  - `rg "Mermaid 11\\.12\\.3|Mermaid CLI pinned to Mermaid 11\\.12\\.3|\\(Mermaid 11\\.12\\.3\\)" crates/xtask/src/cmd/compare/diagrams docs/rendering/SVG_CANONICAL_XML.md docs/alignment/PARITY_HARDENING_PLAN.md -n`:
    no matches.
  - `cargo fmt --check`: passed.
  - `git diff --check`: passed.
  - `cargo nextest run -p xtask`: passed, 67 tests.
  - `cargo run -p xtask -- check-alignment`: passed.
- 2026-05-31 M15C-040 sequence 11.15 probe and central-connection fix:
  - `node -e "console.log(require('./tools/mermaid-cli/node_modules/mermaid/package.json').version)"`:
    printed `11.15.0`.
  - `cargo run -p xtask -- gen-upstream-svgs --diagram sequence --filter basic --out target/upstream-svgs-11-15-probe`:
    generated fresh 11.15 sequence `basic` probe SVGs.
  - Initial fresh `basic` compare failed only on
    `sequence/upstream_docs_sequencediagram_basic_syntax_035`, proving the stored sequence
    baselines were stale but also exposing a real central-connection parser/model gap.
  - Implemented sequence central connections as upstream does: normalized actor ids, numeric
    `centralConnection` on visible messages, and internal type 59/60 control messages. This also
    fixed resulting SVG message `data-id` values (`i0`, `i2`, `i4`) and central circle DOM.
  - Added 11.15 sequence marker defs (`solidTopArrowHead`, `solidBottomArrowHead`,
    `stickTopArrowHead`, `stickBottomArrowHead`), scoped sequence symbol ids, and 11.15 sequence
    data attributes for participants, lifelines, messages, and notes.
  - `cargo run -p xtask -- compare-svg-xml --check --diagram sequence --filter basic --upstream-root target/upstream-svgs-11-15-probe --dom-mode parity --dom-decimals 3`:
    passed.
  - `cargo run -p xtask -- gen-upstream-svgs --diagram sequence --filter central --out target/upstream-svgs-11-15-central`:
    generated fresh 11.15 central-connection SVGs.
  - `cargo run -p xtask -- compare-svg-xml --check --diagram sequence --filter central --upstream-root target/upstream-svgs-11-15-central --dom-mode parity --dom-decimals 3`:
    passed.
  - `cargo nextest run -p merman-core sequence`: passed, 32 tests.
  - `cargo nextest run -p merman-render sequence`: passed, 16 tests.
  - `cargo nextest run -p merman-core fixtures_match_golden_snapshots`: passed after refreshing only
    central-connection semantic goldens.
  - `cargo nextest run -p merman-render fixtures_match_layout_golden_snapshots_when_present`:
    passed after refreshing only central-connection layout goldens.
  - `cargo fmt --check`: passed.
  - Non-sequence marker-ID baseline refresh remains open for C4, Journey, and Timeline; stored
    `fixtures/upstream-svgs/sequence` was not bulk-refreshed in this slice.
- 2026-05-31 M15C-040 C4/Journey/Timeline fresh 11.15 probes:
  - `cargo run -p xtask -- gen-upstream-svgs --diagram c4 --out target/upstream-svgs-11-15-c4`:
    generated 51 fresh Mermaid 11.15 C4 SVGs.
  - Initial C4 fresh compare failed with 51 mismatches. After scoping the C4 base symbol ids and
    updating 11.15 C4 type-label `textLength` constants, `cargo run -p xtask -- compare-svg-xml --check --diagram c4 --upstream-root target/upstream-svgs-11-15-c4 --dom-mode parity --dom-decimals 3`:
    passed.
  - `cargo run -p xtask -- gen-upstream-svgs --diagram journey --out target/upstream-svgs-11-15-journey`:
    generated 26 fresh Mermaid 11.15 Journey SVGs. Mermaid CLI printed upstream NaN attribute
    warnings for a subset of fixtures but exited successfully.
  - Initial Journey fresh compare failed with 17 mismatches. After scoping Journey task-line ids,
    `cargo run -p xtask -- compare-svg-xml --check --diagram journey --upstream-root target/upstream-svgs-11-15-journey --dom-mode parity --dom-decimals 3`:
    passed.
  - `cargo run -p xtask -- gen-upstream-svgs --diagram timeline --out target/upstream-svgs-11-15-timeline`:
    generated 91 fresh Mermaid 11.15 Timeline SVGs.
  - `cargo run -p xtask -- compare-svg-xml --check --diagram timeline --upstream-root target/upstream-svgs-11-15-timeline --dom-mode parity --dom-decimals 3`:
    failed with 90 mismatches. Representative fresh 11.15 deltas include upstream scoped node ids
    such as `<svg-id>-node-0` versus local `node-undefined`, `taskWrapper`/`eventWrapper` class
    ordering and DOM-shape differences, and multiline/tspan differences. Timeline is no longer
    classified as only stale marker-id baseline drift.
  - `cargo nextest run -p merman-render c4`: passed, 3 tests.
  - `cargo nextest run -p merman-render journey`: passed, 3 tests.
  - `cargo fmt --check`: passed.
  - `git diff --check`: passed.
  - Stored upstream SVG baselines were not refreshed in this code-fix slice.
- 2026-05-31 M15C-040 C4/Journey stored baseline refresh and Sequence full-corpus check:
  - `cargo run -p xtask -- gen-upstream-svgs --diagram sequence --out target/upstream-svgs-11-15-sequence`:
    generated 322 fresh Mermaid 11.15 Sequence SVGs. The command took longer than the initial
    5-minute shell timeout, but the original `xtask` process continued and completed.
  - `cargo run -p xtask -- compare-svg-xml --check --diagram sequence --upstream-root target/upstream-svgs-11-15-sequence --dom-mode parity --dom-decimals 3`:
    failed with 121 mismatches. Dominant observed categories include control-structure data/class
    differences, participant type SVG/data differences, note ordering, and wrapped text/tspan
    differences. Stored Sequence baselines were not refreshed.
  - `cargo run -p xtask -- gen-upstream-svgs --diagram c4 --out fixtures/upstream-svgs`: passed and
    refreshed 51 stored C4 SVG baselines.
  - `cargo run -p xtask -- gen-upstream-svgs --diagram journey --out fixtures/upstream-svgs`:
    passed and refreshed 26 stored Journey SVG baselines; Mermaid CLI printed the same upstream NaN
    attribute warnings observed during the target probe.
  - `cargo run -p xtask -- compare-svg-xml --check --diagram c4 --dom-mode parity --dom-decimals 3`:
    passed against stored fixtures.
  - `cargo run -p xtask -- compare-svg-xml --check --diagram journey --dom-mode parity --dom-decimals 3`:
    passed against stored fixtures.
  - `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`:
    failed with the current stored-baseline split: sequence=322, timeline=91, sankey=24, class=9,
    flowchart=1, xychart=1. C4 and Journey are now green in the full gate.
- 2026-06-01 M15C-040 Sequence full-corpus convergence and stored baseline refresh:
  - Implemented the remaining Sequence 11.15 DOM deltas found by the fresh full-corpus probe:
    control-structure group metadata, section-title text classes, participant type data/classes,
    queue/database actor wrapper shape, actor-man DOM/style ordering, self-message `x1`, and note
    wrapping slack.
  - `compare-svg-xml` now attaches the Node KaTeX math renderer for Sequence as well as Flowchart,
    fixing the Sequence math fixture in parity mode.
  - `stress_end_keyword_016` is explicitly excluded from upstream SVG generation/check/DOM compare:
    Mermaid 11.15 rejects `(end)` as a participant id, while merman keeps the fixture for local
    parser coverage.
  - `cargo run -p xtask -- gen-upstream-svgs --diagram sequence --out fixtures/upstream-svgs`:
    refreshed stored Sequence SVG baselines. The shell timeout expired first, but the original
    `xtask` process continued and completed; the only unrefreshable fixture is the skipped
    `stress_end_keyword_016`.
  - `cargo run -p xtask -- gen-upstream-svgs --diagram sequence --filter stress_end_keyword_016 --out target/upstream-svgs-skip-probe`:
    passed, skipped 1 known upstream render gap.
  - `cargo run -p xtask -- check-upstream-svgs --diagram sequence --filter stress_end_keyword_016 --check-dom --dom-mode parity --dom-decimals 3`:
    passed, skipped 1 known upstream render/check gap.
  - `cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-mode parity --dom-decimals 3`:
    passed.
  - `cargo run -p xtask -- compare-svg-xml --check --diagram sequence --dom-mode parity --dom-decimals 3`:
    passed; report records 1 skipped fixture.
  - `cargo nextest run -p merman-render sequence`: passed, 16 tests.
  - `cargo nextest run -p xtask`: passed, 67 tests.
  - `cargo fmt --check`: passed.
  - `git diff --check`: passed.
  - `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`:
    failed with the current remaining split: timeline=91, sankey=24, class=9, flowchart=1,
    xychart=1. Sequence, C4, and Journey no longer appear in the full-gate failure set.
- 2026-06-01 M15C-040 Timeline convergence and stored baseline refresh:
  - Implemented Mermaid 11.15 Timeline scoped node background ids (`<svg-id>-node-N`) while keeping
    the upstream `node-bkg node-undefined` class string.
  - `cargo run -p xtask -- compare-svg-xml --check --diagram timeline --filter upstream_docs_timeline_an_example_of_a_timeline_002 --upstream-root target/upstream-svgs-11-15-timeline --dom-mode parity --dom-decimals 3`:
    passed.
  - `cargo run -p xtask -- compare-svg-xml --check --diagram timeline --upstream-root target/upstream-svgs-11-15-timeline --dom-mode parity --dom-decimals 3`:
    passed against the fresh Mermaid 11.15 Timeline target.
  - `cargo run -p xtask -- gen-upstream-svgs --diagram timeline --out fixtures/upstream-svgs`:
    the shell timed out at 5 minutes, but the original `xtask` process continued and completed;
    91 stored Timeline SVG baselines were refreshed and no generator process remained afterward.
  - `cargo run -p xtask -- compare-timeline-svgs --check-dom --dom-mode parity --dom-decimals 3`:
    passed.
  - `cargo run -p xtask -- compare-svg-xml --check --diagram timeline --dom-mode parity --dom-decimals 3`:
    passed.
  - `cargo nextest run -p merman-render timeline`: passed, 4 tests.
  - `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`:
    failed with the current remaining split: sankey=24, class=9, flowchart=1, xychart=1. Timeline
    no longer appears in the full-gate failure set.
- 2026-06-01 M15C-050 Sankey stored baseline refresh:
  - `cargo run -p xtask -- compare-sankey-svgs --check-dom --dom-mode parity --dom-decimals 3`:
    failed before refresh with 24 `stroke-width` mismatches.
  - `cargo run -p xtask -- gen-upstream-svgs --diagram sankey --out target/upstream-svgs-11-15-sankey`:
    passed and generated 33 fresh Mermaid 11.15 Sankey SVGs.
  - `cargo run -p xtask -- compare-svg-xml --check --diagram sankey --upstream-root target/upstream-svgs-11-15-sankey --dom-mode parity --dom-decimals 3`:
    passed against the fresh Mermaid 11.15 Sankey target, proving the stored failures were stale
    baseline drift rather than renderer math.
  - `cargo run -p xtask -- gen-upstream-svgs --diagram sankey --out fixtures/upstream-svgs`:
    passed and refreshed 33 stored Sankey SVG baselines.
  - `cargo run -p xtask -- compare-sankey-svgs --check-dom --dom-mode parity --dom-decimals 3`:
    passed.
  - `cargo run -p xtask -- compare-svg-xml --check --diagram sankey --dom-mode parity --dom-decimals 3`:
    passed.
  - `cargo nextest run -p merman-render sankey`: passed, 4 tests.
  - `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`:
    failed with the current remaining split: class=9, flowchart=1, xychart=1. Sankey no longer
    appears in the full-gate failure set.
  - `cargo run -p xtask -- check-alignment`: passed.
  - `cargo fmt --check`: passed.
  - `git diff --check`: passed.
- 2026-06-01 M15C-060 residual triage and Flowchart split:
  - `cargo run -p xtask -- gen-upstream-svgs --diagram class --filter <9 failing class fixtures> --out target/upstream-svgs-11-15-class-probe`:
    generated fresh Mermaid 11.15 Class probes.
  - `cargo run -p xtask -- compare-svg-xml --check --diagram class --filter <9 failing class fixtures> --upstream-root target/upstream-svgs-11-15-class-probe --dom-mode parity --dom-decimals 3`:
    failed for the same 9 fixtures. These are real Class renderer/namespace DOM gaps, not stale
    stored baselines. Representative 11.15 deltas include scoped node ids, `data-look`, marker defs,
    drop-shadow defs, and root group structure.
  - `cargo run -p xtask -- gen-upstream-svgs --diagram xychart --filter upstream_cypress_xychart_spec_should_use_all_the_config_from_yaml_013 --out target/upstream-svgs-11-15-xychart-probe`:
    passed.
  - `cargo run -p xtask -- compare-svg-xml --check --diagram xychart --filter upstream_cypress_xychart_spec_should_use_all_the_config_from_yaml_013 --upstream-root target/upstream-svgs-11-15-xychart-probe --dom-mode parity --dom-decimals 3`:
    passed, proving the stored XYChart mismatch was stale baseline drift.
  - `cargo run -p xtask -- gen-upstream-svgs --diagram xychart --filter upstream_cypress_xychart_spec_should_use_all_the_config_from_yaml_013 --out fixtures/upstream-svgs`:
    refreshed the stored XYChart baseline for the stale fixture.
  - `cargo run -p xtask -- compare-xychart-svgs --check-dom --dom-mode parity --dom-decimals 3 --filter upstream_cypress_xychart_spec_should_use_all_the_config_from_yaml_013`:
    passed.
  - `cargo run -p xtask -- gen-upstream-svgs --diagram flowchart --filter upstream_docs_math_flowcharts_001 --out target/upstream-svgs-11-15-flowchart-probe`:
    passed. Fresh Mermaid 11.15 and local output both include MathML `columnalign`; the stored
    baseline was stale for this one fixture.
  - `cargo run -p xtask -- gen-upstream-svgs --diagram flowchart --filter upstream_docs_math_flowcharts_001 --out fixtures/upstream-svgs`:
    refreshed the stored Flowchart Math baseline for the stale `columnalign` fixture.
  - Initial Flowchart 11.15 DOM-envelope renderer work made
    `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3 --filter upstream_docs_math_flowcharts_001`
    pass for the targeted Math fixture, but full Flowchart parity is not green.
  - `cargo run -p xtask -- gen-upstream-svgs --diagram flowchart --out target/upstream-svgs-11-15-flowchart`:
    generated 1070 fresh Mermaid 11.15 Flowchart SVGs after the shell timeout expired and the
    original `xtask` process continued. Five parser-only or upstream-render-failing fixtures did
    not produce SVGs:
    `upstream_flow_text_ellipse_vertex_parser_only_spec`,
    `upstream_html_demos_flowchart_flowchart_040_parser_only_katex`,
    `upstream_html_demos_flowchart_flowchart_042_parser_only_katex`,
    `upstream_html_demos_flowchart_flowchart_044_parser_only_katex`, and
    `upstream_html_demos_flowchart_graph_039_parser_only_katex`.
  - `cargo run -p xtask -- compare-svg-xml --check --diagram flowchart --upstream-root target/upstream-svgs-11-15-flowchart --dom-mode parity --dom-decimals 3`:
    failed with 594 canonical XML mismatches plus one local layout failure for
    `flowchart/upstream_html_demos_flowchart_elk_flowchart_elk_001` because `flowchart-elk` is not
    supported by the local layout path.
  - Lightweight mismatch classification from the fresh Flowchart target:
    `outer_path_class=203`, `edge_markdown_rows=61`, `missing_row_class=61`,
    `shape_path_class=77`, `anchor_or_click=23`, `html_foreign_object=556`,
    `subgraph_cluster=594`, `other=0`.
  - Flowchart is split to `docs/workstreams/flowchart-11-15-svg-convergence` instead of treating
    the old stored-baseline red point as a single MathML fix. Do not bulk-refresh stored Flowchart
    SVG baselines until that child lane is green against fresh Mermaid 11.15 output.
  - First child-lane Flowchart convergence slice reduced fresh Flowchart mismatches from 594 to
    359 plus the existing `flowchart-elk` layout failure. Targeted fresh probes now pass for the
    Math fixture, representative special-shape class/click fixtures, edge markdown labels, cluster
    id scoping, and root-first `htmlLabels` precedence. See
    `docs/workstreams/flowchart-11-15-svg-convergence/EVIDENCE_AND_GATES.md`.
  - Later child-lane slices brought the supported Flowchart fresh corpus to zero canonical XML
    mismatches. `flowchart-elk` is explicitly out of the current headless support matrix and is
    narrowly skipped in `compare-svg-xml` with a local-policy reason until a dedicated ELK layout
    lane lands.
  - `cargo run -p xtask -- compare-svg-xml --check --diagram flowchart --upstream-root target/upstream-svgs-11-15-flowchart --dom-mode parity --dom-decimals 3`:
    passed. `target/compare/xml/xml_report.md` reports `Mismatches (0)` and one documented
    `flowchart-elk` skip.
  - Stored Flowchart baselines were refreshed to Mermaid 11.15: 1069 SVG files changed and 4 stale
    parser-only KaTeX SVG baselines were removed because upstream Mermaid 11.15 no longer
    regenerates them.
  - `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3`:
    passed; `target/compare/flowchart_report.md` reports `All fixtures matched` plus the documented
    `flowchart-elk` skip.
  - `cargo run -p xtask -- compare-svg-xml --check --diagram flowchart --dom-mode parity --dom-decimals 3`:
    passed; `target/compare/xml/xml_report.md` reports `Mismatches (0)` plus the documented
    `flowchart-elk` skip.
  - `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`:
    failed only for the current non-Flowchart remainder: ER has 1 DOM mismatch and Class has 14 DOM
    mismatches.

## Evidence Anchors

- `docs/workstreams/mermaid-11-15-complete-adaptation/DESIGN.md`
- `docs/workstreams/mermaid-11-15-complete-adaptation/TODO.md`
- `docs/workstreams/mermaid-11-15-complete-adaptation/PARITY_FAILURE_INVENTORY.md`
- `docs/workstreams/flowchart-11-15-svg-convergence/`
- `docs/alignment/STATUS.md`
- `docs/rendering/UPSTREAM_SVG_BASELINES.md`
- `tools/upstreams/REPOS.lock.json`
- `target/compare/*_report_parity.md`
