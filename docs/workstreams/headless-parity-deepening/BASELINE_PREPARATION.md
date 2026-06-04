# Headless Parity Baseline Preparation

Status: Active
Last updated: 2026-06-04

## Purpose

This document is the pre-parity baseline readiness plan for the post-Mermaid `11.15.0` lane. The
goal is to make the baseline corpus, generated artifacts, official fixture intake, and readiness
gates trustworthy before spending more work on fine parity fixes.

## Baseline Authority

- Active Mermaid baseline: `mermaid@11.15.0`.
- Source authority:
  - `crates/merman-core/src/baseline.rs`
  - `tools/upstreams/REPOS.lock.json`
  - `docs/adr/0001-upstream-baseline.md`
- Generated file names that still include `11_12_2` are legacy provenance unless a current report,
  renderer output, fixture, or test presents them as active baseline truth.

## Decision

Do not blindly refresh every stored SVG in one unreviewable change. First run a generated-baseline
check, classify the diffs, then refresh by family or by clearly mechanical batch.

Use this order:

1. Current-facing stale outputs and live reports.
2. Stored upstream SVG baselines.
3. Layout goldens and semantic snapshots affected by the same baseline change.
4. Official fixture/test intake.
5. Structural and renderability readiness gates.
6. Only then resume parity/root residual fixes.

## Inventory Snapshot

Current scan results:

- `22` generated Rust files still use the historical `11_12_2` suffix. Treat these as compatibility
  and provenance until a generator or module rename can be done as one mechanical slice.
- `61` `docs/alignment/*.md` files still mention `11.12.x`. Most are historical alignment notes,
  but any minimum/status/current-coverage page that claims the active baseline must be updated or
  explicitly marked historical.
- Current-facing stale surfaces found so far:
  - `crates/merman-render/src/info.rs` rendered `v11.12.2` as visible Info output. Fixed in the
    first baseline preparation slice.
  - Info layout goldens rendered `v11.12.2` and stored upstream SVGs rendered `v11.12.3`. Refreshed
    to `v11.15.0`.
  - `crates/merman-render/src/error.rs` previously rendered Mermaid `11.12.3` in the error
    diagram.
    Fixed by routing the visible error version text through `PINNED_MERMAID_BASELINE_VERSION`.
  - `xtask` import/audit reports previously printed `Mermaid@11.12.3` in some generated report
    headers.
    Fixed by routing gap audit, upstream import report headers, and the Cypress upstream-root
    diagnostic through `pinned_mermaid_baseline_label(...)`.
  - Some internal comments and helper names still mention `11.12.x`. Keep source-port comments when
    they are historical evidence; rename only when they mislabel current behavior.

Generated upstream SVG check inventory from `2026-06-04`:

- Command shape:
  `$env:PUPPETEER_EXECUTABLE_PATH='C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'; .\target\debug\xtask.exe check-upstream-svgs --diagram <family> --check-dom --dom-mode structure --dom-decimals 3`
- Logs:
  - `target/hpd090-baseline-check/<family>.log`
  - `target/hpd090-baseline-check/flowchart-slices/*.log`
- A single all-family run was too coarse for this corpus on Windows and previously left a stuck
  `xtask.exe`; the inventory below was collected by family and by Flowchart prefix slices.

| Family | Result | Stored / Generated | Recommendation |
| --- | --- | ---: | --- |
| `architecture` | pass | `185 / 185` | Keep. |
| `block` | refreshed + DOM parity fixed | `119 / 119` | Keep refreshed; local renderer now matches Mermaid 11.15 current DOM under the parity gate. |
| `c4` | pass | `51 / 51` | Keep. |
| `class` | refreshed + DOM parity fixed | `246 / 246`, `2` diffs | Keep refreshed; local native SVG label wrapping now matches Mermaid 11.15 under the parity gate. |
| `er` | pass | `101 / 101` | Keep. |
| `flowchart` | stale, narrow | `1074 / 1074`, `4` diffs | Point refresh the four HTML demo KaTeX fixtures. |
| `gantt` | refreshed + DOM parity fixed | `151 / 151`, `137` diffs | Keep refreshed; local renderer now matches Mermaid 11.15 current DOM under the parity gate. |
| `gitgraph` | pass under structure gate | `252 / 252` | Keep; textual diff exists, but no structure failure was logged. |
| `info` | pass after refresh | `15 / 15` | Keep. |
| `journey` | pass | `26 / 26` | Keep. |
| `kanban` | refreshed + DOM parity fixed | `87 / 87` | Keep refreshed; local renderer now matches Mermaid 11.15 current DOM under the parity gate. |
| `mindmap` | refreshed + DOM parity fixed | `114 / 114` | Keep refreshed; local renderer now matches Mermaid 11.15 current DOM under the parity gate. |
| `packet` | pass | `33 / 33` | Keep. |
| `pie` | pass | `69 / 69` | Keep. |
| `quadrantchart` | pass | `59 / 59` | Keep. |
| `radar` | refreshed + DOM parity fixed | `41 / 41` | Keep refreshed; local renderer now matches Mermaid 11.15 current root DOM under the parity gate. |
| `requirement` | refreshed + DOM parity fixed | `47 / 47` | Keep refreshed; local renderer now matches Mermaid 11.15 current DOM under the parity gate. |
| `sankey` | pass | `33 / 33` | Keep. |
| `sequence` | pass under structure gate | `322 / 321` | Keep; `stress_end_keyword_016` is a known skipped upstream SVG check fixture. |
| `state` | pass under structure gate | `285 / 285` | Keep; textual diff exists, but no structure failure was logged. |
| `timeline` | stale, narrow | `91 / 91`, `1` diff | Point refresh `upstream_cypress_timeline_spec_12_should_render_timeline_with_proper_vertical_line_lengths_for_012`. |
| `treemap` | pass | `54 / 54` | Keep. |
| `xychart` | pass | `71 / 71` | Keep. |

Decision: do not refresh all stored upstream SVGs. The broad stale family set and Class narrow set
have been refreshed; point-refresh the remaining two narrow stale sets, then run the readiness gates
against the refreshed corpus.

## First Slice Completed

Info baseline hygiene:

- `layout_info_diagram_typed(...)` now formats the visible version from
  `PINNED_MERMAID_BASELINE_VERSION`.
- All `fixtures/info/*.layout.golden.json` now contain `v11.15.0`.
- All `fixtures/upstream-svgs/info/*.svg` now contain `v11.15.0`.
- Upstream SVG generation required `PUPPETEER_EXECUTABLE_PATH` pointing to local Edge because the
  default Puppeteer Chrome cache was missing.

Verification:

- `cargo run -p xtask -- compare-info-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\info_report_parity_after_baseline_hygiene.md`
- `cargo nextest run -p merman-render --test layout_snapshots_test fixtures_match_layout_golden_snapshots_when_present`
- `cargo nextest run -p merman --features render --test resvg_safe_fixture_smoke boundary_fixtures_render_headless_resvg_safe`

## Second Slice Completed

Live baseline text hygiene and inventory:

- Error diagram visible version text now formats from `PINNED_MERMAID_BASELINE_VERSION`.
- `xtask` gap audit and upstream import report headers now print `Mermaid@11.15.0` through the
  pinned baseline label helper instead of hardcoding `Mermaid@11.12.3`.
- The Cypress upstream-root diagnostic now reports the pinned Mermaid checkout label.
- Per-family upstream SVG checks classified the stored baseline refresh set.

Verification:

- `cargo fmt --all --check`
- `cargo nextest run -p xtask pinned_mermaid_baseline_label_reads_lockfile_ref`
- `cargo nextest run -p merman --features render --test resvg_safe_fixture_smoke boundary_fixtures_render_headless_resvg_safe`
- Per-family `check-upstream-svgs` runs listed in the inventory logs above.

## Third Slice Completed

Requirement baseline refresh plus renderer DOM parity:

- All `fixtures/upstream-svgs/requirement/*.svg` were refreshed to the pinned Mermaid 11.15
  baseline.
- Requirement SVG output now emits the current Mermaid 11.15 DOM surfaces:
  - default `data-look="classic"` instead of omitting the look;
  - diagram-prefixed node and edge DOM ids;
  - `class="node ..."` ordering for node groups;
  - `outer-path` requirement boxes and `divider` groups for classic output;
  - root-level drop-shadow filter defs appended by Mermaid's generic renderer path.
- `constructor` remains an allowed rendered id for the prototype-like fixture; only `__proto__`
  stays suppressed by the layout/render safety path.
- Default exact SVG byte comparison still reports textual mismatch for Requirement because
  Mermaid/RoughJS path data and serialization churn are not the current family gate. The parity
  gate is DOM comparison with `--check-dom --dom-mode parity --dom-decimals 3`.

Verification:

- `cargo run -p xtask -- compare-requirement-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo nextest run -p merman --features render --test theme_renderability_smoke requirement_theme_smoke_counts_dom_consumed_neo_and_edge_signals`
- `cargo nextest run -p merman --features render --test resvg_safe_fixture_smoke boundary_fixtures_render_headless_resvg_safe`
- `cargo fmt --all --check`

## Fourth Slice Completed

Block baseline refresh plus renderer DOM/layout parity:

- All `fixtures/upstream-svgs/block/*.svg` were refreshed to the pinned Mermaid 11.15 baseline.
- All `fixtures/block/*.layout.golden.json` were refreshed after moving Block label height
  measurement to current HTML-like `line-height: 1.5` semantics.
- Block SVG output now emits the current Mermaid 11.15 DOM surfaces needed by the family gate:
  - diagram-prefixed node and edge DOM ids;
  - node and edge labels rendered through XHTML `<p>` children;
  - blank placeholder labels keep Mermaid's paragraph child while still measuring as empty;
  - node label containers use `display: table-cell` with `line-height: 1.5`;
  - edge paths carry the repeated Mermaid thickness/pattern class tokens.
- The legacy generated width override table remains for width parity, but its stale height
  overrides are no longer used.

Verification:

- `cargo run -p xtask -- compare-block-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\block_report_parity_hpd090_after_label_fix.md`
- `cargo nextest run -p merman-render --test block_svg_test`
- `cargo nextest run -p merman-render --test layout_snapshots_test fixtures_match_layout_golden_snapshots_when_present`
- `cargo fmt --all --check`

## Fifth Slice Completed

Gantt baseline refresh plus renderer DOM parity:

- `fixtures/upstream-svgs/gantt/*.svg` were regenerated to the pinned Mermaid 11.15 baseline; `137`
  of the `151` stored SVGs changed.
- `cargo run -p xtask -- update-layout-snapshots --diagram gantt` added the missing
  `fixtures/gantt/zed_pr_57644_gantt.layout.golden.json` snapshot.
- Gantt SVG output now emits the current Mermaid 11.15 DOM ids for:
  - excluded date range rectangles;
  - task bar rectangles;
  - task label text nodes.
- Prototype-like task ids such as `__proto__` and `constructor` are now safe because the emitted DOM
  id is diagram-prefixed.
- The Gantt family gate is green under DOM comparison with
  `--check-dom --dom-mode parity --dom-decimals 3`.

Verification:

- `cargo run -p xtask -- compare-gantt-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\gantt_report_parity_hpd090_after_id_fix.md`
- `cargo nextest run -p merman-render --test svg_internal_id_test`
- `cargo nextest run -p merman-render --test layout_snapshots_test fixtures_match_layout_golden_snapshots_when_present`
- `cargo nextest run -p merman --features render --test theme_renderability_smoke gantt_theme_smoke_counts_normal_and_done_task_dom_as_visible`
- `cargo fmt --all --check`

## Sixth Slice Completed

Kanban baseline refresh plus renderer DOM parity:

- `fixtures/upstream-svgs/kanban/*.svg` were regenerated to the pinned Mermaid 11.15 baseline; all
  `87` stored SVGs changed.
- Kanban SVG output now emits current Mermaid 11.15 DOM surfaces for item/section ids and item
  title labels:
  - section and item group DOM ids are diagram-prefixed;
  - prototype-like item ids such as `__proto__` and `constructor` are renderable because the
    emitted DOM id is diagram-prefixed;
  - item title XHTML labels carry `nodeLabel markdown-node-label`, while section labels, tickets,
    assigned labels, and empty placeholders keep the older `nodeLabel` class.
- No layout golden changed in this slice; `update-layout-snapshots --diagram kanban` had no
  additional committed output.
- The Kanban family gate is green under DOM comparison with
  `--check-dom --dom-mode parity --dom-decimals 3`.

Verification:

- `cargo nextest run -p merman-render kanban_dom_ids_are_scoped_by_diagram_id`
- `cargo run -p xtask -- compare-kanban-svgs --check-dom --dom-mode parity --dom-decimals 3`

## Seventh Slice Completed

Mindmap baseline refresh plus renderer DOM parity:

- `fixtures/upstream-svgs/mindmap/*.svg` were regenerated to the pinned Mermaid 11.15 baseline; all
  `114` stored SVGs changed.
- `cargo run -p xtask -- update-layout-snapshots --diagram mindmap` added the missing
  `fixtures/mindmap/zed_pr_57644_mindmap.layout.golden.json` snapshot.
- Mindmap was not a pure stored-SVG refresh. Local SVG output now emits current Mermaid 11.15 DOM
  surfaces needed by the family gate:
  - default/classic nodes and edges explicitly carry `data-look="classic"`;
  - node group ids, default rounded path ids, and edge path ids are diagram-prefixed, while edge
    `data-id` keeps the raw business id;
  - family-level drop-shadow filter defs and Mindmap margin markers are present;
  - node and edge section classes wrap after Mermaid's `0..10` palette cycle instead of emitting
    stale `section-11` style tokens;
  - classic rounded and hexagon nodes render as direct `rect` / `polygon` DOM instead of the old
    rough-wrapper structure;
  - XHTML node labels keep the current `nodeLabel markdown-node-label` class tokens.
- The old local `roughjs46` compatibility helper became unused after the classic shape correction
  and was removed.
- The Mindmap family gate is green under DOM comparison with
  `--check-dom --dom-mode parity --dom-decimals 3`.

Verification:

- `cargo nextest run -p merman-render --test mindmap_svg_test`
- `cargo nextest run -p merman --features render --test theme_renderability_smoke mindmap`
- `cargo run -p xtask -- compare-mindmap-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo nextest run -p merman-render --test layout_snapshots_test fixtures_match_layout_golden_snapshots_when_present`
- `cargo fmt --check -p merman-render -p merman`

## Eighth Slice Completed

Radar baseline refresh plus root DOM parity:

- `fixtures/upstream-svgs/radar/*.svg` were regenerated to the pinned Mermaid 11.15 baseline; all
  `41` stored SVGs changed.
- Radar was not a pure stored-SVG refresh. Mermaid 11.15 Radar roots now emit responsive
  `width="100%"`, omit the fixed root `height`, and carry `max-width: <width>px` in the root
  `style` attribute.
- Local Radar SVG output now uses the current root attribute shape, and the stale
  root-helper `AfterViewBox` fixed-height placement branch was removed after Radar stopped using
  it.
- No layout golden changed in this slice; `update-layout-snapshots --diagram radar` produced no
  committed output.
- The Radar family gate is green under DOM comparison with
  `--check-dom --dom-mode parity --dom-decimals 3`.

Verification:

- `cargo fmt -p merman-render --check`
- `cargo nextest run -p merman-render radar`
- `cargo run -p xtask -- update-layout-snapshots --diagram radar`
- `cargo run -p xtask -- compare-radar-svgs --check-dom --dom-mode parity --dom-decimals 3`

## Ninth Slice Completed

Class baseline refresh plus native SVG label wrapping parity:

- Point-refreshed the two stale `fixtures/upstream-svgs/class/*.svg` files:
  `stress_class_svg_font_size_px_string_precedence_026` and `upstream_parser_class_spec`.
- Class was not a pure stored-SVG refresh. In Mermaid 11.15, the native SVG-label path can wrap
  `htmlLabels=false` labels by using a smaller top-level `fontSize` as the width probe while the
  final SVG text inherits a larger explicit `themeVariables.fontSize` px value.
- Local Class layout and SVG rendering now keep the `: String` type suffix in the same outer
  `tspan` row for that source-backed case instead of splitting `String` into a third row.
- Refreshed the affected `stress_class_svg_font_size_px_string_precedence_026.layout.golden.json`
  snapshot and added the missing `zed_pr_57644_class.layout.golden.json` snapshot for an existing
  Class fixture.
- The Class family gate is green under DOM comparison with
  `--check-dom --dom-mode parity --dom-decimals 3`.

Verification:

- `cargo nextest run -p merman-render --test class_svg_test`
- `cargo run -p xtask -- update-layout-snapshots --diagram class`
- `cargo nextest run -p merman-render --test layout_snapshots_test fixtures_match_layout_golden_snapshots_when_present`
- `cargo run -p xtask -- compare-class-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\class_report_parity_hpd090_after_wrap_fix.md`
- `cargo fmt -p merman-render --check`

## Refresh Policy

Before refreshing all stored SVGs:

1. Run:
   - `cargo run -p xtask -- check-upstream-svgs --diagram all --check-dom --dom-mode structure --dom-decimals 3`
2. If the diff set is small or family-local, refresh by family.
3. If the diff set is broad but mechanical, refresh all in one baseline commit only after:
   - the generated check explains why broad churn is expected;
   - `compare-all-svgs --dom-mode parity` stays green after local code/golden updates;
   - the diff is mostly stored upstream SVG text/CSS/version drift, not mixed renderer changes.
4. If a family has stochastic or environment-sensitive output, keep the family-specific
   check/generation policy from `docs/rendering/UPSTREAM_SVG_BASELINES.md`.

Preferred family refresh command shape:

```powershell
$env:PUPPETEER_EXECUTABLE_PATH='C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'
cargo run -p xtask -- gen-upstream-svgs --diagram <family>
cargo run -p xtask -- update-layout-snapshots --diagram <family>
cargo run -p xtask -- compare-<family>-svgs --check-dom --dom-mode parity --dom-decimals 3
```

## Official Fixture And Test Intake

Only supplement official fixtures when the added fixture improves one of these:

- source coverage for a supported implemented family;
- baseline refresh confidence for a family whose stored SVGs drifted;
- visible renderability coverage that DOM parity does not catch;
- parser-only boundary documentation for upstream examples that Mermaid CLI cannot render.

Admission requirements:

- Fixture source is traceable to pinned Mermaid `11.15.0` docs, package tests, examples, Cypress,
  or HTML demos.
- If renderable by the pinned CLI, it gets:
  - `.mmd`
  - `.golden.json`
  - `.layout.golden.json`
  - `fixtures/upstream-svgs/<family>/*.svg`
- If not renderable by the pinned CLI, it is explicitly parser-only or deferred with a report
  reason. Do not fake an upstream SVG.
- For visible rendering risk, add or extend a public renderability smoke only when the assertion
  corresponds to DOM that the current renderer actually emits.

Do not use this baseline preparation phase to admit new unsupported families. `treeView`,
`ishikawa`, `eventmodeling`, `venn`, and `wardley` need separate family-admission workstreams.

## Readiness Gate

Baseline preparation is ready to hand back to parity work when these pass or have documented
expected diagnostics:

- `cargo fmt --check`
- `git diff --check`
- `cargo run -p xtask -- check-upstream-svgs --diagram all --check-dom --dom-mode structure --dom-decimals 3`
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo nextest run -p merman-render --test layout_snapshots_test`
- `cargo nextest run -p merman --features render --test resvg_safe_fixture_smoke boundary_fixtures_render_headless_resvg_safe`
- Filtered raster `resvg_safe` audits for any family whose stored SVGs or renderability fixtures
  changed.

## Next Actions

1. Point-refresh the remaining narrow stale sets: `timeline` (`1` fixture) and Flowchart HTML demo
   KaTeX fixtures (`4` fixtures).
2. Update affected layout snapshots and run family compare gates after each refresh batch.
3. Re-run the readiness gates. Do not run a broad official fixture import yet; the current
   inventory points to stored-baseline drift in existing fixtures, not missing fixture intake.
