# Headless Parity Deepening - Evidence And Gates

Status: Active
Last updated: 2026-06-07

## HPD-090 - Baseline Preparation, Info Refresh, And Inventory

Outcome:

- Added `BASELINE_PREPARATION.md` as the pre-parity baseline readiness plan.
- Decided to classify all-family upstream baseline diffs before doing a broad stored SVG refresh.
- Fixed the first current-facing stale baseline surface:
  - `layout_info_diagram_typed(...)` now formats visible Info output from
    `PINNED_MERMAID_BASELINE_VERSION`.
  - all `fixtures/info/*.layout.golden.json` now contain `v11.15.0`;
  - all `fixtures/upstream-svgs/info/*.svg` now contain `v11.15.0`.
- Fixed the next current-facing stale baseline surfaces:
  - error diagram visible version text now reads `PINNED_MERMAID_BASELINE_VERSION`;
  - `xtask` gap audit, upstream import report headers, and Cypress upstream-root diagnostics now
    route through `pinned_mermaid_baseline_label(...)`.
- Classified stored upstream SVG drift by family. Broad stale families were `block`, `gantt`,
  `kanban`, `mindmap`, `radar`, and `requirement`. Narrow stale sets are `class` (`2` fixtures),
  `timeline` (`1` fixture), and Flowchart HTML demo KaTeX fixtures (`4` fixtures). The rest passed
  the structure gate or only had non-gating textual churn.
- Refreshed the Requirement stored upstream SVG family and fixed the local Requirement SVG renderer
  to match Mermaid 11.15 current DOM under the parity gate:
  - default and themed output now emits `data-look`;
  - node and edge DOM ids are diagram-prefixed while edge `data-id` remains the raw relationship id;
  - node class ordering, `outer-path`, divider groups, and root drop-shadow defs match the generic
    Mermaid 11.15 render path;
  - the prototype-like `constructor` id remains renderable while `__proto__` stays suppressed.
- Refreshed the Block stored upstream SVG family and fixed the local Block renderer/layout to match
  Mermaid 11.15 current DOM under the parity gate:
  - labels now use current XHTML paragraph children and `display: table-cell; line-height: 1.5`;
  - blank placeholder labels keep the paragraph child in the DOM while still measuring as empty;
  - node and edge DOM ids are diagram-prefixed;
  - edge paths carry Mermaid 11.15's repeated thickness/pattern class tokens;
  - Block layout goldens now reflect current HTML-like label height measurement instead of stale
    generated 11.12 height overrides.
- Refreshed the Gantt stored upstream SVG family and fixed the local Gantt renderer to match
  Mermaid 11.15 current DOM under the parity gate:
  - excluded date range rectangles, task bar rectangles, and task label text nodes now use
    diagram-prefixed DOM ids;
  - prototype-like task ids such as `__proto__` and `constructor` remain renderable because the
    emitted DOM id is diagram-prefixed;
  - Gantt layout snapshot refresh added the missing `zed_pr_57644_gantt` layout golden.
- Refreshed the Kanban stored upstream SVG family and fixed the local Kanban renderer to match
  Mermaid 11.15 current DOM under the parity gate:
  - section and item group DOM ids now use the diagram-prefixed id shape;
  - prototype-like item ids such as `__proto__` and `constructor` remain renderable because the
    emitted DOM id is diagram-prefixed;
  - item title XHTML labels now carry `nodeLabel markdown-node-label`, while section labels,
    ticket/assigned labels, and empty placeholders keep the older `nodeLabel` class.
- Refreshed the Mindmap stored upstream SVG family and fixed the local Mindmap renderer to match
  Mermaid 11.15 current DOM under the parity gate:
  - classic nodes and edges now explicitly emit `data-look="classic"`;
  - node group ids, default rounded path ids, and edge path ids are diagram-prefixed while edge
    `data-id` remains raw;
  - root drop-shadow defs and Mindmap margin markers are present;
  - node and edge section classes wrap through Mermaid's `0..10` palette cycle;
  - classic rounded and hexagon nodes use direct `rect` / `polygon` DOM instead of the stale
    rough-wrapper structure.
- Refreshed the Radar stored upstream SVG family and fixed the local Radar root DOM to match
  Mermaid 11.15 under the parity gate:
  - all `41` stored Radar upstream SVGs now carry Mermaid 11.15 root attributes;
  - local Radar roots now emit responsive `width="100%"`;
  - the fixed root `height` attribute is no longer emitted;
  - root `style` now carries `max-width: <width>px; background-color: white;`;
  - the now-unused root-helper fixed-height `AfterViewBox` branch was removed.
- Point-refreshed the Class narrow stale set and fixed the local Class native SVG-label wrapping to
  match Mermaid 11.15 under the parity gate:
  - the two stale stored Class upstream SVGs were regenerated;
  - `htmlLabels=false` labels with a smaller top-level `fontSize` probe and a larger explicit
    `themeVariables.fontSize` px value now keep the `: String` type suffix in the second outer
    `tspan` instead of splitting `String` into a third row;
  - the affected 026 layout golden was refreshed and the missing existing-fixture
    `zed_pr_57644_class` layout golden was added.
- Point-refreshed the Timeline narrow stale set and fixed the local Timeline layout measurement to
  match Mermaid 11.15's baseline browser fallback under the parity gate:
  - the stale stored Timeline upstream SVG was regenerated;
  - the fixture's bare `Fira Sans` / `17px` text resolves to browser sans-serif fallback metrics in
    the Edge/Puppeteer baseline environment;
  - local Timeline wrap probes now use the matching sans-serif metrics for that bare
    `Fira Sans` case, and the first-line bbox height uses the observed `25px` browser lattice for
    the same `17px` case;
  - the affected Timeline layout golden was refreshed and the missing existing-fixture
    `zed_pr_57644_timeline` layout golden was added.
- Point-refreshed the Flowchart HTML demo KaTeX narrow stale set:
  - the four stale stored Flowchart upstream SVGs were regenerated;
  - the refreshed baselines now carry the current Mermaid 11.15 `_katex` diagram id suffix,
    Flowchart marker margin defs, `data-look="classic"` DOM, `1px` shared edge width,
    neo/drop-shadow CSS rules, and current KaTeX MathML measurement output;
  - local Flowchart output already matched the refreshed baselines under full Flowchart DOM
    parity;
  - `update-layout-snapshots --diagram flowchart` added the missing existing-fixture
    `zed_pr_57644_flowchart` layout golden.
- Closed HPD-090 after readiness revalidation:
  - no broad or narrow stale stored-SVG set remains known;
  - no broad official fixture import is indicated by the current inventory;
  - `compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3` is green for the implemented
    matrix;
  - future work should continue through HPD-080 for fresh visible renderability defects or HPD-050
    for source-backed Architecture/Dagre/Graphlib audits.
- Follow-up test hygiene aligned the HPD-080 raster missing-font regression's synthetic visible
  version text with `PINNED_MERMAID_BASELINE_VERSION`. This removed a stale `v11.12.2` literal from
  current tests without changing renderer behavior, fixtures, stored SVG baselines, or HPD-090
  closeout scope.
- The first attempt to regenerate Info upstream SVGs failed because Puppeteer could not find its
  cached Chrome. The successful run set `PUPPETEER_EXECUTABLE_PATH` to local Microsoft Edge.

Touched surfaces:

- `docs/workstreams/headless-parity-deepening/BASELINE_PREPARATION.md`
- `crates/merman-render/src/info.rs`
- `crates/merman-render/src/error.rs`
- `crates/merman-render/src/svg/parity/requirement.rs`
- `crates/merman-render/src/block.rs`
- `crates/merman-render/src/svg/parity/block.rs`
- `crates/merman-render/src/svg/parity/gantt.rs`
- `crates/merman-render/src/svg/parity/kanban.rs`
- `crates/merman-render/src/svg/parity/mindmap.rs`
- `crates/merman-render/src/svg/parity/radar.rs`
- `crates/merman-render/src/svg/parity/root_svg.rs`
- `crates/merman-render/src/class.rs`
- `crates/merman-render/src/timeline.rs`
- `crates/merman-render/src/svg/parity/class/label.rs`
- `crates/merman-render/src/svg/parity/roughjs46.rs`
- `crates/merman-render/tests/block_svg_test.rs`
- `crates/merman-render/tests/class_svg_test.rs`
- `crates/merman-render/tests/mindmap_svg_test.rs`
- `crates/merman-render/tests/svg_internal_id_test.rs`
- `crates/merman/tests/theme_renderability_smoke.rs`
- `crates/xtask/src/cmd/audit.rs`
- `crates/xtask/src/cmd/import/{cypress,docs,examples}.rs`
- `crates/merman/src/render/raster.rs`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-06-hpd-090-raster-baseline-test-hygiene.md`
- `fixtures/info/*.layout.golden.json`
- `fixtures/block/*.layout.golden.json`
- `fixtures/class/stress_class_svg_font_size_px_string_precedence_026.layout.golden.json`
- `fixtures/class/zed_pr_57644_class.layout.golden.json`
- `fixtures/timeline/upstream_cypress_timeline_spec_12_should_render_timeline_with_proper_vertical_line_lengths_for_012.layout.golden.json`
- `fixtures/timeline/zed_pr_57644_timeline.layout.golden.json`
- `fixtures/flowchart/zed_pr_57644_flowchart.layout.golden.json`
- `fixtures/gantt/zed_pr_57644_gantt.layout.golden.json`
- `fixtures/mindmap/zed_pr_57644_mindmap.layout.golden.json`
- `fixtures/upstream-svgs/info/*.svg`
- `fixtures/upstream-svgs/block/*.svg`
- `fixtures/upstream-svgs/class/{stress_class_svg_font_size_px_string_precedence_026,upstream_parser_class_spec}.svg`
- `fixtures/upstream-svgs/gantt/*.svg`
- `fixtures/upstream-svgs/kanban/*.svg`
- `fixtures/upstream-svgs/mindmap/*.svg`
- `fixtures/upstream-svgs/radar/*.svg`
- `fixtures/upstream-svgs/requirement/*.svg`
- `fixtures/upstream-svgs/timeline/upstream_cypress_timeline_spec_12_should_render_timeline_with_proper_vertical_line_lengths_for_012.svg`
- `fixtures/upstream-svgs/flowchart/upstream_html_demos_flowchart_flowchart_{040,042,044}_katex.svg`
- `fixtures/upstream-svgs/flowchart/upstream_html_demos_flowchart_graph_039_katex.svg`
- `target/hpd090-baseline-check/*.log`
- `target/hpd090-baseline-check/flowchart-slices/*.log`

Focused verification:

- `cargo fmt -p merman-render` - passed.
- `cargo run -p xtask -- update-layout-snapshots --diagram info` - passed.
- `$env:PUPPETEER_EXECUTABLE_PATH='C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'; cargo run -p xtask -- gen-upstream-svgs --diagram info` -
  passed.
- `cargo run -p xtask -- compare-info-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\info_report_parity_after_baseline_hygiene.md` -
  passed.
- `cargo nextest run -p merman-render --test layout_snapshots_test fixtures_match_layout_golden_snapshots_when_present` -
  passed, `1` test run.
- `cargo nextest run -p merman --features render --test resvg_safe_fixture_smoke boundary_fixtures_render_headless_resvg_safe` -
  passed, `1` test run.
- `cargo fmt --all --check` - passed.
- `cargo nextest run -p xtask pinned_mermaid_baseline_label_reads_lockfile_ref` - passed, `1`
  test run.
- `cargo run -p xtask -- compare-requirement-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed after the Requirement renderer DOM update.
- `cargo run -p xtask -- compare-block-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\block_report_parity_hpd090_after_label_fix.md` -
  passed after the Block renderer/layout update.
- `cargo nextest run -p merman-render --test block_svg_test` - passed, `6` tests run.
- `cargo nextest run -p merman-render --test layout_snapshots_test fixtures_match_layout_golden_snapshots_when_present` -
  passed, `1` test run after the Block layout golden refresh.
- `cargo run -p xtask -- compare-gantt-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\gantt_report_parity_hpd090_after_id_fix.md` -
  passed after the Gantt renderer DOM id update.
- `cargo nextest run -p merman-render --test svg_internal_id_test` - passed, `6` tests run.
- `cargo nextest run -p merman --features render --test theme_renderability_smoke gantt_theme_smoke_counts_normal_and_done_task_dom_as_visible` -
  passed, `1` test run.
- `cargo nextest run -p merman-render kanban_dom_ids_are_scoped_by_diagram_id` - passed, `1`
  test run.
- `cargo run -p xtask -- compare-kanban-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed after the Kanban renderer DOM id/title-label update.
- `cargo nextest run -p merman-render --test mindmap_svg_test` - passed, `3` tests run.
- `cargo nextest run -p merman --features render --test theme_renderability_smoke mindmap` -
  passed, `2` tests run.
- `cargo run -p xtask -- compare-mindmap-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed after the Mindmap renderer DOM update.
- `cargo nextest run -p merman-render --test layout_snapshots_test fixtures_match_layout_golden_snapshots_when_present` -
  passed, `1` test run after the Mindmap layout golden refresh.
- `cargo fmt --check -p merman-render -p merman` - passed.
- `cargo fmt -p merman-render --check` - passed after the Radar root update.
- `cargo nextest run -p merman-render radar` - passed, `3` tests run.
- `cargo run -p xtask -- update-layout-snapshots --diagram radar` - passed with no committed
  layout snapshot changes.
- `cargo run -p xtask -- compare-radar-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed after the Radar root DOM update.
- `cargo nextest run -p merman-render --test class_svg_test` - passed, `21` tests run.
- `cargo run -p xtask -- update-layout-snapshots --diagram class` - passed and produced the Class
  layout golden updates listed above.
- `cargo nextest run -p merman-render --test layout_snapshots_test fixtures_match_layout_golden_snapshots_when_present` -
  passed, `1` test run after the Class layout golden refresh.
- `cargo run -p xtask -- compare-class-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\class_report_parity_hpd090_after_wrap_fix.md` -
  passed after the Class wrapping update and stored SVG refresh.
- `cargo fmt -p merman-render --check` - passed after the Class wrapping update.
- `cargo nextest run -p merman-render fira_sans_17_timeline_metrics_match_mermaid_browser_wrap` -
  passed, `1` test run after the Timeline browser-fallback measurement update.
- `cargo nextest run -p merman-render --test timeline_svg_test` - passed after the Timeline
  measurement update.
- `cargo run -p xtask -- update-layout-snapshots --diagram timeline` - passed and produced the
  Timeline layout golden updates listed above.
- `cargo nextest run -p merman-render --test layout_snapshots_test fixtures_match_layout_golden_snapshots_when_present` -
  passed, `1` test run after the Timeline layout golden refresh.
- `cargo run -p xtask -- compare-timeline-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\timeline_report_parity_hpd090_after_fira_sans_measurement.md` -
  passed after the Timeline measurement update and stored SVG refresh.
- `cargo fmt -p merman-render --check` - passed after the Timeline measurement update.
- `$env:PUPPETEER_EXECUTABLE_PATH='C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'; cargo run -p xtask -- check-upstream-svgs --diagram flowchart --filter upstream_html_demos_flowchart --check-dom --dom-mode structure --dom-decimals 3` -
  passed after point-refreshing the Flowchart HTML demo KaTeX stored SVGs.
- `cargo run -p xtask -- compare-flowchart-svgs --filter upstream_html_demos_flowchart --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\flowchart_report_parity_hpd090_html_katex.md` -
  passed for the refreshed Flowchart HTML demo slice; the known ELK fixture remains skipped as a
  documented unsupported layout.
- `cargo run -p xtask -- update-layout-snapshots --diagram flowchart` - passed and produced the
  missing `zed_pr_57644_flowchart` layout golden.
- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\flowchart_report_parity_hpd090_html_katex_full.md` -
  passed for the full Flowchart family under DOM parity.
- `cargo nextest run -p merman-render --test layout_snapshots_test fixtures_match_layout_golden_snapshots_when_present` -
  passed, `1` test run after the Flowchart layout golden addition.
- Closeout `cargo fmt --check` - passed.
- Closeout `cargo nextest run -p merman-render --test layout_snapshots_test fixtures_match_layout_golden_snapshots_when_present` -
  passed, `1` test run.
- Closeout `cargo nextest run -p merman --features render --test resvg_safe_fixture_smoke boundary_fixtures_render_headless_resvg_safe` -
  passed, `1` test run and `5` skipped.
- Closeout `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed for er, flowchart, state, class, sequence, info, pie, sankey, packet, timeline, journey,
  kanban, gitgraph, gantt, c4, block, radar, requirement, mindmap, architecture, quadrantchart,
  treemap, and xychart.
- Closeout JSON/JSONL validation for `TASKS.jsonl`, `CONTEXT.jsonl`, `CAMPAIGNS.jsonl`, and
  `WORKSTREAM.json` - passed.
- Closeout `git diff --check` - passed; Git only reported the existing JSONL LF-to-CRLF working
  copy warnings.
- `cargo nextest run -p merman --features render --test theme_renderability_smoke requirement_theme_smoke_counts_dom_consumed_neo_and_edge_signals` -
  passed, `1` test run.
- `cargo nextest run -p merman --features render --test resvg_safe_fixture_smoke boundary_fixtures_render_headless_resvg_safe` -
  passed, `1` test run.
- JSON/JSONL validation for `TASKS.jsonl`, `CONTEXT.jsonl`, `CAMPAIGNS.jsonl`, and
  `WORKSTREAM.json` - passed.
- `git diff --check` - passed; Git only reported the existing JSONL CRLF conversion warnings.
- Per-family and Flowchart-prefix `check-upstream-svgs --check-dom --dom-mode structure` runs wrote
  the inventory logs under `target/hpd090-baseline-check/`.
- Follow-up raster baseline test hygiene verification:
  - `cargo fmt --check -p merman` - passed.
  - `cargo nextest run -p merman --features raster render::raster::tests::svg_to_png_keeps_text_visible_when_requested_font_is_missing` -
    passed, `1` test run and `54` skipped.

Residual note:

- HPD-090 is closed. This slice prepared the baseline corpus; it does not claim broad
  `parity-root` residual closure. The broad stale family set plus the Class, Timeline, and
  Flowchart narrow stale sets are handled. Do not refresh all baselines or run a broad official
  fixture import unless a fresh inventory changes the decision. The raster test hygiene follow-up
  is not a baseline refresh or root residual fix. Continue with HPD-080 only for fresh visible
  renderability defects; otherwise return to HPD-050 source-backed audits.

## HPD-050 - Architecture Root Revalidation After HPD-090

Outcome:

- Regenerated Architecture structural DOM parity and `parity-root` diagnostic reports after HPD-090
  baseline preparation closed.
- Architecture structural DOM parity remains green.
- Architecture `parity-root` remains an expected diagnostic failure with `25` root/style width
  mismatch rows.
- The current leading root queue is:
  - `stress_architecture_junction_fork_join_026`: `+13.976px`
  - `stress_architecture_batch5_long_titles_and_punct_076`: `+5.000px`
  - `stress_architecture_html_titles_and_escapes_041`: `+5.000px`
  - `stress_architecture_unicode_and_xml_escapes_019`: `+3.000px`
  - `stress_architecture_batch6_init_fontsize_icon_size_wrap_093`: `-2.500px`
  - `stress_architecture_nested_groups_002`: `+2.500px`
- `stress_architecture_group_port_edges_017` is zero-delta in the fresh all-row report and should
  not be reopened from older pre-Procrustes diagnostics.
- No renderer, layout, fixture, baseline, or source code changed.

Touched surfaces:

- `target\compare\architecture_report_parity_after_hpd090_closeout_revalidation.md`
- `target\compare\architecture_report_parity_root_after_hpd090_closeout.md`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-04-hpd-050-architecture-root-revalidation-after-hpd090.md`

Focused verification:

- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture_report_parity_after_hpd090_closeout_revalidation.md` -
  passed.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_report_parity_root_after_hpd090_closeout.md` -
  expected-failed with the existing `25` root/style mismatch rows.

Residual note:

- This is a diagnostic refresh, not a production fix. Continue Architecture work only from
  source-backed FCoSE/Cytoscape phase evidence; standalone group padding, root padding, font-family
  switching, exact label-width lookup, and one-off root pins remain rejected.

## HPD-050 - Architecture Render-Path Internal FCoSE Phases

Outcome:

- Extended `tools/debug/arch_render_path_probe_fixture.js` so the actual Mermaid
  `mermaid.render(...)` Architecture probe captures bundled nested `cytoscape-fcose@2.2.0` /
  `cose-base@2.2.0` internal stages in `probe.fcoseStages`.
- Captured phases include `coseLayout.start`, after process-children, after process-edges /
  constraints, `classicLayout.start`, `initConstraintVariables.start`, first tick start / after
  move, `classicLayout.end`, `coseLayout.after-runLayout`, and `relocateComponent.before-shift`.
- The render-path probe Markdown now writes bundled FCoSE/Cose internal stage and compound-rect
  tables.
- `debug-architecture-delta --render-probe-dir` now compares those bundled layout-base group rects
  with local FCoSE compound rectangles.
- No production renderer, layout formula, SVG fixture, or baseline changed.

Focused `junction_fork_join_026` findings:

- Render-path `renderedFacts` still match `storedFacts`.
- The focused probe captured `22` bundled internal FCoSE/Cose stages and `0` probe errors.
- Local FCoSE compound widths/heights match bundled run `0` `classicLayout.end` /
  `coseLayout.after-runLayout` exactly for both `left` and `right`.
- Bundled run `1` `classicLayout.end` / `coseLayout.after-runLayout` diverges by the same group
  width/height deltas seen in the residual:
  - `left`: `dw=+17.331122`, `dh=-18.609285`
  - `right`: `dw=-3.388269`, `dh=+6.107441`
- The row is now narrowed to second FCoSE rerun / segment-adjusted phase behavior, not root bounds,
  group padding, final group rect emission, or stale upstream SVG baselines.

Touched surfaces:

- `tools/debug/arch_render_path_probe_fixture.js`
- `crates/xtask/src/cmd/debug/architecture.rs`
- `target\compare\architecture-render-path-internal-probe-hpd050\stress_architecture_junction_fork_join_026.render-path-probe.json`
- `target\compare\architecture-render-path-internal-probe-hpd050\stress_architecture_junction_fork_join_026.render-path-probe.md`
- `target\compare\architecture-delta-render-path-internal-join-hpd050\stress_architecture_junction_fork_join_026.md`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-04-hpd-050-architecture-render-path-internal-fcose.md`

Focused verification:

- `node --check tools\debug\arch_render_path_probe_fixture.js` - passed.
- `cargo fmt -p xtask` - passed.
- `cargo nextest run -p xtask render_path_probe_markdown_summarizes_facts_and_stages architecture_render_path_join_reports_local_deltas` -
  passed, `2` tests run.
- `cargo run -p xtask -- debug-architecture-render-path-probe --fixture stress_architecture_junction_fork_join_026 --browser-exe "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe" --out target\compare\architecture-render-path-internal-probe-hpd050` -
  passed.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_junction_fork_join_026 --probe-dir target\compare\architecture-fcose-probe-active-residuals-hpd050 --render-probe-dir target\compare\architecture-render-path-internal-probe-hpd050 --out target\compare\architecture-delta-render-path-internal-join-hpd050` -
  passed.
- `cargo fmt --check -p xtask` - passed.
- JSONL validation for `docs\workstreams\headless-parity-deepening\CONTEXT.jsonl` - passed.
- `git diff --check` - passed; Git reported only the existing CRLF normalization warning for
  `CONTEXT.jsonl`.
- `cargo nextest run -p xtask` - passed, `106` tests run.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture_report_parity_after_internal_fcose_probe_hpd050.md` -
  passed; Architecture structural parity stayed green.

Residual note:

- This is evidence tooling only. The next production-capable junction slice should compare local
  `manatee` run sequencing and segment-edge rerun behavior against bundled `cytoscape-fcose` run
  `1`. Do not tune Architecture root width, group padding, or emitted group rectangles from this
  evidence.

## HPD-050 - Architecture FCoSE Current Reclassification

Outcome:

- Revalidated the current HEAD before making any production renderer or `manatee` change.
- `stress_architecture_junction_fork_join_026` is no longer an active Architecture root residual at
  the current gate precision. The focused `parity-root` report passes and shows only
  `-0.000244px` max-width/viewBox width drift.
- Full Architecture structural `parity` remains green.
- Full Architecture `parity-root` remains an expected diagnostic failure, but the leading active
  row is now `stress_architecture_batch6_junctions_multi_split_with_group_edges_087` at
  `+46.001831px` max-width/viewBox width delta.
- The current render-path delta join says render-path stored facts still match upstream for both
  focused rows. For `batch6_junctions_multi_split_with_group_edges_087`, local groups/services are
  displaced almost symmetrically from the render-path SVG facts: `edge` around `-23.000899px` on X,
  `core` around `+23.000899px` on X, with local `core` group height `+7.345448px`.
- This supersedes the older junction-focused handoff. The current evidence does not justify a
  production change to root bounds, group padding, final group rectangle emission, or `manatee`
  rerun sequencing for `junction_fork_join_026`.

Touched surfaces:

- `target\compare\architecture_junction_current_hpd050_fcose.md`
- `target\compare\architecture_batch6_junctions_current_hpd050_fcose.md`
- `target\compare\architecture_report_parity_current_hpd050_fcose.md`
- `target\compare\architecture_report_parity_root_current_hpd050_fcose.md`
- `target\compare\architecture-render-path-current-hpd050\stress_architecture_junction_fork_join_026.render-path-probe.json`
- `target\compare\architecture-render-path-current-hpd050\stress_architecture_batch6_junctions_multi_split_with_group_edges_087.render-path-probe.json`
- `target\compare\architecture-delta-render-path-current-hpd050\stress_architecture_junction_fork_join_026.md`
- `target\compare\architecture-delta-render-path-current-hpd050\stress_architecture_batch6_junctions_multi_split_with_group_edges_087.md`
- `target\compare\architecture-delta-render-path-current-hpd050\architecture-delta-batch.md`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-04-hpd-050-architecture-fcose-current-reclassification.md`

Focused verification:

- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_junction_fork_join_026 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_junction_current_hpd050_fcose.md` -
  passed.
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_batch6_junctions_multi_split_with_group_edges_087 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_batch6_junctions_current_hpd050_fcose.md` -
  expected-failed with the existing root/style mismatch, upstream `653.25px` vs local `699.25px`.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture_report_parity_current_hpd050_fcose.md` -
  passed.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_report_parity_root_current_hpd050_fcose.md` -
  expected-failed with the current root/style queue led by
  `batch6_junctions_multi_split_with_group_edges_087`.
- `cargo run -p xtask -- report-overrides --check-no-growth` - passed; Architecture root overrides
  remain `0`.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_junction_fork_join_026 --fixture stress_architecture_batch6_junctions_multi_split_with_group_edges_087 --render-probe-dir target\compare\architecture-render-path-current-hpd050 --out target\compare\architecture-delta-render-path-current-hpd050` -
  passed and wrote the focused render-path delta reports.

Residual note:

- This is evidence-only classification. The next Architecture production slice should start from
  the current `batch6` group/service displacement evidence and only change code if a source-backed
  rule survives family-level validation.

## HPD-050 - Architecture Render-Path Probe Xtask Wrapper

Outcome:

- Added `xtask debug-architecture-render-path-probe` as the stable command wrapper for the
  Architecture render-path browser probe.
- The command reuses `tools/debug/arch_render_path_probe_fixture.js`, so evidence still comes from
  the actual installed Mermaid `mermaid.render(...)` Architecture renderer path instead of the
  manual ArchitectureDB/FCoSE reconstruction harness.
- The wrapper supports repeated `--fixture` filters, `--out` / `--out-dir`, and `--browser-exe`.
- Each fixture now writes:
  - raw JSON as `<fixture>.render-path-probe.json`
  - Markdown summary as `<fixture>.render-path-probe.md`
  - `architecture-render-path-probe-batch.md` when multiple fixtures are run
- The Markdown summary records Mermaid/Cytoscape/FCoSE versions, rendered-vs-stored root facts,
  group rectangles, service positions, captured stage bboxes, and group bounds by stage.
- A focused junction run through the new xtask wrapper reproduced the same authoritative result:
  `facts match: true`, `6` captured stages, `2` SVG groups, and `5` SVG services.
- No renderer output, layout formula, SVG fixture, or baseline behavior changed.

Touched surfaces:

- `crates/xtask/src/cmd/debug/architecture.rs`
- `crates/xtask/src/cmd/debug/mod.rs`
- `crates/xtask/src/main.rs`
- `target\compare\architecture-render-path-probe-xtask-hpd050\stress_architecture_junction_fork_join_026.render-path-probe.json`
- `target\compare\architecture-render-path-probe-xtask-hpd050\stress_architecture_junction_fork_join_026.render-path-probe.md`

Focused verification:

- `cargo fmt -p xtask` - passed.
- `cargo nextest run -p xtask render_path_probe fcose_probe_args_accept_out_dir_aliases fcose_probe_batch_markdown_links_per_fixture_artifacts` -
  passed, `7` tests run.
- `cargo run -p xtask -- debug-architecture-render-path-probe --fixture stress_architecture_junction_fork_join_026 --browser-exe "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe" --out target\compare\architecture-render-path-probe-xtask-hpd050` -
  passed and wrote JSON plus Markdown artifacts.

Residual note:

- This is evidence tooling only. Future `junction_fork_join_026` work should use the xtask wrapper
  when it needs actual Mermaid render-path facts, then move to bundled FCoSE/Cose internal-phase
  comparison if more source evidence is needed. It still does not justify tuning `manatee` against
  the older manual probe when that probe disagrees with the real render path.

## HPD-050 - Architecture Render-Path Delta Join

Outcome:

- Extended `xtask debug-architecture-delta` with optional `--render-probe-dir`.
- The delta report can now join actual Mermaid render-path probe facts with local SVG deltas,
  separately from the older manual FCoSE probe supplied by `--probe-dir`.
- The new report section records:
  - render-path stored root facts versus local root facts;
  - render-path SVG group rectangles versus local emitted group rectangles;
  - render-path SVG service positions versus local emitted service positions;
  - render-path group stage `bb` values versus local FCoSE compound rectangles.
- The Architecture delta batch index now has separate `probe json` and `render-path probe json`
  columns.
- Focused `junction_fork_join_026` output confirms the render-path probe is still authoritative:
  `rendered/stored facts match: true`, stored max-width `2808.126709`, local max-width
  `2822.102295`, delta `+13.975586`.
- The same focused report shows the leading local emitted differences:
  - `left` group: `dx=-6.954918`, `dy=+6.250922`, `dw=+17.331122`, `dh=-18.609285`;
  - `right` group: `dx=+10.376204`, `dy=-12.358363`, `dw=-3.388269`, `dh=+6.107441`;
  - service position deltas remain split across the same displacement axes.
- Render-path stage comparison makes the junction row more precise: local FCoSE compound bounds are
  close to the render-path `layoutstop-run1-before-segments` group `bb` shape (`dx=+3.25`,
  `dy=+11`, `dw=-5`, `dh=-22` for both groups), but diverge from the post-rerun
  `draw-after-layout-before-svg-emission` group `bb` state that feeds the stored SVG.
- No production renderer, layout formula, SVG fixture, or baseline changed.

Touched surfaces:

- `crates/xtask/src/cmd/debug/architecture.rs`
- `target\compare\architecture-delta-render-path-join-hpd050\stress_architecture_junction_fork_join_026.md`
- `target\compare\architecture_report_parity_after_render_path_join_hpd050.md`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-04-hpd-050-architecture-render-path-delta-join.md`

Focused verification:

- `cargo fmt --check -p xtask` - passed.
- `cargo nextest run -p xtask` - passed, `106` tests run.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_junction_fork_join_026 --probe-dir target\compare\architecture-fcose-probe-active-residuals-hpd050 --render-probe-dir target\compare\architecture-render-path-probe-xtask-hpd050 --out target\compare\architecture-delta-render-path-join-hpd050` -
  passed and wrote the joined junction report.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture_report_parity_after_render_path_join_hpd050.md` -
  passed; Architecture structural parity stayed green.

Residual note:

- This is evidence tooling only. The next source-backed junction slice should compare local
  `manatee` against bundled `cytoscape-fcose@2.2.0` / `cose-base@2.2.0` internal phases from the
  actual render path. Do not tune root bounds, group padding, or final SVG group rect emission from
  this report alone.

## HPD-050 - Architecture Render-Path Probe

Outcome:

- Added `tools/debug/arch_render_path_probe_fixture.js` as a diagnostic-only probe for the actual
  installed Mermaid Architecture render path.
- Unlike the existing manual ArchitectureDB/FCoSE probe, this script runs `mermaid.render(...)` and
  patches the installed Mermaid `11.15.0` IIFE in memory, so captured Cytoscape state comes from the
  bundled renderer path used by upstream SVG generation.
- For `stress_architecture_junction_fork_join_026`, the probe reproduced the stored upstream SVG
  facts exactly:
  - viewBox
    `-1362.063232421875 -1213.2674560546875 2808.126708984375 2557.534912109375`
  - max-width `2808.126708984375`
  - `left` group `1788.5571178808743 x 1649.1539928009868`
- The captured stage split shows the real SVG emission consumes the post-rerun state:
  - `layoutstop-run1-before-segments`: graph bbox `2743.102 x 2465.033`, `left` group
    `1805.888 x 1630.544`
  - `cy-ready-before-resolve`: graph bbox `2729.127 x 2477.535`, `left` group
    `1788.557 x 1649.154`
  - `draw-after-layout-before-svg-emission`: graph bbox `2729.127 x 2477.535`, `left` group
    `1788.557 x 1649.154`
- The render-path toolchain is Mermaid `11.15.0`, Cytoscape `3.33.4`,
  Cytoscape FCoSE `2.2.0`, nested `cose-base@2.2.0`, and nested `layout-base@2.0.1`.
- No renderer output, layout formula, SVG fixture, or baseline behavior changed.

Touched surfaces:

- `tools/debug/arch_render_path_probe_fixture.js`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-04-hpd-050-architecture-render-path-probe.md`
- `target\compare\architecture-render-path-probe-hpd050\stress_architecture_junction_fork_join_026.render-path-probe.json`

Focused verification:

- `$env:PUPPETEER_EXECUTABLE_PATH='C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'; node tools\debug\arch_render_path_probe_fixture.js stress_architecture_junction_fork_join_026 > target\compare\architecture-render-path-probe-hpd050\stress_architecture_junction_fork_join_026.render-path-probe.json` -
  passed.
- Read the generated JSON and confirmed `renderedFacts` matched `storedFacts` for root, group, and
  service facts used by the current `junction_fork_join_026` diagnosis.

Residual note:

- `junction_fork_join_026` is no longer an unexplained stored-baseline/probe split. The stored SVG
  is reproduced by the real render path, while the manual ArchitectureDB reconstruction probe
  remains diagnostic-only. Future junction work should instrument the bundled render path or build a
  reference harness against the same nested FCoSE/Cose stack; do not tune manatee to the manual
  probe when it disagrees with this evidence.

## HPD-050 - Architecture Child Group Inset Experiment Rejected

Outcome:

- Audited the renderer-side nested group production path in
  `crates/merman-render/src/svg/parity/architecture/geometry.rs`.
- `GroupRectComputer` does not union child group emitted rects raw for parent content; it first
  applies the existing `child_group_inset = 1.0` on each edge, then applies group padding.
- A focused experiment changed that inset to `0.75` to test whether `nested_groups_002/platform`
  was a narrow child-group aggregate boundary fix.
- The experiment was rejected:
  - Architecture `parity-root` mismatches expanded from the current `24` to `44`.
  - `nested_groups_002` worsened from `+2.500` to `+2.750`.
  - Previously resolved `group_port_edges_017` reappeared at `+0.250`.
  - Deep/nested group rows such as `deep_group_chain_027` and
    `batch6_deep_group_chain_crosslinks_094` regressed.
- The production code was restored to `child_group_inset = 1.0`; no renderer behavior changed.

Touched surfaces:

- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-04-hpd-050-architecture-child-group-inset-rejected.md`
- `target\compare\architecture_report_parity_root_child_inset_075_hpd050.md`

Focused verification:

- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_report_parity_root_child_inset_075_hpd050.md` -
  expected-failed with `44` root-only mismatches under the rejected experiment.
- `git diff -- crates/merman-render/src/svg/parity/architecture/geometry.rs` - clean after
  restoring `child_group_inset = 1.0`.

Residual note:

- Do not fix `nested_groups_002` by retuning `child_group_inset` globally. The residual remains a
  child-group aggregate boundary diagnostic unless a narrower source-backed phase model survives
  full Architecture verification.

## HPD-050 - Architecture Nested Group Aggregate Edge Attribution

Outcome:

- `xtask debug-architecture-delta --probe-dir` now adds a `Group aggregate edge attribution` table.
- The table extends edge attribution beyond direct services by comparing:
  - browser direct service child unions plus child-group `node.boundingBox()` values
  - local direct service contribution bounds plus child-group emitted rects
- This makes nested parent group content deltas attributable to left/right/top/bottom child owners
  instead of requiring manual reconstruction from the aggregate content table.
- Regenerated the current top-residual batch under
  `target\compare\architecture-delta-current-top-aggregate-edge-hpd050`.
- In `nested_groups_002/platform`, the new row reports:
  - child groups: `data, runtime`
  - browser/local left owner: `data`, left dx `44.250000`
  - browser/local right owner: `data`, right dx `43.750000`
  - aggregate edge width delta: `-0.500000`
  - browser/local top owner: `runtime`, top dy `40.000000`
  - browser/local bottom owner: `data`, bottom dy `40.000000`
  - aggregate edge height delta: `0.000000`
- This directly attributes the parent `platform` aggregate width tail to child-group boundary
  placement/width, not direct services or final group expansion.
- No renderer output, layout formula, SVG fixture, or baseline behavior changed.

Touched surfaces:

- `crates/xtask/src/cmd/debug/architecture.rs`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-04-hpd-050-architecture-nested-group-aggregate-edge.md`
- `target\compare\architecture-delta-current-top-aggregate-edge-hpd050\architecture-delta-batch.md`

Focused verification:

- `cargo fmt -p xtask` - passed.
- `cargo nextest run -p xtask architecture_probe_join_reports_nested_group_aggregate_content architecture_probe_join_decomposes_group_and_service_bounds` -
  passed, `2` tests run.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_junction_fork_join_026 --fixture stress_architecture_batch5_long_titles_and_punct_076 --fixture stress_architecture_html_titles_and_escapes_041 --fixture stress_architecture_unicode_and_xml_escapes_019 --fixture stress_architecture_batch6_init_fontsize_icon_size_wrap_093 --fixture stress_architecture_nested_groups_002 --probe-dir target\compare\architecture-fcose-probe-active-residuals-hpd050 --out target\compare\architecture-delta-current-top-aggregate-edge-hpd050` -
  passed and wrote the aggregate-edge batch.
- `cargo fmt --check -p xtask` - passed.
- `git diff --check` - passed.
- `cargo nextest run -p xtask` - passed, `100` tests run.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed; Architecture structural parity stayed green.

Residual note:

- This is evidence tooling only. It narrows `nested_groups_002/platform` to child-group aggregate
  boundary drift, but it does not justify changing group padding, final group expansion, service
  label measurement, or root-bounds formulas.

## HPD-050 - Architecture Delta Batch Root Residual Score Projection

Outcome:

- `xtask debug-architecture-delta` now projects the same root residual vocabulary used by
  `summarize-architecture-deltas`.
- Per-fixture delta reports now show:
  - `viewBox width delta`
  - `viewBox height delta`
  - `max-width delta`
  - `root residual score`
- Multi-fixture `architecture-delta-batch.md` now includes those same columns and sorts rows by
  root residual score descending, then fixture name.
- Regenerated the current top-residual batch under
  `target\compare\architecture-delta-current-top-root-score-hpd050`.
- The batch index now makes the top row self-contained:
  - `junction_fork_join_026`: viewBox width `+13.976`, viewBox height `-12.502`,
    max-width `+13.976`, score `13.976`.
- The focused test covers the height-only ordering case: a `-6.000` viewBox height row sorts ahead
  of a `+5.000` max-width-only row.
- No renderer output, layout formula, SVG fixture, or baseline behavior changed.

Touched surfaces:

- `crates/xtask/src/cmd/debug/architecture.rs`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-04-hpd-050-architecture-delta-batch-root-score.md`
- `target\compare\architecture-delta-current-top-root-score-hpd050\architecture-delta-batch.md`

Focused verification:

- `cargo fmt -p xtask` - passed.
- `cargo nextest run -p xtask architecture_delta_batch_markdown_links_per_fixture_artifacts architecture_delta_summary_order_sorts_by_root_residual_score_then_stem` -
  passed, `2` tests run.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_junction_fork_join_026 --fixture stress_architecture_batch5_long_titles_and_punct_076 --fixture stress_architecture_html_titles_and_escapes_041 --fixture stress_architecture_unicode_and_xml_escapes_019 --fixture stress_architecture_batch6_init_fontsize_icon_size_wrap_093 --fixture stress_architecture_nested_groups_002 --probe-dir target\compare\architecture-fcose-probe-active-residuals-hpd050 --out target\compare\architecture-delta-current-top-root-score-hpd050` -
  passed and wrote the root-score batch index plus per-fixture reports.
- `cargo fmt --check -p xtask` - passed.
- `git diff --check` - passed.
- `cargo nextest run -p xtask` - passed, `100` tests run.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed; Architecture structural parity stayed green.

Residual note:

- This is evidence tooling only. The current-top Architecture delta batch now uses the same
  root-score ordering as the all-fixture summary, but it does not change residual classification or
  justify a production layout tweak.

## HPD-050 - Architecture Delta Summary Root Residual Score

Outcome:

- `xtask summarize-architecture-deltas` now reports `viewBox width delta`,
  `viewBox height delta`, and `root residual score`.
- The score is the maximum absolute value across `max-width`, viewBox width, and viewBox height
  deltas, then fixture name remains the deterministic tie-breaker.
- This keeps height-only and viewBox-dominant root tails visible in the local Architecture delta
  summary instead of letting the report be shaped only by `max-width`.
- Regenerated the summary under
  `target\compare\architecture-delta-summary-root-score-hpd050\architecture-delta-summary.md`.
- The current top rows remain the active Architecture root queue:
  - `junction_fork_join_026`: score `13.976`, with viewBox width `+13.976` and height `-12.502`.
  - `batch5_long_titles_and_punct_076`: score `5.000`.
  - `html_titles_and_escapes_041`: score `5.000`.
  - `unicode_and_xml_escapes_019`: score `3.000`.
  - `batch6_init_fontsize_icon_size_wrap_093`: score `2.500`.
  - `nested_groups_002`: score `2.500`.
- Smaller viewBox-height tails are now ordered correctly too: `group_to_group_multi_034`
  scores `0.755` from height delta and ranks above `long_group_titles_018` at `0.656`.
- `group_port_edges_017` remains zero-delta on current HEAD and should not be reopened from stale
  pre-Procrustes artifacts.
- No renderer output, layout formula, SVG fixture, or baseline behavior changed.

Touched surfaces:

- `crates/xtask/src/cmd/debug/architecture.rs`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-04-hpd-050-architecture-delta-summary-root-score.md`
- `target\compare\architecture-delta-summary-root-score-hpd050\architecture-delta-summary.md`
- `target\compare\architecture_report_parity_root_hpd050_current.md`

Focused verification:

- `cargo fmt -p xtask` - passed.
- `cargo fmt --check -p xtask` - passed.
- `cargo nextest run -p xtask architecture_delta_summary_order_sorts_by_root_residual_score_then_stem` -
  passed, `1` test run.
- `cargo nextest run -p xtask` - passed, `100` tests run.
- `cargo run -p xtask -- summarize-architecture-deltas --out target\compare\architecture-delta-summary-root-score-hpd050` -
  passed and wrote the root-score sorted summary.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed; Architecture structural parity stayed green.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_report_parity_root_hpd050_current.md` -
  expected-failed with `24` root-only mismatches.
- `git diff --check` - passed.

Residual note:

- This is evidence tooling only. Root-score ordering makes the Architecture delta queue more honest
  for width and height tails, but it does not justify a production layout, group-padding,
  final-rect, or root-bounds formula change.

## HPD-050 - Architecture Nested Group Aggregate Delta Report

Outcome:

- `xtask debug-architecture-delta --probe-dir` now adds a `Group aggregate child attribution` table
  to the browser probe phase join.
- The aggregate table combines local direct service contribution bounds with direct child-group
  emitted rects, then compares that local aggregate against browser
  `childrenBoundingBoxIncludeLabels`.
- This closes a diagnostic blind spot in nested Architecture fixtures where parent groups have no
  direct services. The old direct-service table correctly printed `<none>` for those parents, but
  that made nested residuals depend on manual child-group reconstruction.
- Regenerated the current top Architecture residual batch under
  `target\compare\architecture-delta-current-top-residuals-hpd050`.
- In `nested_groups_002`, the new `platform` row now reports child groups `data, runtime`, local
  aggregate width `375.654085`, browser children width `376.154085`, `content dw=-0.500000`, and
  matching local/browser expansion `dw=83.000000`. This isolates the parent-width tail to nested
  child-group aggregate width, not a direct-service missing-data gap.
- No renderer output, layout formula, SVG fixture, or baseline behavior changed.

Touched surfaces:

- `crates/xtask/src/cmd/debug/architecture.rs`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-04-hpd-050-architecture-nested-group-aggregate.md`
- `target\compare\architecture-delta-current-top-residuals-hpd050\architecture-delta-batch.md`

Focused verification:

- `cargo fmt -p xtask` - passed.
- `cargo nextest run -p xtask architecture_probe_join` - passed, `2` tests run.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_junction_fork_join_026 --fixture stress_architecture_batch5_long_titles_and_punct_076 --fixture stress_architecture_html_titles_and_escapes_041 --fixture stress_architecture_unicode_and_xml_escapes_019 --fixture stress_architecture_batch6_init_fontsize_icon_size_wrap_093 --fixture stress_architecture_nested_groups_002 --probe-dir target\compare\architecture-fcose-probe-active-residuals-hpd050 --out target\compare\architecture-delta-current-top-residuals-hpd050` -
  passed and wrote the indexed current top-residual batch.
- `cargo fmt --check -p xtask` - passed.
- `cargo nextest run -p xtask` - passed, `100` tests run.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed; Architecture structural parity stayed green.

Residual note:

- This is evidence tooling only. It makes nested group residuals source-phase-auditable, but it does
  not justify a global group padding, final rect, child label, or root-bounds formula change.

## HPD-050 - Architecture Delta Batch Index

Outcome:

- `xtask debug-architecture-delta` now writes `architecture-delta-batch.md` when a run includes
  more than one `--fixture`.
- The batch index lists each fixture's Markdown report, copied upstream SVG, local SVG, optional
  browser probe JSON, `max-width` delta, and matched service/junction/group-rect counts.
- Single-fixture behavior is unchanged; the index is only emitted for batch runs.
- Regenerated probe-backed reports for `batch5`, `html_titles`, and `unicode` under
  `target\compare\architecture-delta-batch-index-hpd050`.
- The new index records the focused residuals directly:
  - `batch5_long_titles_and_punct_076`: `max-width delta=+5.000`, `4` services, `1` group rect.
  - `html_titles_and_escapes_041`: `max-width delta=+5.000`, `3` services, `1` group rect.
  - `unicode_and_xml_escapes_019`: `max-width delta=+3.000`, `4` services, `1` group rect.
- No renderer output, layout formula, SVG fixture, or baseline behavior changed.

Touched surfaces:

- `crates/xtask/src/cmd/debug/architecture.rs`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-04-hpd-050-architecture-delta-batch-index.md`
- `target\compare\architecture-delta-batch-index-hpd050\architecture-delta-batch.md`

Focused verification:

- `cargo fmt -p xtask` - passed.
- `cargo nextest run -p xtask architecture_delta` - passed, `3` tests run.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_batch5_long_titles_and_punct_076 --fixture stress_architecture_html_titles_and_escapes_041 --fixture stress_architecture_unicode_and_xml_escapes_019 --probe-dir target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050 --out target\compare\architecture-delta-batch-index-hpd050` -
  passed and wrote the batch index.
- `cargo fmt --check -p xtask` - passed.
- `cargo nextest run -p xtask` - passed, `99` tests run.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed; Architecture structural parity stayed green.

Residual note:

- This is evidence tooling only. The index makes multi-fixture local delta reports citable and
  reviewable, but it does not change Architecture residual classification or production layout.

## HPD-050 - Architecture Delta Batch Fixture CLI

Outcome:

- `xtask debug-architecture-delta` now accepts repeated `--fixture` filters, matching the
  batch-friendly shape already used by `debug-architecture-fcose-probe`.
- A single command can regenerate multiple local delta reports in one output directory while
  preserving the existing one-report-per-fixture Markdown and SVG artifacts.
- Existing single-fixture behavior is preserved.
- `--probe-dir` still works with repeated fixtures, so the current service/body/label/final-bbox
  join can be regenerated for the focused Architecture residual set without manual command loops.
- Regenerated probe-backed reports for `batch5`, `html_titles`, and `unicode` under
  `target\compare\architecture-delta-batch-cli-hpd050`.
- No renderer output, layout formula, SVG fixture, or baseline behavior changed.

Touched surfaces:

- `crates/xtask/src/cmd/debug/architecture.rs`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-04-hpd-050-architecture-delta-batch-cli.md`
- `target\compare\architecture-delta-batch-cli-hpd050`

Focused verification:

- `cargo fmt -p xtask` - passed.
- `cargo nextest run -p xtask architecture_delta_args_accept_probe_dir` - passed, `1` test run.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_batch5_long_titles_and_punct_076 --out target\compare\architecture-delta-batch-cli-hpd050` -
  passed, preserving single-fixture behavior.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_batch5_long_titles_and_punct_076 --fixture stress_architecture_html_titles_and_escapes_041 --out target\compare\architecture-delta-batch-cli-hpd050` -
  passed, writing two reports from one command.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_batch5_long_titles_and_punct_076 --fixture stress_architecture_html_titles_and_escapes_041 --fixture stress_architecture_unicode_and_xml_escapes_019 --probe-dir target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050 --out target\compare\architecture-delta-batch-cli-hpd050` -
  passed, writing three probe-joined reports.

Residual note:

- This is evidence tooling only. Batch delta regeneration reduces stale manual report drift, but it
  does not change Architecture residual classification or production layout.

## HPD-050 - Architecture Delta Summary Residual Ordering

Outcome:

- Enhanced `xtask summarize-architecture-deltas` with a `max-width delta` column.
- The summary now sorts rows by absolute `max-width` delta descending, with fixture name as the
  deterministic tie-breaker.
- This makes the local Architecture delta summary align with the `parity-root` residual queue
  instead of hiding active rows in alphabetical fixture order.
- Refreshed the current Architecture root snapshot at
  `target\compare\architecture_report_parity_root_hpd050_current.md`; it expected-fails with `24`
  root-only mismatches.
- Regenerated the ordered delta summary at
  `target\compare\architecture-delta-summary-hpd050-current\architecture-delta-summary.md`.
- The top rows now surface directly in one table:
  - `junction_fork_join_026`: `max-width Δ=+13.976`, `group max dw=+17.331`,
    `group max dh=-18.609`.
  - `batch5_long_titles_and_punct_076`: `max-width Δ=+5.000`, `group max dw=+5.000`.
  - `html_titles_and_escapes_041`: `max-width Δ=+5.000`, `group max dw=+5.000`.
  - `unicode_and_xml_escapes_019`: `max-width Δ=+3.000`, `group max dw=+3.000`.
  - `batch6_init_fontsize_icon_size_wrap_093`: `max-width Δ=-2.500`,
    `group max dw=-3.000`.
  - `nested_groups_002`: `max-width Δ=+2.500`, `group max dw=-0.500`.
- `group_port_edges_017` is zero-delta in the current summary and should not be treated as part of
  the active root mismatch queue unless a fresh report regresses.
- No renderer output, layout formula, SVG fixture, or baseline behavior changed.

Touched surfaces:

- `crates/xtask/src/cmd/debug/architecture.rs`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-04-hpd-050-architecture-delta-summary-order.md`
- `target\compare\architecture_report_parity_root_hpd050_current.md`
- `target\compare\architecture-delta-summary-hpd050-current`

Focused verification:

- `cargo fmt --check -p xtask` - passed.
- `cargo nextest run -p xtask architecture_delta_summary_order_sorts_by_abs_max_width_delta_then_stem architecture_probe_join_decomposes_group_and_service_bounds` -
  passed, `2` tests run.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_report_parity_root_hpd050_current.md` -
  expected-failed with `24` root-only mismatches.
- `cargo run -p xtask -- summarize-architecture-deltas --out target\compare\architecture-delta-summary-hpd050-current` -
  passed and wrote the sorted summary.

Residual note:

- This is evidence tooling only. Sorting by current residual size prevents stale 25-row queue
  assumptions from driving work, but it does not change the known Architecture residual
  classification or justify a production layout tweak.

## HPD-050 - Architecture Service Label Final-Frame Report

Outcome:

- Extended `debug-architecture-delta --probe-dir` service joins with
  `local contribution label final-frame` plus label `dx` / `dy` / `dw` / `dh` columns.
- The new final-frame label column shifts the local contribution-label rectangle by half the local
  body size before comparing it with browser `labelBounds.all`.
- This makes the concept boundary explicit: local contribution-label bounds are extended child
  contribution rectangles from icon top to label bottom, not browser text-label bounds.
- Regenerated focused reports under
  `target\compare\architecture-delta-label-final-frame-hpd050`.
- Representative boundary-service label readings:
  - `batch5` / `registry`: `label dx=-1.5`, `label dw=+2`, `label dh=+77`.
  - `batch5` / `storage`: `label dx=-2.5`, `label dw=+4`, `label dh=+77`.
  - `html_titles` / `web`: `label dx=-0.5`, `label dw=+2`, `label dh=+77`.
  - `html_titles` / `origin`: `label dx=-1.5`, `label dw=+4`, `label dh=+77`.
  - `unicode` / `metrics`: `label dx=-3.5`, `label dw=+4`, `label dh=+77`.
  - `unicode` / `store`: `label dx=-0.5`, `label dw=-2`, `label dh=+77`.
- All focused rows show `label dy=-78` and `label dh=+77`, which is the expected phase mismatch
  between the extended local contribution-label rectangle and browser text label bounds. The useful
  residual signal is the service-specific horizontal `label dx` / `label dw`, not the vertical
  label comparison.
- No renderer output, layout formula, SVG fixture, or baseline behavior changed.

Touched surfaces:

- `crates/xtask/src/cmd/debug/architecture.rs`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-04-hpd-050-architecture-service-label-final-frame-report.md`
- `target\compare\architecture-delta-label-final-frame-hpd050`

Focused verification:

- `cargo nextest run -p xtask architecture_probe_join_decomposes_group_and_service_bounds` -
  passed, `1` test run.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_batch5_long_titles_and_punct_076 --probe-dir target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050 --out target\compare\architecture-delta-label-final-frame-hpd050` -
  passed and wrote the `pipeline` label final-frame join.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_html_titles_and_escapes_041 --probe-dir target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050 --out target\compare\architecture-delta-label-final-frame-hpd050` -
  passed and wrote the `ui` label final-frame join.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_unicode_and_xml_escapes_019 --probe-dir target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050 --out target\compare\architecture-delta-label-final-frame-hpd050` -
  passed and wrote the `i` label final-frame join.

Residual note:

- Treat this as evidence tooling only. The new label columns narrow the next audit to
  service-specific horizontal contribution width and placement drift. They do not justify changing
  vertical label math, group padding, final rect emission, or a lookup-only `labelWidth` patch.

## HPD-050 - Architecture Service Final BBox Report

Outcome:

- Extended `debug-architecture-delta --probe-dir` service joins with a diagnostic
  `local final bb final-frame` column.
- The new column applies the source-shaped `1px` final `node.boundingBox()` expansion to the local
  child union after shifting it into browser final-frame coordinates.
- Added final `dx` / `dy` / `dw` / `dh` columns against browser final service `node.boundingBox()`.
- Regenerated focused reports under
  `target\compare\architecture-delta-service-final-bbox-hpd050`.
- Representative boundary-service final-bbox readings:
  - `batch5` / `registry`: `final dw=+2`, `final dh=-1`.
  - `batch5` / `storage`: `final dw=+4`, `final dh=-1`.
  - `html_titles` / `web`: `final dw=+2`, `final dh=-1`.
  - `html_titles` / `origin`: `final dw=+4`, `final dh=-1`.
  - `unicode` / `metrics`: `final dw=+4`, `final dh=-1`.
  - `unicode` / `store`: `final dw=-2`, `final dh=-1`.
- Width drift survives final expansion, while the previous local-union-vs-browser height comparison
  narrows from `-3px` to final `-1px`. This keeps the residual in the child contribution and
  service position phase, not group padding or final rect emission.
- No renderer output, layout formula, SVG fixture, or baseline behavior changed.

Touched surfaces:

- `crates/xtask/src/cmd/debug/architecture.rs`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-04-hpd-050-architecture-service-final-bbox-report.md`
- `target\compare\architecture-delta-service-final-bbox-hpd050`

Focused verification:

- `cargo fmt --check -p xtask` - passed.
- `cargo nextest run -p xtask architecture_probe_join_decomposes_group_and_service_bounds` -
  passed, `1` test run.
- `cargo nextest run -p xtask architecture_delta_args_accept_probe_dir architecture_probe_join_decomposes_group_and_service_bounds` -
  passed, `2` tests run.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_batch5_long_titles_and_punct_076 --probe-dir target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050 --out target\compare\architecture-delta-service-final-bbox-hpd050` -
  passed and wrote the `pipeline` final-bbox join.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_html_titles_and_escapes_041 --probe-dir target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050 --out target\compare\architecture-delta-service-final-bbox-hpd050` -
  passed and wrote the `ui` final-bbox join.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_unicode_and_xml_escapes_019 --probe-dir target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050 --out target\compare\architecture-delta-service-final-bbox-hpd050` -
  passed and wrote the `i` final-bbox join.
- `cargo nextest run -p xtask` - passed, `97` tests run.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed; Architecture structural parity stayed green.
- `git diff --check` - passed.

Residual note:

- Treat this as evidence tooling only. The new final-bbox phase column rejects a standalone final
  rect or group-padding tweak and points back to service child contribution width, body/label
  bounds, and position drift.

## HPD-050 - Architecture LabelWidth Measurement Seam Audit

Outcome:

- Audited the existing text measurement and lookup infrastructure before adding any new
  Architecture production formula.
- Confirmed the shared `TextMeasurer` abstraction covers deterministic SVG text probes, while
  generated lookup tables remain diagram/phase scoped.
- Confirmed the C4 headless-shell text table is not directly reusable for Architecture: C4 captures
  SVG `<text>.getBBox().width`, while Architecture needs Cytoscape renderer `labelWidth` for
  compound child sizing.
- Confirmed the reusable Architecture infrastructure is the existing browser probe/report pipeline:
  `debug-architecture-fcose-probe` writes final service `metrics.labelWidth`,
  `labelBounds.all`, `bodyBounds`, and final `node.boundingBox()`, and
  `debug-architecture-delta --probe-dir` already joins those browser values with local service
  label metrics.
- Counted the active seven-probe residual batch and verified it already contains browser
  service-label widths for the current residual set: `batch5` 4, `batch6` 3, `group_port_edges` 4,
  `html_titles` 3, `junction_fork_join` 5, `nested_groups` 5, and `unicode` 4.
- No renderer output, layout formula, SVG fixture, generated lookup table, or baseline behavior
  changed.

Touched surfaces:

- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-04-hpd-050-architecture-labelwidth-measurement-seam-audit.md`
- `docs/workstreams/headless-parity-deepening/EVIDENCE_AND_GATES.md`
- `docs/workstreams/headless-parity-deepening/HANDOFF.md`
- `docs/workstreams/headless-parity-deepening/TODO.md`
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl`

Focused verification:

- Source read: `crates/xtask/src/cmd/overrides/c4.rs`.
- Source read: `crates/merman-render/src/c4.rs`.
- Source read: `crates/merman-render/src/text/measure.rs`.
- Source read: `crates/merman-render/src/text/overrides.rs`.
- Source read: `crates/xtask/src/cmd/overrides/report.rs`.
- Source read: `crates/merman-render/src/architecture.rs`.
- Source read: `crates/merman-render/src/architecture_metrics.rs`.
- Source read: `crates/xtask/src/cmd/debug/architecture.rs`.
- Source read: `tools/debug/arch_fcose_browser_probe_fixture_025.js`.
- Existing probe artifacts counted under
  `target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050`.
- `CONTEXT.jsonl` validation - passed, `612` JSON lines.
- `git diff --check` - passed with the existing `CONTEXT.jsonl` LF-to-CRLF working-copy warning.
- Rust tests were not run because this slice changed only workstream documentation.

Residual note:

- The next safe implementation candidate is not C4 lookup reuse and not an exact Architecture
  `labelWidth` table by itself. Any production change must pair browser-faithful service
  `labelWidth` with the source child-union and final `node.boundingBox()` expansion phases, then
  survive full Architecture root verification.

## HPD-050 - Architecture Cytoscape Child Union Source Audit

Outcome:

- Audited the installed Mermaid `11.15.0` / Cytoscape `3.33.4` source path behind Architecture
  service child-union bounds.
- Confirmed Mermaid Architecture services set Cytoscape node `width` / `height` from
  `architecture.iconSize`, service `label` from title, `compound-sizing-wrt-labels: include`,
  `text-valign: bottom`, `text-halign: center`, and `font-size` from `architecture.fontSize`.
- Confirmed Mermaid emits group rects from final Cytoscape `node.boundingBox()`, then shifts
  `x1` / `y1` by `halfIconSize`.
- Confirmed Cytoscape parent compound content uses
  `children.boundingBox({ includeLabels: true, includeOverlays: false, useCache: false })`.
- Confirmed Cytoscape service child contribution is the union of separately stored `bodyBounds`
  and `labelBounds.all`: body bounds are expanded by `1px`, label bounds use renderer
  `labelWidth` / `labelHeight`, `text-valign`, `text-halign`, and a hardcoded
  `marginOfError = 2`.
- Confirmed final default `node.boundingBox()` adds another whole-bbox `1px` expansion, but the
  child bbox used by compound sizing does not apply that final expansion again.
- This source path explains the observed child-union `dy=+1`, `dh=-2` phase and the distinct
  final-bbox expansion, but it still does not justify a production formula without a durable
  browser-faithful Architecture service `labelWidth` measurement seam.
- No code, renderer output, SVG fixture, or baseline behavior changed.

Touched surfaces:

- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-04-hpd-050-architecture-cytoscape-child-union-source-audit.md`
- `docs/workstreams/headless-parity-deepening/EVIDENCE_AND_GATES.md`
- `docs/workstreams/headless-parity-deepening/HANDOFF.md`
- `docs/workstreams/headless-parity-deepening/TODO.md`
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl`

Focused verification:

- Source read:
  `repo-ref\mermaid\packages\mermaid\src\diagrams\architecture\architectureRenderer.ts`.
- Source read: `repo-ref\mermaid\packages\mermaid\src\diagrams\architecture\svgDraw.ts`.
- Source read: `tools\mermaid-cli\node_modules\cytoscape\dist\cytoscape.cjs.js`.
- Confirmed installed package versions: Mermaid `11.15.0`, Cytoscape `3.33.4`.
- Confirmed pinned Mermaid checkout: `41646dfd43ac83f001b03c70605feb036afae46d`.

Residual note:

- The next production-capable seam is Architecture service label measurement, not group padding or
  body-border tweaks. A candidate must provide browser-faithful `labelWidth` and pair it with the
  source child-union plus final group-expansion phases across the full Architecture queue.

## HPD-050 - Architecture Service Child Union Attribution

Outcome:

- Extended `debug-architecture-delta --probe-dir` so the service join reports browser child union
  (`bodyBounds` union `labelBounds.all`), local service union shifted into the same final-frame
  coordinates, child `dx/dy/dw/dh`, and final-bbox frame `dx/dy`.
- Added a `Group content edge attribution` table that identifies which direct service owns each
  browser/local group-content edge and reports the resulting edge deltas.
- Regenerated focused reports under
  `target\compare\architecture-delta-service-child-union-hpd050`.
- Representative edge attribution:
  - `batch5` / `pipeline`: left edge from `storage` is `dx=-2.5`, right edge from `registry` is
    `dx=+0.5`, producing `edge dw=+3`; top/bottom are `+1/-1`, producing `edge dh=-2`.
  - `html_titles` / `ui`: left edge from `web` is `dx=-0.5`, right edge from `origin` is
    `dx=+2.5`, producing `edge dw=+3`; top/bottom are `+1/-1`, producing `edge dh=-2`.
  - `unicode` / `i`: left edge from `metrics` is `dx=-3.5`, right edge from `store` is `dx=-2.5`,
    producing `edge dw=+1`; top/bottom are `+1/-1`, producing `edge dh=-2`.
- This explains the direct group-content residuals by boundary service edge ownership rather than
  aggregate group width alone. It also confirms the height side is a stable child-union `-2px`
  phase that is later canceled by the final group expansion `+2px`.
- No renderer output, layout formula, SVG fixture, or baseline behavior changed.

Touched surfaces:

- `crates/xtask/src/cmd/debug/architecture.rs`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-04-hpd-050-architecture-service-child-union-attribution.md`
- `target\compare\architecture-delta-service-child-union-hpd050`

Focused verification:

- `cargo nextest run -p xtask architecture_probe_join_decomposes_group_and_service_bounds` -
  passed, `1` test run.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_batch5_long_titles_and_punct_076 --probe-dir target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050 --out target\compare\architecture-delta-service-child-union-hpd050` -
  passed and wrote the `pipeline` child-union edge attribution.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_html_titles_and_escapes_041 --probe-dir target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050 --out target\compare\architecture-delta-service-child-union-hpd050` -
  passed and wrote the `ui` child-union edge attribution.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_unicode_and_xml_escapes_019 --probe-dir target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050 --out target\compare\architecture-delta-service-child-union-hpd050` -
  passed and wrote the `i` child-union edge attribution.
- `cargo nextest run -p xtask` - passed, `97` tests run.
- `cargo fmt --check -p xtask` - passed.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed; Architecture structural parity stayed green.

Residual note:

- Continue from a source-backed service child-contribution model only if it can explain boundary
  service edge deltas across the full Architecture queue. The current evidence still rejects global
  label-scale, body-border, group-padding, and final-rect tweaks.

## HPD-050 - Architecture Service Label Metrics

Outcome:

- Extended `ArchitectureCytoscapeServiceBounds` with optional `label_metrics`
  (`text_width`, `half_width`, `applied_scale`) so local service child contribution rows expose the
  raw deterministic label measurement inputs behind local Cytoscape contribution bounds.
- Extended `debug-architecture-delta --probe-dir` service joins to read browser final-node
  `metrics.labelWidth` / `metrics.labelHeight` from the FCoSE probe JSON and report local-vs-browser
  label metric deltas beside existing body/label/union bbox deltas.
- Regenerated focused reports under
  `target\compare\architecture-delta-service-label-metrics-hpd050`.
- Representative service metric readings:
  - `batch5` / `storage`: browser labelWidth `217.000`, local text_width `222.828`, metric
    `dw=+5.828`, contribution-label `dw=+4`, local union versus browser final service bbox
    `+2w/-3h`.
  - `html_titles` / `web`: browser labelWidth `123.000`, local text_width `122.570`, metric
    `dw=-0.430`, contribution-label `dw=+2`, local union versus browser final service bbox
    `0w/-3h`.
  - `unicode` / `metrics`: browser labelWidth `117.000`, local text_width `118.055`, metric
    `dw=+1.055`, contribution-label `dw=+4`, local union versus browser final service bbox
    `+2w/-3h`.
- This narrows the seam but still rejects a single global label-scale or body-border production
  tweak: raw font metric drift, local scale/rounding, browser label padding, body border, and group
  height cancellation are separate phases.
- No renderer output, layout formula, SVG fixture, or baseline behavior changed.

Touched surfaces:

- `crates/merman-render/src/model.rs`
- `crates/merman-render/src/architecture.rs`
- `crates/merman-render/tests/architecture_layout_test.rs`
- `crates/xtask/src/cmd/debug/architecture.rs`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-04-hpd-050-architecture-service-label-metrics.md`
- `target\compare\architecture-delta-service-label-metrics-hpd050`

Focused verification:

- `cargo nextest run -p merman-render architecture_layout_exposes_cytoscape_service_child_bounds_by_service_id` -
  passed, `1` test run.
- `cargo nextest run -p xtask architecture_probe_join_decomposes_group_and_service_bounds` -
  passed, `1` test run.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_batch5_long_titles_and_punct_076 --probe-dir target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050 --out target\compare\architecture-delta-service-label-metrics-hpd050` -
  passed and wrote the `storage` label-metric join.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_html_titles_and_escapes_041 --probe-dir target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050 --out target\compare\architecture-delta-service-label-metrics-hpd050` -
  passed and wrote the `web` label-metric join.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_unicode_and_xml_escapes_019 --probe-dir target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050 --out target\compare\architecture-delta-service-label-metrics-hpd050` -
  passed and wrote the `metrics` label-metric join.
- `cargo nextest run -p merman-render --test architecture_layout_test` - passed, `7` tests run.
- `cargo nextest run -p xtask` - passed, `97` tests run.
- `cargo fmt --check -p merman-render -p xtask` - passed.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed; Architecture structural parity stayed green.

Residual note:

- Continue from a phase-specific service final-bbox contribution model rather than a global label
  scale, body border, group padding, or final group rect tweak. The next candidate must preserve
  the observed group-level height cancellation while explaining body border, label padding, font
  metric drift, and service position drift together.

## HPD-050 - Architecture Probe Phase Join

Outcome:

- Added optional `--probe-dir` support to `xtask debug-architecture-delta`, so local Architecture
  delta reports can read the matching browser/Cytoscape FCoSE probe JSON and emit phase-joined
  Markdown directly.
- Added a `Group content decomposition` table that reports browser
  `childrenBoundingBoxIncludeLabels`, local direct-service content union, content `dw` / `dh`,
  browser final expansion, local emitted expansion, expansion `dw` / `dh`, and emitted group
  `dw` / `dh`.
- Added a `Service bbox join` table that reports browser final service `bodyBounds`,
  `labelBounds.all`, `node.boundingBox()`, local service contribution body/label/union, service
  position drift, label-width delta, and local-union-vs-browser-final-bbox delta.
- The automated output reproduces the earlier manual direct-width decomposition:
  - `batch5` / `pipeline`: content `dw=+3`, expansion `dw=+2`, emitted `dw=+5`; height content
    `dh=-2` plus expansion `dh=+2` gives emitted `dh=0`.
  - `html_titles` / `ui`: content `dw=+3`, expansion `dw=+2`, emitted `dw=+5`; height again
    cancels as `-2 + 2 = 0`.
  - `unicode` / `i`: content `dw=+1`, expansion `dw=+2`, emitted `dw=+3`; height again cancels
    as `-2 + 2 = 0`.
- The service join now exposes the next seam without manual subtraction: `storage` and `metrics`
  each have local contribution-label width `+4px` over browser label width and local union
  `+2px/-3px` versus browser final service bbox; `web` has label width `+2px`, union width `0`,
  and union height `-3px`.
- No renderer output, layout formula, SVG fixture, or baseline behavior changed.

Touched surfaces:

- `crates/xtask/src/cmd/debug/architecture.rs`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-04-hpd-050-architecture-probe-phase-join.md`
- `target\compare\architecture-delta-probe-phase-join-hpd050`

Focused verification:

- `cargo nextest run -p xtask architecture_delta_args_accept_probe_dir architecture_probe_join_decomposes_group_and_service_bounds fcose_probe_markdown_summarizes_stage_and_node_bounds` -
  passed, `3` tests run.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_batch5_long_titles_and_punct_076 --probe-dir target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050 --out target\compare\architecture-delta-probe-phase-join-hpd050` -
  passed and wrote the automatic `pipeline` phase join.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_html_titles_and_escapes_041 --probe-dir target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050 --out target\compare\architecture-delta-probe-phase-join-hpd050` -
  passed and wrote the automatic `ui` phase join.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_unicode_and_xml_escapes_019 --probe-dir target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050 --out target\compare\architecture-delta-probe-phase-join-hpd050` -
  passed and wrote the automatic `i` phase join.
- `cargo nextest run -p xtask` - passed, `97` tests run.
- `cargo fmt --check -p xtask` - passed.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed; Architecture structural parity stayed green.
- `git diff --check` - passed.

Residual note:

- This is evidence tooling only. The new automated join reinforces the rejection of standalone
  group-padding, root-padding, group-title-bounds, final-rect-emission, and direct FCoSE compound
  rect substitution fixes for these rows. Continue from individual service label/content
  contribution width, service position drift, and their feed into final group expansion.

## HPD-050 - Architecture Service Phase Join

Outcome:

- Joined the local service contribution reports with the browser/Cytoscape final node probe for the
  active direct Architecture group-width rows.
- Decomposed the current emitted group-width tails:
  - `batch5` / `pipeline`: browser children-labels width `379.926`, local content width `382.926`,
    content `dw=+3`, expansion `dw=+2`, emitted group `dw=+5`.
  - `html_titles` / `ui`: browser `316.926`, local `319.926`, content `dw=+3`, expansion `dw=+2`,
    emitted group `dw=+5`.
  - `unicode` / `i`: browser `306.822`, local `307.822`, content `dw=+1`, expansion `dw=+2`,
    emitted group `dw=+3`.
- Confirmed why a pure group-padding change is still rejected: in the same rows, local content
  height is `-2px` versus browser children-label height, while local final expansion is `+2px`,
  giving zero emitted group `dh`.
- No code, layout formula, renderer output, SVG fixture, or baseline behavior changed.

Touched surfaces:

- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-04-hpd-050-architecture-service-phase-join.md`
- `target\compare\architecture-delta-service-contribution-hpd050`
- `target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050`

Focused verification:

- Read local service contribution rows from
  `target\compare\architecture-delta-service-contribution-hpd050\*.md`.
- Read browser group `childrenBoundingBoxIncludeLabels` and final service `node.boundingBox()` rows
  from `target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050\*.md`.
- `git diff --check` - passed with the existing `CONTEXT.jsonl` LF-to-CRLF warning only.

Residual note:

- The next source-backed candidate is not group padding or final group rect emission. It is the
  individual service label/content union width versus browser final service `node.boundingBox()`,
  including service position drift and label width rounding.

## HPD-050 - Architecture Service Contribution Report

Outcome:

- Exposed local Architecture service child contribution phases as
  `ArchitectureDiagramLayout.cytoscape_service_bounds`, preserving each service's body, label, and
  union bounds by service id and optional parent group id.
- Added a `Local Cytoscape service child bounds` table to `debug-architecture-delta` reports so the
  child content inputs to `GroupRectComputer` can be audited from Markdown instead of stderr-only
  `MERMAN_ARCH_DEBUG_GROUP_RECT` runs.
- Focused reports for the direct group-width rows now show representative local child union inputs:
  `batch5` / `pipeline` / `storage` is `225x97`, `html_titles` / `ui` / `web` is `129x97`, and
  `unicode` / `i` / `metrics` is `125x97`.
- The same reports keep the emitted local-vs-upstream group-width tails unchanged at `+5px`,
  `+5px`, and `+3px`; this is evidence tooling, not a layout or SVG output change.

Touched surfaces:

- `crates/merman-render/src/model.rs`
- `crates/merman-render/src/architecture.rs`
- `crates/merman-render/tests/architecture_layout_test.rs`
- `crates/xtask/src/cmd/debug/architecture.rs`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-04-hpd-050-architecture-service-contribution-report.md`
- `target\compare\architecture-delta-service-contribution-hpd050`

Focused verification:

- `cargo nextest run -p merman-render architecture_layout_exposes_cytoscape_service_child_bounds_by_service_id` -
  passed, `1` test run.
- `cargo nextest run -p merman-render --test architecture_layout_test` - passed, `7` tests run.
- `cargo nextest run -p xtask fcose_probe_markdown_summarizes_stage_and_node_bounds` - passed,
  `1` test run.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_batch5_long_titles_and_punct_076 --out target\compare\architecture-delta-service-contribution-hpd050` -
  passed and wrote service child contribution rows, including `storage` union `225x97`.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_html_titles_and_escapes_041 --out target\compare\architecture-delta-service-contribution-hpd050` -
  passed and wrote `web` union `129x97`.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_unicode_and_xml_escapes_019 --out target\compare\architecture-delta-service-contribution-hpd050` -
  passed and wrote `metrics` union `125x97`.
- `cargo nextest run -p merman-render --test architecture_svg_test` - passed, `7` tests run
  (`1` skipped).
- `cargo nextest run -p xtask` - passed, `95` tests run.
- `cargo fmt --check -p merman-render -p xtask` - passed.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed; Architecture structural parity stayed green.

Residual note:

- Keep `cytoscape_service_bounds` as a child-contribution evidence surface. It should not become a
  generic root-bounds source or a broad renderer-side formula without a source-backed phase model.

## HPD-050 - Architecture FCoSE Compound Bounds Output

Outcome:

- Exposed final layout-base compound rectangles from `manatee::algo::fcose::IndexedLayoutResult`
  as `compound_bounds`, then mapped them to Architecture group ids in
  `ArchitectureDiagramLayout.fcose_compound_bounds`.
- Kept SVG rendering behavior unchanged: Architecture group rect rendering still uses
  `GroupRectComputer`, and the new field is an evidence/debug seam rather than a production group
  rect source.
- `debug-architecture-delta` reports now include `Local FCoSE compound bounds vs emitted group
  rects`, comparing the local FCoSE compound rect phase with the local emitted SVG group rect phase.
- Focused reports for the active direct group-width rows show the phases are materially different:
  `pipeline` emitted width is `+107px` over the FCoSE rect, `ui` is `+44px`, and `i` is `+32px`.
  The same rows still show upstream/local emitted group-width tails of `+5px`, `+5px`, and `+3px`.
- This rejects direct substitution of local layout-base compound rects for emitted group rects. It
  narrows the evidence chain by making the local final compound phase visible.

Touched surfaces:

- `crates/manatee/src/graph/mod.rs`
- `crates/manatee/src/lib.rs`
- `crates/manatee/src/algo/fcose/mod.rs`
- `crates/merman-render/src/model.rs`
- `crates/merman-render/src/architecture.rs`
- `crates/merman-render/tests/architecture_layout_test.rs`
- `crates/xtask/src/cmd/debug/architecture.rs`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-04-hpd-050-architecture-fcose-compound-bounds-output.md`
- `target\compare\architecture-delta-fcose-compound-bounds-hpd050`

Focused verification:

- `cargo nextest run -p manatee indexed_layout_matches_string_graph_layout_for_compound_constraints` -
  passed, `1` test run.
- `cargo nextest run -p merman-render architecture_layout_exposes_fcose_compound_bounds_by_group_id` -
  passed, `1` test run.
- `cargo nextest run -p xtask fcose_probe_markdown_summarizes_stage_and_node_bounds` - passed,
  `1` test run.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_batch5_long_titles_and_punct_076 --out target\compare\architecture-delta-fcose-compound-bounds-hpd050` -
  passed; `pipeline` FCoSE-vs-emitted row reports `dw=+107px`, while emitted local-vs-upstream
  group width remains `+5px`.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_html_titles_and_escapes_041 --out target\compare\architecture-delta-fcose-compound-bounds-hpd050` -
  passed; `ui` FCoSE-vs-emitted row reports `dw=+44px`, while emitted local-vs-upstream group
  width remains `+5px`.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_unicode_and_xml_escapes_019 --out target\compare\architecture-delta-fcose-compound-bounds-hpd050` -
  passed; `i` FCoSE-vs-emitted row reports `dw=+32px`, while emitted local-vs-upstream group width
  remains `+3px`.
- `cargo nextest run -p manatee` - passed, `12` tests run.
- `cargo nextest run -p merman-render --test architecture_layout_test` - passed, `6` tests run.
- `cargo nextest run -p xtask` - passed, `95` tests run.
- `cargo fmt --check -p manatee -p merman-render -p xtask` - passed.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed; Architecture structural parity stayed green.

Residual note:

- Do not consume `fcose_compound_bounds` as a renderer group-rect replacement without a new
  source-backed phase model. The remaining direct group-width rows still point at child
  service-label/content bounds feeding `GroupRectComputer`, not at final rect emission.

## HPD-050 - Architecture Group Content Union Audit

Outcome:

- Audited the source path behind the remaining direct Architecture group-width tails after the
  label-phase join.
- Confirmed pinned Mermaid `svgDraw.ts::drawGroups(...)` draws group rectangles from final
  Cytoscape `node.boundingBox()`, then shifts `x` / `y` by `halfIconSize`; group title SVG text is
  emitted after the rect and does not drive the compound bbox.
- Confirmed local group rects are rebuilt by `GroupRectComputer` from child service/junction/group
  bounds. In-group services feed that union through
  `ArchitectureServiceBoundsEstimate.cytoscape_group_child_contribution.union_bounds`.
- Confirmed default local group inflation is `architecture.padding + 2.5px`, i.e. `42.5px` per side
  for the active default-padding rows.
- Focused `MERMAN_ARCH_DEBUG_GROUP_RECT` runs show the active direct width tails are already present
  in the child content union phase:
  - `batch5_long_titles` `pipeline`: content `(-194.463,-83.463)-(188.463,214.463)`, pad `42.5`,
    final local width `467.926` versus upstream `462.926`.
  - `html_titles` `ui`: content `(-129.963,-83.463)-(189.963,214.463)`, pad `42.5`, final local
    width `404.926` versus upstream `399.926`.
  - `unicode` `i`: content `(-131.911,-83.797)-(175.911,214.797)`, pad `42.5`, final local width
    `392.822` versus upstream `389.822`.
- No production code, layout formula, renderer output, SVG fixture, or baseline behavior changed.

Touched surfaces:

- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-04-hpd-050-architecture-group-content-union-audit.md`
- `target\compare\architecture-delta-debug-label-phase-grouprect-current`

Focused verification:

- Source reads:
  `repo-ref/mermaid/packages/mermaid/src/diagrams/architecture/svgDraw.ts`,
  `crates/merman-render/src/architecture_metrics.rs`,
  `crates/merman-render/src/svg/parity/architecture.rs`,
  `crates/merman-render/src/svg/parity/architecture/geometry.rs`,
  `crates/merman-render/src/svg/parity/architecture/nodes.rs`, and
  `crates/merman-render/src/svg/parity/architecture/viewport.rs`.
- `MERMAN_ARCH_DEBUG_GROUP_RECT=pipeline cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_batch5_long_titles_and_punct_076 --out target\compare\architecture-delta-debug-label-phase-grouprect-current` -
  passed.
- `MERMAN_ARCH_DEBUG_GROUP_RECT=ui cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_html_titles_and_escapes_041 --out target\compare\architecture-delta-debug-label-phase-grouprect-current` -
  passed.
- `MERMAN_ARCH_DEBUG_GROUP_RECT=i cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_unicode_and_xml_escapes_019 --out target\compare\architecture-delta-debug-label-phase-grouprect-current` -
  passed.

Residual note:

- Do not change group padding, root padding, group title bounds, or final group rect emission for
  the active direct width tails. The remaining seam is now narrowed to child service-label/content
  bounds feeding `GroupRectComputer`; any production candidate must be scoped there and verified
  against the full Architecture root queue.

## HPD-050 - Architecture Label Phase Join

Outcome:

- Joined the current-HEAD local Architecture delta reports with the browser/Cytoscape
  label-contribution probe batch.
- Regenerated current local delta reports for the seven representative residual samples under
  `target\compare\architecture-delta-label-phase-current-hpd050`.
- Confirmed `group_port_edges_017` is no longer an active current group/root residual after the
  narrow Procrustes compatibility slice: upstream and local max-width both report `707.769226`, and
  `group-outer` / `group-inner` have zero `dx`, `dy`, `dw`, and `dh`.
- The active direct group-width tails remain:
  - `batch5_long_titles` `pipeline`: local `dw=+5`, browser child-label contribution
    `dw=97 dh=17`, final group expansion `dw=83 dh=83`.
  - `html_titles` `ui`: local `dw=+5`, browser child-label contribution `dw=34 dh=17`,
    final group expansion `dw=83 dh=83`.
  - `unicode` `i`: local `dw=+3`, browser child-label contribution `dw=24 dh=17`, final group
    expansion `dw=83 dh=83`.
- `nested_groups` and `batch6_init` stay separate phase classes: `nested_groups` is dominated by
  placement and tiny `dw=-0.5` tails, while `batch6_init` has custom-init group sizing
  (`left dw=-3`, `right dw=-1`) plus large `dx` shifts.
- This join rejects another production formula attempt in this slice. The focused `+5px` rows are
  not explained by final group expansion alone, and prior exact labelWidth lookup evidence already
  showed label width alone reduced focused rows only to `+2px` while raising the full root queue.
- No production code, layout formula, renderer output, SVG fixture, or baseline behavior changed.

Touched surfaces:

- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-04-hpd-050-architecture-label-phase-join.md`
- `target\compare\architecture-delta-label-phase-current-hpd050`
- `target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050`

Focused verification:

- `cargo run -p xtask -- debug-architecture-delta --fixture <seven representative fixtures> --out target\compare\architecture-delta-label-phase-current-hpd050` -
  passed for all `7` fixtures.
- Current `group_port_edges_017` report shows exact upstream/local max-width and zero
  service/group deltas.
- `rg -n "| group-rect" target\compare\architecture-delta-label-phase-current-hpd050 -g "*.md"` -
  extracted current group deltas for the joined rows.

Residual note:

- Do not reopen `group_port_edges_017` unless a fresh current report regresses. The next viable
  production candidate for the remaining group rows must explain child label contribution, final
  compound group bbox, and root SVG consumption together; standalone group padding, font-family
  switching, or exact labelWidth lookup remains rejected.

## HPD-050 - Architecture Probe Label Contribution Summary

Outcome:

- Strengthened the `debug-architecture-fcose-probe` Markdown summary so group rows expose the
  complete child-body, child-label, and final-group expansion chain in one table.
- The `Final Node Bounds` table now includes `children labels over body`, computed from
  `childrenBoundingBoxIncludeLabels` minus `childrenBoundingBoxBodyOnly`.
- The existing `bb over children labels` column is unchanged, so each group row now directly shows
  `children body -> children labels -> final node.boundingBox()`.
- Regenerated the seven-fixture active Architecture residual probe batch under
  `target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050`.
- Representative rows now make the label phase explicit:
  `batch5_long_titles` `pipeline` reports
  `l=69.500 r=27.500 t=0.000 b=17.000 dw=97.000 dh=17.000`;
  `html_titles` `ui` reports
  `l=22.500 r=11.500 t=0.000 b=17.000 dw=34.000 dh=17.000`;
  `group_port_edges` `outer` reports zero label expansion over body while `inner` reports
  `b=17.000 dh=17.000`;
  `batch6` custom-init groups show asymmetric horizontal label contribution while retaining the
  final `31.5px` per-side group expansion.
- No Architecture layout, renderer, SVG, probe JSON, fixture, or baseline behavior changed.

Touched surfaces:

- `crates/xtask/src/cmd/debug/architecture.rs`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-04-hpd-050-architecture-probe-label-contribution.md`
- `target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050`

Focused verification:

- `cargo nextest run -p xtask fcose_probe_markdown_summarizes_stage_and_node_bounds` - passed,
  `1` test run.
- `cargo nextest run -p xtask` - passed, `95` tests run.
- `cargo fmt --check -p xtask` - passed.
- `cargo run -p xtask -- debug-architecture-fcose-probe --fixture stress_architecture_junction_fork_join_026 --fixture stress_architecture_batch5_long_titles_and_punct_076 --fixture stress_architecture_html_titles_and_escapes_041 --fixture stress_architecture_unicode_and_xml_escapes_019 --fixture stress_architecture_nested_groups_002 --fixture stress_architecture_batch6_init_fontsize_icon_size_wrap_093 --fixture stress_architecture_group_port_edges_017 --out-dir target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050 --browser-exe "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"` -
  passed and wrote `7` per-fixture summaries plus the batch index.
- `rg -n "children labels over body" target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050 -g "*.md"` -
  found the new column in all `7` summaries.

Residual note:

- This is source-evidence tooling only. The new label-contribution phase should be joined with
  local delta reports before any production formula change; it does not close any Architecture
  root residual by itself.

## HPD-050 - Architecture Probe Expansion Active-Residual Batch

Outcome:

- Regenerated the representative Architecture active-residual browser probe batch after the
  `bb over children labels` Markdown column was added.
- The new batch index is
  `target\compare\architecture-fcose-probe-expansion-active-residuals-hpd050\architecture-fcose-probe-batch.md`.
- The batch covers the seven current source-backed residual samples:
  `junction_fork_join_026`, `batch5_long_titles_and_punct_076`,
  `html_titles_and_escapes_041`, `unicode_and_xml_escapes_019`, `nested_groups_002`,
  `batch6_init_fontsize_icon_size_wrap_093`, and `group_port_edges_017`.
- All seven per-fixture Markdown summaries contain the new `bb over children labels` column.
- The focused standard-padding group rows (`pipeline`, `ui`, `i`, `platform`, `data`, `inner`,
  `outer`, and the large junction groups) directly report
  `l=41.500 r=41.500 t=41.500 b=41.500 dw=83.000 dh=83.000`.
- The custom-init `batch6_init_fontsize_icon_size_wrap_093` `left` / `right` groups report
  `l=31.500 r=31.500 t=31.500 b=31.500 dw=63.000 dh=63.000`, matching their smaller configured
  group expansion phase.
- No code, layout, renderer, root-bounds, SVG, fixture, or baseline behavior changed in this
  evidence-collection slice.

Touched surfaces:

- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-04-hpd-050-architecture-probe-expansion-active-batch.md`
- `target\compare\architecture-fcose-probe-expansion-active-residuals-hpd050`

Focused verification:

- `cargo run -p xtask -- debug-architecture-fcose-probe --fixture stress_architecture_junction_fork_join_026 --fixture stress_architecture_batch5_long_titles_and_punct_076 --fixture stress_architecture_html_titles_and_escapes_041 --fixture stress_architecture_unicode_and_xml_escapes_019 --fixture stress_architecture_nested_groups_002 --fixture stress_architecture_batch6_init_fontsize_icon_size_wrap_093 --fixture stress_architecture_group_port_edges_017 --out-dir target\compare\architecture-fcose-probe-expansion-active-residuals-hpd050 --browser-exe "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"` -
  passed and wrote `7` per-fixture summaries plus the batch index.
- `Select-String -Path target\compare\architecture-fcose-probe-expansion-active-residuals-hpd050\*.fcose-browser-probe.md -Pattern 'bb over children labels'` -
  found the new column in all `7` summaries.
- Focused row extraction confirmed the expected group expansion rows for `pipeline`, `ui`, `i`,
  `platform`, `data`, `left`, `right`, `inner`, `outer`, and the junction groups.
- `git diff --check` - passed with the existing `CONTEXT.jsonl` LF-to-CRLF warning only.
- Line-by-line JSON parse for `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` - passed,
  `567` JSONL records parsed.
- `docs/workstreams/headless-parity-deepening/WORKSTREAM.json` parse - passed.

Residual note:

- This batch makes the browser final-group expansion phase directly citable for the active
  Architecture residual set. It is evidence only; do not treat the uniform-looking `41.5px` rows as
  permission to tune local group padding without reconciling child contribution, final group bbox,
  and local delta reports.

## HPD-050 - Architecture Probe Group Expansion Summary

Outcome:

- Strengthened the `debug-architecture-fcose-probe` Markdown summary for Architecture group-bbox
  residual audits.
- The `Final Node Bounds` table now includes `bb over children labels`, computed from final
  browser/Cytoscape `node.boundingBox()` minus `childrenBoundingBoxIncludeLabels`.
- The new cell reports left/right/top/bottom expansion plus aggregate `dw` / `dh`, so group rows no
  longer require manual subtraction between the `bb` and `children labels` columns.
- A focused real probe for `stress_architecture_batch5_long_titles_and_punct_076` now shows
  `pipeline` expansion as `l=41.500 r=41.500 t=41.500 b=41.500 dw=83.000 dh=83.000`, making the
  final-group expansion phase explicit in the artifact.
- No Architecture layout, renderer, root-bounds, SVG, probe JSON, or fixture behavior changed.

Touched surfaces:

- `crates/xtask/src/cmd/debug/architecture.rs`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-04-hpd-050-architecture-probe-group-expansion.md`

Focused verification:

- `cargo nextest run -p xtask fcose_probe_markdown_summarizes_stage_and_node_bounds` - passed,
  `1` test run.
- `cargo nextest run -p xtask` - passed, `95` tests run.
- `cargo fmt --check -p xtask` - passed.
- `cargo run -p xtask -- debug-architecture-fcose-probe --fixture stress_architecture_batch5_long_titles_and_punct_076 --out target\compare\architecture-fcose-probe-expansion-hpd050 --browser-exe "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"` -
  passed and wrote a Markdown summary containing the new `bb over children labels` column.
- `git diff --check` - passed with the existing `CONTEXT.jsonl` LF-to-CRLF warning only.
- Line-by-line JSON parse for `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` - passed,
  `565` JSONL records parsed.
- `docs/workstreams/headless-parity-deepening/WORKSTREAM.json` parse - passed.

Residual note:

- This is source-evidence tooling only. It makes final group bbox expansion easier to cite for the
  active `+5px` Architecture rows, but it does not alter the group formula or close any root
  residual.

## HPD-050 - Graphlib Stringified-ID Boundary

Outcome:

- Audited the remaining upstream Graphlib `graph-test.js` id-stringification cases around
  `setNode`, `setParent`, `setEdge`, and undirected edge endpoint ordering.
- Confirmed the source seam is JS dynamic-argument coercion: `repo-ref/graphlib/lib/graph.js`
  converts edge endpoints with `"" + v_` / `"" + w_`, then undirected graphs canonicalize by
  string comparison.
- Confirmed local `dugong-graphlib::Graph` already exposes a typed Rust string API
  (`impl Into<String>` for setters and `&str` for lookups), so accepting arbitrary numeric/object
  ids is an explicit Rust/JS API-shape non-target unless a future FFI/raw Graphlib bridge needs it.
- Added a consumer-relevant post-coercion regression:
  `undirected_edges_follow_graphlib_string_order_for_stringified_ids` covers `"9"` / `"10"`
  endpoint ordering, verifying both lookup directions and the canonical stored edge key.
- Updated `docs/dugong/GRAPHLIB_UPSTREAM_TEST_COVERAGE.md` to map the covered undirected
  string-order subset and document the remaining numeric/object coercion boundary.
- No production Graphlib, Dagre, renderer, SVG, or fixture behavior changed.

Touched surfaces:

- `crates/dugong-graphlib/tests/graph_core_test.rs`
- `docs/dugong/GRAPHLIB_UPSTREAM_TEST_COVERAGE.md`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-04-hpd-050-graphlib-stringified-id-boundary.md`

Focused verification:

- `cargo nextest run -p dugong-graphlib undirected_edges_follow_graphlib_string_order_for_stringified_ids` -
  passed, `1` test run.
- `cargo nextest run -p dugong-graphlib` - passed, `97` tests run.
- `cargo fmt --check -p dugong-graphlib` - passed.
- `git diff --check` - passed with the existing `CONTEXT.jsonl` LF-to-CRLF warning only.
- Line-by-line JSON parse for `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` - passed,
  `562` JSONL records parsed.
- `docs/workstreams/headless-parity-deepening/WORKSTREAM.json` parse - passed.

Residual note:

- This closes the useful post-coercion undirected edge ordering slice under the current Rust API
  shape. It does not add JS-style numeric/object id coercion to Rust Graphlib APIs; that should
  only be reopened for a concrete FFI/raw input bridge.

## HPD-050 - Dagre Reference Graph-Dimension Delta

Outcome:

- Strengthened the Dagre JS/Rust reference comparison surface after the graph-dimension output seam.
- `DagreReferenceComparison` now reports absolute top-level graph `width` / `height` deltas in
  addition to node geometry, edge geometry, and identity drift.
- `compare_graph_to_js_reference(...)` reads JS reference dimensions from the Graphlib JSON
  top-level `value.width` / `value.height` fields; missing JS dimensions become infinite
  diagnostic deltas instead of silently disappearing.
- `compare-dagre-layout` now prints `graph dimension delta: width=... height=...` beside the
  existing geometry and identity summary.
- No layout, renderer, Graphlib, SVG, or fixture behavior changed in this slice.

Touched surfaces:

- `crates/xtask/src/cmd/debug/dagre_reference.rs`
- `crates/xtask/src/cmd/debug/dagre.rs`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-04-hpd-050-dagre-reference-graph-dimension-delta.md`

Focused verification:

- `cargo nextest run -p xtask dagre_reference` - passed, `6` tests run.
- `cargo fmt --check -p xtask` - passed.
- `cargo run -p xtask -- compare-dagre-layout --diagram state --fixture basic --out-dir target\compare\dagre-layout-hpd050-graph-dimension-delta` -
  passed with graph dimension delta `width=0.000000 height=0.000000`, max node delta
  `0.000000`, max edge delta `0.000000`, and zero node/edge identity drift.
- `git diff --check` - passed with the existing `CONTEXT.jsonl` LF-to-CRLF warning only.
- Line-by-line JSON parse for `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` - passed,
  `559` JSONL records parsed.
- `docs/workstreams/headless-parity-deepening/WORKSTREAM.json` parse - passed.

Residual note:

- This is reference truth-surface hardening, not a Dagre layout implementation change. It makes
  graph-level root-size drift visible in the comparison result before any future Dagre-backed
  residual audit tries to explain or tune it.

## HPD-050 - Dagre Attribute Case-Insensitivity API-Shape Audit

Outcome:

- Audited the remaining upstream `repo-ref/dagre/test/layout-test.js` case
  `treats attributes with case-insensitivity`.
- Confirmed the source seam lives in JS `buildLayoutGraph(...)`: Dagre calls `canonicalize(attrs)`
  before selecting whitelisted graph, node, and edge layout attributes, so input object keys such as
  `nodeSep` normalize to `nodesep`.
- Confirmed local Dugong does not expose an equivalent raw JS object input surface. `GraphLabel` is
  a typed Rust struct with lowercase semantic fields (`nodesep`, `ranksep`, `edgesep`, `marginx`,
  `marginy`), and Mermaid-facing renderer builders set those typed fields directly from typed
  configuration extraction.
- Recorded the upstream case as an explicit Rust API-shape non-target in
  `docs/dugong/DAGRE_UPSTREAM_TEST_COVERAGE.md`.
- No production layout, renderer, Graphlib, xtask, or SVG behavior changed.

Touched surfaces:

- `docs/dugong/DAGRE_UPSTREAM_TEST_COVERAGE.md`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-04-hpd-050-dagre-attribute-case-insensitivity.md`

Focused verification:

- `rg -n "nodeSep|nodesep|canonicalize|graphNumAttrs" repo-ref\dagre\lib repo-ref\dagre\test\layout-test.js` -
  confirmed the source test and `canonicalize(attrs)` lowering seam.
- `rg -n "GraphLabel|nodesep|ranksep|edgesep|marginx|marginy|nodeSep|rankSep|edgeSep|layout_dagreish" crates\dugong crates\merman-render crates\xtask -g "*.rs"` -
  confirmed the local typed `GraphLabel` / renderer construction path and found no raw `nodeSep`
  Dagre input bridge.
- `git diff --check` - passed with the existing `CONTEXT.jsonl` LF-to-CRLF warning only.
- Line-by-line JSON parse for `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` - passed,
  `555` JSONL records parsed.
- `docs/workstreams/headless-parity-deepening/WORKSTREAM.json` parse - passed.

Residual note:

- This closes the source audit for the upstream JS key-casing case under current Rust API shape. It
  should not be implemented as an ad hoc alias table unless a public JSON/FFI Dugong input surface
  starts accepting raw graph-label objects.

## HPD-050 - Dagreish Bounding-Box Source Coverage

Outcome:

- Continued the source-backed Dagreish layout audit from the graph-dimension output seam into
  upstream `repo-ref/dagre/test/layout-test.js` bounding-box assertions.
- Added direct `layout_dagreish(...)` coverage for the upstream node coordinate bounding-box case
  across `TB`, `BT`, `LR`, and `RL`.
- Added direct `layout_dagreish(...)` coverage for the upstream `labelpos = l` edge-label
  bounding-box case across `TB`, `BT`, `LR`, and `RL`.
- These tests exercise the full consumer path after coordinate-system undo and `translateGraph(...)`
  dimension writeback, so they are closer to root-bounds output than isolated helper tests.
- No production layout, renderer, reference adapter, or SVG behavior changed.

Touched surfaces:

- `crates/dugong/tests/layout_test.rs`
- `docs/dugong/DAGRE_UPSTREAM_TEST_COVERAGE.md`

Focused verification:

- `cargo nextest run -p dugong --test layout_test` - passed, `19` tests run.
- `cargo nextest run -p dugong` - passed, `275` tests run.
- `cargo fmt --check -p dugong` - passed.
- `git diff --check` - passed with the existing `CONTEXT.jsonl` LF-to-CRLF warning only.
- Line-by-line JSON parse for `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` - passed,
  `555` JSONL records parsed.
- `docs/workstreams/headless-parity-deepening/WORKSTREAM.json` parse - passed.

Residual note:

- This is a coverage slice for already-correct Dagreish bounding-box behavior. It does not claim
  default minimal `dugong::layout(...)` equivalence, JS object-key case-insensitivity, or
  Architecture root residual closure.

## HPD-050 - Dagreish Graph Dimension Output Seam

Outcome:

- Implemented the upstream Dagre `translateGraph(...)` graph-dimension output seam on the full
  `layout_dagreish(...)` path.
- `dugong::GraphLabel` now carries `width` and `height` output fields. `layout_dagreish(...)`
  computes them from the same source-backed bbox phase as Dagre: positioned node boxes plus edge
  label boxes with explicit `x/y`, excluding intermediate edge points, and including
  `marginx/marginy`.
- Added source-backed coverage for upstream `repo-ref/dagre/test/layout-test.js` case
  `adds dimensions to the graph`.
- Added a focused Rust regression for the margin half of Dagre's formula so a single-node graph
  with `marginx=8` / `marginy=10` reports `width=116` and `height=70`.
- Updated the Dagre reference adapter so output snapshots include graph `width` / `height`, while
  input snapshots continue omitting them. This keeps JS harness inputs clean and lets future
  Rust/JS reference artifacts expose graph-dimension drift.

Touched production surfaces:

- `crates/dugong/src/model.rs`
- `crates/dugong/src/pipeline/dagreish.rs`
- `crates/xtask/src/cmd/debug/dagre_reference.rs`
- `crates/dugong/tests/layout_test.rs`
- `docs/dugong/DAGRE_UPSTREAM_TEST_COVERAGE.md`

Focused verification:

- `cargo nextest run -p dugong --test layout_test` - passed, `17` tests run.
- `cargo nextest run -p dugong` - passed, `273` tests run.
- `cargo nextest run -p dugong-graphlib` - passed, `96` tests run.
- `cargo nextest run -p xtask dagre_reference` - passed, `5` tests run.
- `cargo run -p xtask -- compare-dagre-layout --diagram state --fixture basic --out-dir target\compare\dagre-layout-hpd050-graph-dimensions` -
  passed with max node delta `0.000000`, max edge delta `0.000000`, node identity drift
  `rust-only=0 js-only=0`, and edge identity drift `rust-only=0 js-only=0`. Generated input
  JSON omitted graph `width` / `height`; JS and Rust output graph dimensions both reported
  `100.109375 x 298`.
- `cargo run -p xtask -- compare-dagre-layout --diagram state --fixture stress_state_composite_with_external_edges_028 --out-dir target\compare\dagre-layout-hpd050-graph-dimensions-composite` -
  passed with max node delta `0.000000`, max edge delta `0.000000`, and zero node/edge identity
  drift.
- `cargo run -p xtask -- compare-dagre-layout --diagram state --fixture stress_state_composite_with_external_edges_028 --cluster state-Big-7 --out-dir target\compare\dagre-layout-hpd050-graph-dimensions-cluster` -
  passed with max node delta `0.000000`, max edge delta `0.000000`, and zero node/edge identity
  drift.
- `cargo fmt --check -p dugong -p dugong-graphlib -p xtask` - passed.
- `git diff --check` - passed with the existing `CONTEXT.jsonl` LF-to-CRLF warning only.
- Line-by-line JSON parse for `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` - passed,
  `555` JSONL records parsed.
- `docs/workstreams/headless-parity-deepening/WORKSTREAM.json` parse - passed.
- Line-by-line JSON parse for `docs/workstreams/headless-parity-deepening/TASKS.jsonl` - passed,
  `8` JSONL records parsed.
- Line-by-line JSON parse for `docs/workstreams/headless-parity-deepening/CAMPAIGNS.jsonl` -
  passed, `4` JSONL records parsed.

Residual note:

- This closes a Dagreish layout-output/root-bounds seam, not an Architecture FCoSE root residual.
  It does not make any claim about default minimal `dugong::layout(...)` graph-dimension parity or
  Mermaid SVG viewport closure.

## HPD-050 - Dagreish Layout Source Coverage

Outcome:

- Continued HPD-050 through the Dugong/Dagre source-audit lane after the Graphlib public API seam
  coverage showed no production change was needed.
- Added direct `layout_dagreish(...)` coverage for four upstream `repo-ref/dagre/test/layout-test.js`
  cases that map to the full Dagre pipeline consumed by Mermaid-facing renderers:
  `can layout a long edge with a label`, `can layout out a short cycle`,
  `minimizes separation between nodes not adjacent to subgraphs`, and
  `can layout subgraphs with different rankdirs`.
- The new tests lock three layout phases that are more relevant to State/Class/Flowchart consumers
  than ordinary Graphlib queries: edge-label coordinates on long edges, acyclic undo point
  direction after full layout, and compound subgraph geometry/rankdir behavior.
- Updated `docs/dugong/DAGRE_UPSTREAM_TEST_COVERAGE.md` so those source cases point at the direct
  Rust coverage.
- No production Dagre, Graphlib, renderer, or SVG behavior changed.

Focused verification:

- `cargo nextest run -p dugong layout_dagreish_can_layout_a_long_edge_with_a_label`
- `cargo nextest run -p dugong layout_dagreish_can_layout_a_short_cycle`
- `cargo nextest run -p dugong layout_dagreish_minimizes_separation_between_nodes_not_adjacent_to_subgraphs`
- `cargo nextest run -p dugong layout_dagreish_can_layout_subgraphs_with_different_rankdirs`
- `cargo nextest run -p dugong --test layout_test` - passed, `15` tests run.
- `cargo nextest run -p dugong` - passed, `271` tests run.
- `cargo fmt --check -p dugong -p dugong-graphlib` - passed.
- `git diff --check` - passed with the existing `CONTEXT.jsonl` LF-to-CRLF warning only.
- Line-by-line JSON parse for `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` - passed,
  `542` JSONL records parsed.
- `docs/workstreams/headless-parity-deepening/WORKSTREAM.json` parse - passed.

Residual note:

- This is a source-backed coverage slice for the full Dagreish consumer path. It does not claim
  default `dugong::layout(...)` minimal-pipeline equivalence, GraphLabel `width` / `height`
  writeback parity, or Architecture root residual closure.

## HPD-050 - Graphlib Node Optional Label Seam

Outcome:

- Continued HPD-050 through the Dugong/Graphlib source-audit lane after the Architecture
  canvas-width audit rejected standalone root-width production fixes.
- Ported the upstream Graphlib `setNode("a", undefined)` / `node("a")` optional-label seam to a
  direct Rust regression:
  `crates/dugong-graphlib/tests/graph_core_test.rs::set_node_with_optional_label_can_clear_label_without_removing_node`.
- The test locks the Rust `Option<T>` mapping used by Graphlib JSON and public graph APIs: missing
  node lookup returns `None`, while a present node with an explicitly cleared upstream
  `undefined` value is represented as `Some(&None)` and keeps `has_node("a") == true`.
- Updated `docs/dugong/GRAPHLIB_UPSTREAM_TEST_COVERAGE.md` so the relevant `graph-test.js`
  `setNode` and `node` cases now point at the direct Rust coverage.
- No production graph implementation change was needed.

Focused verification:

- `cargo nextest run -p dugong-graphlib set_node_with_optional_label_can_clear_label_without_removing_node`
- `cargo nextest run -p dugong-graphlib` - passed, `96` tests run.

Residual note:

- This is a source-backed API seam slice, not a root residual closure. It strengthens the Graphlib
  compatibility layer that Dagre-facing audits depend on while leaving Architecture numeric
  diagnostics unchanged.

## HPD-050 - Architecture Cytoscape Canvas Width Audit

Outcome:

- Confirmed the remaining `+5px`-class Architecture rows are not caused by using the wrong
  rendered SVG service-title style. Stored upstream/local SVG service titles inherit the Mermaid
  root SVG font, while Cytoscape compound child labels are a separate layout-measurement phase.
- Confirmed pinned Cytoscape's source rule for child service labels:
  `font-family: Helvetica Neue, Helvetica, sans-serif`, canvas
  `Math.ceil(ctx.measureText(...).width)`, and centered label bounds
  `labelBounds.w = labelWidth + 4`.
- An Edge/Puppeteer canvas probe exactly matched the browser/Cytoscape probe label widths for the
  two `+5px` rows:
  `149`, `133`, `217`, `77`, `123`, `86`, and `101` for the sampled labels.
- Rejected a direct local font-family switch. Local vendored Arial/Helvetica metrics do not match
  Edge canvas Arial metrics, so switching Architecture compound measurement to Cytoscape's default
  font made focused rows wider.
- Rejected an exact 169-title Cytoscape `labelWidth` lookup as a standalone production seam. It
  improved `batch5`, `html_titles`, and `unicode` to `+2px`, but left a half-source final group
  phase, raised the full Architecture root queue back to `25`, and shifted
  `batch6_init_fontsize_icon_size_wrap_093` to `-8px`.
- Rejected combining that lookup with final group extra padding `2.5px -> 1.5px`. It made focused
  widths exact but made heights `2px` short, matching the already-rejected split-axis group-padding
  path.
- No production renderer, layout, measurement, generated table, SVG output, or root override
  behavior changed; all temporary patches were reverted.

Implementation boundary:

- Do not try a font-family switch, shared font-table rebuild, exact service-label lookup table, or
  group-padding tweak by itself for the remaining Architecture width tails.
- A future candidate must model the child body, child label, final group `node.boundingBox()`, and
  root SVG consumption phases together, then survive full Architecture structural and root
  diagnostics.

Focused verification:

- Edge/Puppeteer canvas measurement for seven focused service labels - passed and matched existing
  browser/Cytoscape probe label widths.
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_batch5_long_titles_and_punct_076 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_batch5_hpd050_cytoscape_font_experiment.md` -
  expected failure; font switch worsened width delta to `+9.5px`.
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_html_titles_and_escapes_041 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_html_hpd050_cytoscape_font_experiment.md` -
  expected failure; font switch worsened width delta to `+9px`.
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_unicode_and_xml_escapes_019 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_unicode_hpd050_cytoscape_font_experiment.md` -
  expected failure; font switch worsened width delta to `+6.5px`.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_report_parity_root_hpd050_cytoscape_lookup_experiment.md` -
  expected failure; exact labelWidth lookup plus old final group phase had `25` mismatches.
- Focused exact labelWidth plus `1.5px` group-extra experiments - expected failures; widths became
  exact but heights became `2px` short.

Residual note:

- This is evidence and boundary sharpening, not residual closure. The current accepted production
  baseline remains the Procrustes slice with Architecture root queue at `24`.

## HPD-050 - Architecture Group Port Relocation And Repulsion Seam

Outcome:

- Extended the Architecture FCoSE browser probe so it can record `relocateComponent(...)`,
  first-iteration `updateDisplacements(...)`, and CoSE layout compound-node stages. The probe still
  reconstructs Architecture inputs manually; this is source-evidence tooling only, not renderer or
  layout production behavior.
- Ran a full Architecture `parity-root` diagnostic with `MANATEE_FCOSE_DISABLE_RELOCATE=1`.
  It was not a global fix: `stress_architecture_group_port_edges_017` became root-exact, but the
  mismatch count increased from `25` to `27` by adding
  `stress_architecture_batch6_junctions_multi_split_with_group_edges_087`,
  `stress_architecture_bidirectional_boundary_traversal_020`, and
  `upstream_architecture_docs_groups_within_groups`.
- Focused `group_port_edges_017` evidence now shows the first run relocation is identical between
  upstream and local:
  `orig=(0.000,8.500)`, current rect center `(26.799,22.441)`, delta
  `(-26.799,-13.941)`.
- The second run original center is also identical between upstream and local:
  `orig=(1.500,17.750)`. Therefore the next root cause is not a wrong second-run
  `eles.boundingBox()` original-center input.
- The divergence starts inside the second run's first CoSE tick, before constraint relaxation:
  upstream's `inner` compound receives `repulsion=(0,250)` and displacement `(0,30)`, while local
  receives `repulsion=(40,40)` and displacement `(6,6)`. That propagated compound displacement is
  the source of the local vertical compression.
- Source comparison points to a `layout-base` clipping / near-touching-rectangle boundary:
  upstream second-run `inner` and `out1` are separated by a tiny floating gap after
  `ConstraintHandler.handleConstraints(...)`, so `IGeometry.getIntersection(...)` takes the
  non-overlap path and produces a near-vertical minimum-distance repulsion. Local currently snaps
  the same phase into the touching/overlap path.
- No production renderer, layout, measurement, SVG output, or root override behavior changed.

Implementation boundary:

- Do not "fix" this row by globally disabling relocation. That removes this residual but creates
  new root mismatches.
- Do not globally change group padding, final group bbox padding, or `GroupRectComputer`.
- Do not globally loosen/tighten `rects_intersect(...)` or add an epsilon without family-level
  Architecture verification; the same branch can affect many compound-heavy rows.
- A production path would need a focused `layout-base` clipping/repulsion parity test first, then a
  narrowly justified `manatee` correction that survives full Architecture structural and root
  diagnostics.

Focused verification:

- `MANATEE_FCOSE_DISABLE_RELOCATE=1 cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_report_parity_root_hpd050_disable_relocate.md` -
  expected failure; `group_port_edges_017` became exact, total root mismatches became `27`.
- `cargo run -p xtask -- debug-architecture-fcose-probe --fixture stress_architecture_group_port_edges_017 --out target\compare\architecture-fcose-probe-group-port-relocate-hpd050 --browser-exe "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"` -
  passed and wrote relocation/displacement/compound-stage browser probe artifacts.
- `MANATEE_FCOSE_DEBUG_RELOCATE=1 MANATEE_FCOSE_DEBUG_ELES_BBOX=1 cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_group_port_edges_017 --out target\compare\architecture-delta-debug-group-port-relocate` -
  passed and printed local relocation centers.
- `MANATEE_FCOSE_DISABLE_RELOCATE=1 cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_group_port_edges_017 --out target\compare\architecture-delta-debug-group-port-disable-relocate` -
  passed; local group/service sizes matched upstream and remaining deltas were a uniform
  translation.
- `MANATEE_FCOSE_DEBUG_POSITIONS=1 MANATEE_FCOSE_DEBUG_POSITIONS_ALL=1 cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_group_port_edges_017 --out target\compare\architecture-delta-debug-group-port-positions-all` -
  passed and exposed local second-run compound/leaf stages.
- `MANATEE_FCOSE_DEBUG_FORCES=1 MANATEE_FCOSE_DEBUG_EDGE_FORCES=1 cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_group_port_edges_017 --out target\compare\architecture-delta-debug-group-port-forces` -
  passed and confirmed local second-run `inner` compound repulsion remains `(40,40)`.
- `cargo nextest run -p xtask fcose_probe_markdown_summarizes_stage_and_node_bounds` - passed.
- `cargo nextest run -p xtask` - passed, `94` tests run.
- `cargo fmt --check -p xtask` - passed.
- `git diff --check` - passed.
- Line-by-line JSON parse for `docs\workstreams\headless-parity-deepening\CONTEXT.jsonl` -
  passed, `529` JSONL records parsed.

Residual note:

- This is a source-backed narrowing step, not a production residual closure. It moves
  `group_port_edges_017` from "relocation maybe wrong" to "second-run compound repulsion / clipping
  boundary differs after otherwise matching original-center and pre-constraint inputs."

## HPD-050 - Architecture Procrustes Narrow Compatibility

Outcome:

- Narrowed `procrustes_transform_from_pairs(...)` to the measured Architecture group-port seam.
  The half-EPS tail now only applies when source and target positions are bitwise identical, the
  Procrustes sample has six pairs, and the covariance shape matches the measured L-shaped
  `group_port_edges_017` case. This restores the row at 3-decimal precision without adding new
  structural mismatches.
- The browser probe summary now records `leftTop` and `size` for the FCoSE node snapshots, so the
  same fixture can be re-audited with the pre/post-bounds stage data already in the artifact.

Focused verification:

- `cargo fmt --check` - passed.
- `git diff --check` - passed.
- `cargo nextest run -p manatee` - passed, 12 tests run.
- `cargo nextest run -p xtask` - passed, 94 tests run.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_group_port_edges_017 --out target\compare\architecture-delta-hpd050-procrustes-narrow` - passed.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture_report_parity_hpd050_procrustes_narrow_sequential.md` - passed.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_report_parity_root_hpd050_procrustes_narrow_sequential.md` - expected failure; the root mismatch queue dropped from `25` to `24`, and the only removed row was `stress_architecture_group_port_edges_017`.

Residual note:

- This is a targeted compatibility shim, not a blanket SVD rewrite. The remaining Architecture
  root residuals still need source-backed audits.

## HPD-050 - Architecture Group Port Source Seam

Outcome:

- Audited the local source path for `stress_architecture_group_port_edges_017` before changing any
  renderer formula.
- Confirmed local Architecture SVG group rectangles are rebuilt in
  `crates/merman-render/src/svg/parity/architecture.rs` from `GroupRectComputer`, which consumes
  service bounds, junction bounds, and recursively computed child group bounds. They do not consume
  final compound group rectangles from `manatee`.
- Confirmed Architecture root viewport finalization is driven by emitted SVG bounds plus
  renderer-owned `content_bounds` in `svg/parity/architecture/viewport.rs`; the layout-level
  `ArchitectureDiagramLayout.bounds` is not the active SVG root source.
- Confirmed pinned Mermaid 11.15 draws group rectangles from Cytoscape final
  `node.boundingBox()` in `repo-ref/mermaid/packages/mermaid/src/diagrams/architecture/svgDraw.ts`.
- A focused `MANATEE_FCOSE_DEBUG_ELES_BBOX=1` run reported local `run=1` `eles.boundingBox()` as
  `(-313.618759,-204.551469)-(316.618759,240.051469)`, height `444.602938px`. That matches both
  the browser probe `bbAfterSegments.h=444.603px` and the local outer group/root height phase.
- The stored upstream SVG outer group is still the final compound bbox phase:
  `x=-90.610885 y=-164.224041 w=447.995496 h=462.448081`.
- The row is therefore not a pure group padding miss. Local service/inner-group positions are
  vertically compressed relative to upstream by `8.922571px` on each side, which produces the full
  `-17.845142px` outer group/root height tail.
- No production renderer, layout, measurement, xtask, or SVG output behavior changed.

Source-backed boundary:

- Upstream SVG group phase: final Cytoscape compound `node.boundingBox()` from `drawGroups(...)`.
- Local group/root phase: renderer-side group reconstruction plus root `getBBox()` approximation.
- Local layout evidence phase: `manatee` / browser `bbAfterSegments` `eles.boundingBox()` around
  the FCoSE rerun and segment-stage bbox, not final compound group emission.
- Do not globally change group padding, export layout-base compound rectangles directly, or tune
  root height from `ArchitectureDiagramLayout.bounds` for this row. The next implementation path
  needs a phase-specific model that separates layout relocation bboxes, final compound group
  bboxes, and `{group}` edge endpoint position propagation.

Focused verification:

- `MANATEE_FCOSE_DEBUG_ELES_BBOX=1 cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_group_port_edges_017 --out target\compare\architecture-delta-debug-group-port-eles` -
  passed and printed the local `run=1` total bbox above.

Residual note:

- This is a source audit and implementation boundary, not a root residual closure. It prevents the
  next pass from conflating `bbAfterSegments` with final group `node.boundingBox()`.

## HPD-050 - Architecture Active Residual Phase Join

Outcome:

- Joined the existing seven-fixture browser/Cytoscape FCoSE probe batch with the local
  upstream-vs-rendered group-size delta reports. No renderer, layout, measurement constant, SVG
  output behavior, or xtask command changed.
- For all non-junction focused group rows, the browser probe final `node.boundingBox().w/h`
  matches the stored upstream SVG group rect `w/h` at report precision. That means those rows can
  use final browser group bbox as the source phase when comparing local group `dw` / `dh`.
- `junction_fork_join_026` is the exception: an explicit Edge rerun reproduced the probe geometry,
  but the stored upstream SVG group rects and service positions differ from the probe. Treat that
  row as a probe-harness / CLI-baseline divergence plus solver/phase residual before attempting a
  production formula change.
- `group_port_edges_017` exposes a concrete phase seam: local `group-outer h=444.603px` matches the
  browser probe `bbAfterSegments.h=444.603px`, while the upstream final outer group bbox is
  `462.448px`. The root height delta (`-17.845px`) follows that stage-bbox-vs-final-compound-bbox
  gap.
- The two `+5px` rows and `unicode_and_xml_escapes_019` are direct group width tails: local group
  `dw` equals the root width delta.
- `nested_groups_002` and `batch6_init_fontsize_icon_size_wrap_093` are not pure final group size
  rows. Their root width tails combine small group `dw` with nested/position propagation, so they
  need phase-specific placement/root aggregation evidence before a production fix.

Source phase join:

| fixture | group | browser final group w/h | upstream group w/h | local group dw/dh | root dw/dh |
|---|---|---:|---:|---:|---:|
| `batch5_long_titles_and_punct_076` | `pipeline` | `462.926 / 382.926` | `462.926 / 382.926` | `+5.000 / +0.000` | `+5.000 / +0.000` |
| `html_titles_and_escapes_041` | `ui` | `399.926 / 382.926` | `399.926 / 382.926` | `+5.000 / +0.000` | `+5.000 / +0.000` |
| `unicode_and_xml_escapes_019` | `i` | `389.822 / 383.593` | `389.822 / 383.593` | `+3.000 / -0.000` | `+3.000 / +0.000` |
| `nested_groups_002` | `platform` | `459.154 / 542.658` | `459.154 / 542.658` | `-0.500 / +0.000` | `+2.500 / +0.000` |
| `nested_groups_002` | `data` | `376.154 / 182.000` | `376.154 / 182.000` | `-0.500 / +0.000` | `+2.500 / +0.000` |
| `nested_groups_002` | `runtime` | `365.654 / 182.000` | `365.654 / 182.000` | `+0.000 / +0.000` | `+2.500 / +0.000` |
| `batch6_init_fontsize_icon_size_wrap_093` | `left` | `162.000 / 124.000` | `162.000 / 124.000` | `-3.000 / +0.000` | `-2.500 / +0.000` |
| `batch6_init_fontsize_icon_size_wrap_093` | `right` | `236.605 / 160.924` | `236.605 / 160.924` | `-1.000 / +0.000` | `-2.500 / +0.000` |
| `group_port_edges_017` | `outer` | `447.995 / 462.448` | `447.995 / 462.448` | `+0.030 / -17.845` | `+1.468 / -17.845` |
| `group_port_edges_017` | `inner` | `364.995 / 182.000` | `364.995 / 182.000` | `+0.030 / +0.000` | `+1.468 / -17.845` |
| `junction_fork_join_026` | `left` | `1809.785 / 1626.571` | `1788.557 / 1649.154` | `+17.331 / -18.609` | `+13.976 / -12.502` |
| `junction_fork_join_026` | `right` | `941.374 / 1017.806` | `945.473 / 1010.381` | `-3.388 / +6.107` | `+13.976 / -12.502` |

Focused verification:

- Read-only PowerShell JSON/Markdown join over
  `target\compare\architecture-fcose-probe-active-residuals-hpd050\*.json` and
  `target\compare\architecture-delta-active-residuals-hpd050-group-size\*.md` - passed and
  produced the table above.
- `cargo run -p xtask -- debug-architecture-fcose-probe --fixture junction_fork_join_026 --out target\compare\architecture-fcose-probe-junction-rerun-hpd050` - expected-failed because the
  default Puppeteer Chrome cache is absent locally.
- `cargo run -p xtask -- debug-architecture-fcose-probe --fixture junction_fork_join_026 --out target\compare\architecture-fcose-probe-junction-rerun-hpd050 --browser-exe "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"` -
  passed and reproduced the earlier `junction_fork_join_026` probe geometry.

Residual note:

- This is an evidence/classification step. It narrows the next implementation candidates to
  phase-specific fixes only: direct service/group width phase for the `+5px` / `unicode` rows, a
  final-compound-vs-layout-stage phase audit for `group_port_edges_017`, and a separate
  probe-vs-baseline harness check before using `junction_fork_join_026` as a formula target.

## HPD-050 - Architecture Delta Group Size Columns

Outcome:

- Extended `debug-architecture-delta` so group-rect rows report `dw` and `dh` explicitly and rank
  by `max(abs(dx), abs(dy), abs(dw), abs(dh))`.
- Extended `summarize-architecture-deltas` with `group max dx`, `group max dy`, `group max dw`,
  and `group max dh` columns.
- Regenerated the seven active-residual local delta reports in a new artifact directory:
  `target\compare\architecture-delta-active-residuals-hpd050-group-size`.
- Regenerated the all-fixture Architecture delta summary:
  `target\compare\architecture-delta-summary-hpd050-group-size\architecture-delta-summary.md`.
- No renderer, layout, measurement constant, SVG output behavior, browser probe behavior, or root
  residual status changed.

Touched production surfaces:

- [crates/xtask/src/cmd/debug/architecture.rs](/F:/SourceCodes/Rust/merman/crates/xtask/src/cmd/debug/architecture.rs)

Restored phase signal:

| fixture | local group phase signal now explicit |
|---|---|
| `stress_architecture_batch5_long_titles_and_punct_076` | `group-pipeline` `dw=+5.000px`, matching the root-width tail directly |
| `stress_architecture_html_titles_and_escapes_041` | `group-ui` `dw=+5.000px`, with services only `+0.500px` on X |
| `stress_architecture_unicode_and_xml_escapes_019` | `group-i` `dw=+3.000px`, with services `-1.500px` on X |
| `stress_architecture_nested_groups_002` | `group-data` / `group-platform` `dx=+4.250px`, `dw=-0.500px`, preserving the nested compound-bounds classification |
| `stress_architecture_batch6_init_fontsize_icon_size_wrap_093` | `group-left` `dx=+24.464px`, `dw=-3.000px`; services shift `+22.964px` on X |
| `stress_architecture_group_port_edges_017` | `group-outer` `dh=-17.845px`, making the vertical compression explicit |
| `stress_architecture_junction_fork_join_026` | group max `dw=+17.331px`, `dh=-18.609px`, beside service/junction placement drift |

Focused verification:

- `cargo fmt -p xtask` - applied.
- `cargo nextest run -p xtask architecture_svg_id_normalizer` - passed, `2` tests run.
- `cargo run -p xtask -- debug-architecture-delta --fixture <each of the seven active Architecture residual fixtures> --out target\compare\architecture-delta-active-residuals-hpd050-group-size` -
  passed and wrote reports with explicit `dw` / `dh` columns.
- `cargo run -p xtask -- summarize-architecture-deltas --out target\compare\architecture-delta-summary-hpd050-group-size` -
  passed and wrote the all-fixture summary with group max delta columns.
- `cargo nextest run -p xtask` - passed, `94` tests run.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture_report_parity_hpd050_delta_group_size.md` -
  passed.
- `cargo fmt --check` - passed.
- `git diff --check` - passed.
- Line-by-line JSON parse for `docs\workstreams\headless-parity-deepening\CONTEXT.jsonl` - passed,
  `515` JSONL records parsed.

Residual note:

- This makes the Rust-side local delta artifact comparable to the browser/Cytoscape probe summaries
  for group width/height phase analysis. It does not imply a production group-bbox formula change.

## HPD-050 - Architecture Delta Extractor Current ID Normalizer

Outcome:

- Repaired the local Architecture delta evidence seam after the active residual batch exposed that
  `debug-architecture-delta` still looked for legacy `service-*`, `junction-*`, and `group-*` root
  ids.
- Current Architecture SVG ids are diagram-scoped (`<diagram>-service-*`,
  `<diagram>-group-*`), and current junction groups carry the transform on a classed `<g>` while
  the stable id is on the child `<rect>` as `<diagram>-node-*`.
- Added a small id normalizer shared by `debug-architecture-delta` and
  `summarize-architecture-deltas`, preserving legacy id support while restoring current element
  extraction.
- Before the fix, the seven active-residual local delta reports only had root viewBox/max-width
  evidence and printed `services=0 junctions=0 group_rects=0`. After the fix, the same batch
  captures service, junction, and group-rect deltas for all representative rows.
- No renderer, layout, measurement constant, SVG output behavior, or browser probe output changed.

Touched production surfaces:

- [crates/xtask/src/cmd/debug/architecture.rs](/F:/SourceCodes/Rust/merman/crates/xtask/src/cmd/debug/architecture.rs)

Local delta evidence:

- `target\compare\architecture-delta-active-residuals-hpd050\stress_architecture_junction_fork_join_026.md`
- `target\compare\architecture-delta-active-residuals-hpd050\stress_architecture_batch5_long_titles_and_punct_076.md`
- `target\compare\architecture-delta-active-residuals-hpd050\stress_architecture_html_titles_and_escapes_041.md`
- `target\compare\architecture-delta-active-residuals-hpd050\stress_architecture_unicode_and_xml_escapes_019.md`
- `target\compare\architecture-delta-active-residuals-hpd050\stress_architecture_nested_groups_002.md`
- `target\compare\architecture-delta-active-residuals-hpd050\stress_architecture_batch6_init_fontsize_icon_size_wrap_093.md`
- `target\compare\architecture-delta-active-residuals-hpd050\stress_architecture_group_port_edges_017.md`
- `target\compare\architecture-delta-summary-hpd050-id-normalizer\architecture-delta-summary.md`

Restored element counts for the focused reports:

| fixture | services | junctions | group rects | leading local-vs-upstream delta |
|---|---:|---:|---:|---|
| `stress_architecture_junction_fork_join_026` | 5 | 2 | 2 | group/service/junction phase drift up to `12.358px`; no missing elements |
| `stress_architecture_batch5_long_titles_and_punct_076` | 4 | 0 | 1 | `group-pipeline` `x=-3.5px`, `w=+5px`; services `x=-0.5px` |
| `stress_architecture_html_titles_and_escapes_041` | 3 | 0 | 1 | `group-ui` `x=-1.5px`, `w=+5px`; services `x=+0.5px` |
| `stress_architecture_unicode_and_xml_escapes_019` | 4 | 0 | 1 | `group-i` `x=-4.5px`, `w=+3px`; services `x=-1.5px` |
| `stress_architecture_nested_groups_002` | 5 | 0 | 3 | `group-data` / `group-platform` `x=+4.25px`, `w=-0.5px`; services `x=+1.25px` |
| `stress_architecture_batch6_init_fontsize_icon_size_wrap_093` | 3 | 0 | 2 | services `x=+22.964px`; group widths `-3px` / `-1px` |
| `stress_architecture_group_port_edges_017` | 4 | 0 | 2 | vertical spread compressed by `17.845px`; no missing elements |

Focused verification:

- `cargo fmt -p xtask` - applied.
- `cargo nextest run -p xtask architecture_svg_id_normalizer` - passed, `2` tests run.
- `cargo run -p xtask -- debug-architecture-delta --fixture <each of the seven active Architecture residual fixtures> --out target\compare\architecture-delta-active-residuals-hpd050` -
  passed and restored non-zero service/junction/group-rect counts in the generated reports.
- `cargo run -p xtask -- summarize-architecture-deltas --out target\compare\architecture-delta-summary-hpd050-id-normalizer` -
  passed and wrote the all-fixture delta summary with populated service/junction delta columns.
- `cargo nextest run -p xtask` - passed, `94` tests run.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture_report_parity_hpd050_delta_id_normalizer.md` -
  passed.
- `cargo fmt --check` - passed.
- `git diff --check` - passed.
- Line-by-line JSON parse for `docs\workstreams\headless-parity-deepening\CONTEXT.jsonl` - passed,
  `511` JSONL records parsed.

Residual note:

- This repairs local source-backed audit visibility. It does not close root residuals, but it makes
  the next phase comparison concrete: browser/Cytoscape final node/edge/child bboxes can now be
  reviewed beside local upstream-vs-rendered service, group, and junction deltas for the same
  fixtures.

## HPD-050 - Architecture Active Residual Probe Batch

Outcome:

- Generated a fresh Architecture `parity-root` diagnostic report for current HEAD. It remains an
  expected failure with the same active `25` root-only mismatch queue; the leading rows are still
  `junction_fork_join_026` (`+13.976px`), `batch5_long_titles_and_punct_076` (`+5.000px`),
  `html_titles_and_escapes_041` (`+5.000px`), `unicode_and_xml_escapes_019` (`+3.000px`),
  `batch6_init_fontsize_icon_size_wrap_093` (`-2.500px`), `nested_groups_002` (`+2.500px`), and
  `group_port_edges_017` (`+1.468px`).
- Used the existing batch `debug-architecture-fcose-probe` command to capture those seven
  representative active residual samples in one source-backed artifact set.
- The batch wrote per-fixture raw JSON, per-fixture Markdown summaries, and
  `target\compare\architecture-fcose-probe-active-residuals-hpd050\architecture-fcose-probe-batch.md`
  as the index.
- The sampled summaries all capture the same four probe stages, and `bbBeforeRun2` equals
  `bbAfterSegments` for the sampled rows. That keeps the next audit focused on final
  Cytoscape/FCoSE node, child, and edge bounds rather than a later segment-pass bbox expansion.
- No renderer, layout, measurement constant, probe command shape, or SVG output behavior changed.

Probe index:

- `target\compare\architecture-fcose-probe-active-residuals-hpd050\architecture-fcose-probe-batch.md`

Fixture coverage:

| fixture | root delta | stages | nodes | edges | residual role |
|---|---:|---:|---:|---:|---|
| `stress_architecture_junction_fork_join_026` | `+13.976px` | 4 | 9 | 7 | largest source-input-matched solver/phase candidate |
| `stress_architecture_batch5_long_titles_and_punct_076` | `+5.000px` | 4 | 5 | 4 | group/service child-label bbox phase |
| `stress_architecture_html_titles_and_escapes_041` | `+5.000px` | 4 | 4 | 3 | group/service child-label bbox phase |
| `stress_architecture_unicode_and_xml_escapes_019` | `+3.000px` | 4 | 5 | 4 | service label / group child bbox phase |
| `stress_architecture_nested_groups_002` | `+2.500px` | 4 | 8 | 5 | nested compound-bounds phase |
| `stress_architecture_batch6_init_fontsize_icon_size_wrap_093` | `-2.500px` | 4 | 5 | 2 | custom init font/icon child-bbox phase |
| `stress_architecture_group_port_edges_017` | `+1.468px` | 4 | 6 | 4 | edge endpoint / compound-bound drift |

Focused verification:

- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_report_parity_root_hpd050_active_residual_probe_prep.md` -
  expected failure with the current `25` Architecture root-only mismatch rows.
- `cargo run -p xtask -- debug-architecture-fcose-probe --fixture stress_architecture_junction_fork_join_026 --fixture stress_architecture_batch5_long_titles_and_punct_076 --fixture stress_architecture_html_titles_and_escapes_041 --fixture stress_architecture_unicode_and_xml_escapes_019 --fixture stress_architecture_nested_groups_002 --fixture stress_architecture_batch6_init_fontsize_icon_size_wrap_093 --fixture stress_architecture_group_port_edges_017 --out-dir target\compare\architecture-fcose-probe-active-residuals-hpd050 --browser-exe 'C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'` -
  passed and wrote `7` JSON artifacts, `7` Markdown summaries, and the batch index.
- `cargo fmt --check` - passed.
- `git diff --check` - passed.
- Line-by-line JSON parse for `docs\workstreams\headless-parity-deepening\CONTEXT.jsonl` - passed,
  `506` JSONL records parsed.

Residual note:

- This closes an evidence-collection gap, not a root residual. The next implementation decision
  should compare these browser/Cytoscape final node/edge/child phases against the Rust
  measurement/root-bounds seams before changing production formulas.

## HPD-050 - Architecture FCoSE Probe Batch Index

Outcome:

- Extended batch `debug-architecture-fcose-probe` runs to write
  `architecture-fcose-probe-batch.md` in the output directory.
- The batch index lists each fixture, its raw JSON artifact, its Markdown summary artifact, and the
  captured stage/node/edge counts.
- The per-fixture JSON/Markdown artifacts remain unchanged; the index is only a navigation and
  audit overview file for multi-fixture residual probes.
- No renderer, layout, measurement constant, or SVG output behavior changed.

Touched production surfaces:

- [crates/xtask/src/cmd/debug/architecture.rs](/F:/SourceCodes/Rust/merman/crates/xtask/src/cmd/debug/architecture.rs)

Focused verification:

- `cargo nextest run -p xtask fcose_probe_batch_markdown` - passed, `1` test run.
- `cargo nextest run -p xtask fcose_probe` - passed, `6` tests run.
- `cargo nextest run -p xtask` - passed, `92` tests run.
- `cargo run -p xtask -- debug-architecture-fcose-probe --fixture stress_architecture_batch5_long_titles_and_punct_076 --fixture stress_architecture_html_titles_and_escapes_041 --fixture stress_architecture_group_port_edges_017 --out-dir target\compare\architecture-fcose-probe-batch-index-hpd050 --browser-exe 'C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'` -
  passed and wrote per-fixture JSON/Markdown artifacts plus
  `target\compare\architecture-fcose-probe-batch-index-hpd050\architecture-fcose-probe-batch.md`.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture_report_parity_hpd050_fcose_probe_batch_index.md` -
  passed.
- `cargo fmt --check` - passed.
- `git diff --check` - passed.

Residual note:

- This is batch artifact navigation infrastructure. It does not alter Architecture layout or close
  residuals, but it makes small residual-class probe batches easier to review and cite.

## HPD-050 - Architecture FCoSE Probe Batch Fixture Support

Outcome:

- Extended `debug-architecture-fcose-probe` so `--fixture` may be passed more than once.
- The command now resolves and runs each requested Architecture fixture in order, while preserving
  the existing single-fixture behavior and per-fixture JSON/Markdown artifacts.
- A focused batch run now captures the two active `+5px` group/service bbox rows plus the active
  `group_port_edges_017` edge/endpoint residual in one repeatable command.
- No renderer, layout, measurement constant, or SVG output behavior changed.

Touched production surfaces:

- [crates/xtask/src/cmd/debug/architecture.rs](/F:/SourceCodes/Rust/merman/crates/xtask/src/cmd/debug/architecture.rs)

Focused verification:

- `cargo nextest run -p xtask fcose_probe_args` - passed, `3` tests run.
- `cargo nextest run -p xtask fcose_probe` - passed, `5` tests run.
- `cargo nextest run -p xtask` - passed, `91` tests run.
- `cargo run -p xtask -- debug-architecture-fcose-probe --fixture stress_architecture_batch5_long_titles_and_punct_076 --fixture stress_architecture_html_titles_and_escapes_041 --fixture stress_architecture_group_port_edges_017 --out-dir target\compare\architecture-fcose-probe-batch-hpd050 --browser-exe 'C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'` -
  passed and wrote per-fixture JSON plus Markdown summaries for all three fixtures.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture_report_parity_hpd050_fcose_probe_batch.md` -
  passed.
- `cargo fmt --check` - passed.
- `git diff --check` - passed.

Residual note:

- This is batch evidence collection infrastructure. It reduces repeated manual probe commands for
  active Architecture residual classes but does not change or close any root residual.

## HPD-050 - Architecture FCoSE Probe Edge Summary

Outcome:

- Extended the `debug-architecture-fcose-probe` Markdown summary with a `Final Edge Bounds` table.
- The edge summary records each browser/Cytoscape final edge id, source/target endpoint ids,
  source/target directions, final edge `boundingBox()`, source/target endpoint coordinates,
  `curve-style`, `segment-weights`, `segment-distances`, and `edge-distances`.
- Generated a focused summary for `stress_architecture_group_port_edges_017`, the active
  group-port residual row whose previous classification depends on final edge endpoint and segment
  evidence.
- No renderer, layout, measurement constant, or SVG output behavior changed.

Touched production surfaces:

- [crates/xtask/src/cmd/debug/architecture.rs](/F:/SourceCodes/Rust/merman/crates/xtask/src/cmd/debug/architecture.rs)

Focused verification:

- `cargo nextest run -p xtask fcose_probe_markdown` - passed, `1` test run.
- `cargo nextest run -p xtask fcose_probe` - passed, `4` tests run.
- `cargo nextest run -p xtask` - passed, `90` tests run.
- `cargo run -p xtask -- debug-architecture-fcose-probe --fixture stress_architecture_group_port_edges_017 --out-dir target\compare\architecture-fcose-probe-edge-summary-hpd050 --browser-exe 'C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'` -
  passed and wrote JSON plus Markdown summary with `4` captured stages, `6` final nodes, and `4`
  final edges.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture_report_parity_hpd050_fcose_probe_edge_summary.md` -
  passed.
- `cargo fmt --check` - passed.
- `git diff --check` - passed.

Residual note:

- This is source-evidence ergonomics for edge/endpoint residual audits. It does not change
  Architecture edge routing or claim `group_port_edges_017` root closure.

## HPD-050 - Architecture FCoSE Probe Markdown Summary

Outcome:

- Deepened the new `debug-architecture-fcose-probe` xtask entry so each browser/Cytoscape probe now
  writes both raw JSON and a compact Markdown summary.
- The summary records fixture/source paths, Architecture config values, layout bbox stages such as
  `bbBeforeRun2` / `bbAfterSegments`, and final node rows with position, `node.boundingBox()`,
  `bodyBounds`, `labelBounds.all`, `childrenBoundingBoxIncludeLabels`, and
  `childrenBoundingBoxBodyOnly`.
- Focused probe runs for the two active `+5px` group/service bbox rows now produce directly
  reviewable summaries:
  - `stress_architecture_batch5_long_titles_and_punct_076`
  - `stress_architecture_html_titles_and_escapes_041`
- No renderer, layout, measurement constant, or SVG output behavior changed.

Touched production surfaces:

- [crates/xtask/src/cmd/debug/architecture.rs](/F:/SourceCodes/Rust/merman/crates/xtask/src/cmd/debug/architecture.rs)

Focused verification:

- `cargo nextest run -p xtask fcose_probe_markdown` - passed, `1` test run.
- `cargo nextest run -p xtask fcose_probe` - passed, `4` tests run.
- `cargo nextest run -p xtask` - passed, `90` tests run.
- `cargo run -p xtask -- debug-architecture-fcose-probe --fixture stress_architecture_batch5_long_titles_and_punct_076 --out-dir target\compare\architecture-fcose-probe-summary-hpd050 --browser-exe 'C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'` -
  passed and wrote JSON plus Markdown summary with `4` captured stages, `5` final nodes, and `4`
  final edges.
- `cargo run -p xtask -- debug-architecture-fcose-probe --fixture stress_architecture_html_titles_and_escapes_041 --out-dir target\compare\architecture-fcose-probe-summary-hpd050 --browser-exe 'C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'` -
  passed and wrote JSON plus Markdown summary with `4` captured stages, `4` final nodes, and `3`
  final edges.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture_report_parity_hpd050_fcose_probe_summary.md` -
  passed.
- `cargo fmt --check` - passed.
- `git diff --check` - passed.

Residual note:

- This is still reference-harness infrastructure, not a root residual fix. The value is that future
  source-backed Cytoscape bbox audits can inspect group and child bbox phases from a small Markdown
  table before drilling into the raw JSON.

## HPD-050 - Architecture FCoSE Browser Probe xtask Entry

Outcome:

- Promoted the manual Architecture FCoSE/Cytoscape browser probe into a reusable xtask entry:
  `debug-architecture-fcose-probe`.
- The command resolves a single Architecture fixture by filter, invokes
  `tools/debug/arch_fcose_browser_probe_fixture_025.js`, validates JSON stdout, and writes a stable
  `<fixture>.fcose-browser-probe.json` artifact under the requested output directory.
- Added an optional `--browser-exe` flag that maps to Puppeteer's
  `PUPPETEER_EXECUTABLE_PATH`, matching the existing manual Edge-backed probe workflow without
  changing the JS probe logic.
- This is reference-harness infrastructure only. No renderer, layout, measurement constant, or SVG
  output behavior changed.

Touched production surfaces:

- [crates/xtask/src/cmd/debug/architecture.rs](/F:/SourceCodes/Rust/merman/crates/xtask/src/cmd/debug/architecture.rs)
- [crates/xtask/src/cmd/debug/mod.rs](/F:/SourceCodes/Rust/merman/crates/xtask/src/cmd/debug/mod.rs)
- [crates/xtask/src/main.rs](/F:/SourceCodes/Rust/merman/crates/xtask/src/main.rs)

Focused verification:

- `cargo fmt --check` - passed.
- `cargo nextest run -p xtask fcose_probe` - passed, `3` tests run.
- `cargo nextest run -p xtask` - passed, `89` tests run.
- `cargo run -p xtask -- debug-architecture-fcose-probe --fixture stress_architecture_batch5_long_titles_and_punct_076 --out-dir target\compare\architecture-fcose-probe-hpd050 --browser-exe 'C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'` -
  passed and wrote
  `target\compare\architecture-fcose-probe-hpd050\stress_architecture_batch5_long_titles_and_punct_076.fcose-browser-probe.json`
  with `4` captured stages, `5` final nodes, and `4` final edges.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture_report_parity_hpd050_fcose_probe_xtask.md` -
  passed.
- `git diff --check` - passed.

Residual note:

- This closes a probe-harness repeatability gap, not an Architecture root residual. Future
  Cytoscape bbox audits can now cite a checked xtask artifact path instead of relying on ad hoc
  shell redirection from the raw Node script.

## HPD-050 - Architecture FCoSE Node BoundsExtras Contribution

Outcome:

- Continued the Architecture child contribution seam into the FCoSE `BoundsExtras` adapter.
- `architecture_measure_cytoscape_node_bbox_extras(...)` now derives `left` / `right` / `top` /
  `bottom` extras from an explicit expanded body, optional label, and union contribution instead of
  keeping a separate implicit `half_w` / `bottom` formula.
- The debug path behind `MERMAN_ARCH_DEBUG_CY_BBOX=1` now prints body, label, and union phases for
  the FCoSE node-bounds contribution.
- No measurement constants or output behavior changed. Architecture structural parity stayed green,
  and Architecture `parity-root` remained the existing `25` mismatch diagnostic queue.

Touched production surfaces:

- [crates/merman-render/src/architecture_metrics.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/architecture_metrics.rs)

Focused verification:

- `cargo nextest run -p merman-render architecture` - passed, `29` tests run.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture_report_parity_hpd050_fcose_contribution.md` -
  passed.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_report_parity_root_hpd050_fcose_contribution.md` -
  expected failure with the existing `25` Architecture root-only mismatches.

Residual note:

- This is a behavior-preserving phase-modeling seam. It lets FCoSE node `BoundsExtras` and SVG/root
  group service bounds speak the same body/label/union vocabulary before any future source-backed
  Cytoscape bbox formula change is attempted.

## HPD-050 - Architecture Cytoscape Child Contribution Bounds

Outcome:

- Continued HPD-050 from the child-label bounds phase cleanup without changing layout constants or
  tuning root residuals.
- Replaced the remaining single `cytoscape_group_child_bounds` service-estimate field with
  `ArchitectureCytoscapeChildContributionBounds`, which exposes:
  - `body_bounds`: emitted icon/body contribution,
  - `label_bounds`: optional Cytoscape child label phase,
  - `union_bounds`: the compound child contribution used by group sizing/root estimates.
- SVG/group service-bounds estimation and isolated top-level service root-bounds logic now consume
  `cytoscape_group_child_contribution.union_bounds`.
- The existing `MERMAN_ARCH_DEBUG_SERVICE_BOUNDS` output now prints body, label, and union phases
  separately.
- Architecture structural parity stayed green, and Architecture `parity-root` remained the existing
  `25` mismatch diagnostic queue.

Touched production surfaces:

- [crates/merman-render/src/architecture_metrics.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/architecture_metrics.rs)
- [crates/merman-render/src/svg/parity/architecture.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/architecture.rs)

Focused verification:

- `cargo fmt --check -p merman-render` - passed.
- `cargo nextest run -p merman-render architecture` - passed, `28` tests run.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture_report_parity_hpd050_child_contribution.md` -
  passed.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_report_parity_root_hpd050_child_contribution.md` -
  expected failure with the existing `25` Architecture root-only mismatches. The leading rows remain
  `junction_fork_join_026` (`+13.976px`), `batch5_long_titles_and_punct_076` (`+5.000px`), and
  `html_titles_and_escapes_041` (`+5.000px`).
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed; implemented-matrix structural parity stayed green after the Architecture child
  contribution seam.

Residual note:

- This is a phase-modeling seam. It removes a duplicated single-field abstraction and makes the
  source-backed Cytoscape child body/label union explicit, but it does not replace the headless
  measurement model or claim root residual closure.

## HPD-050 - Dagre Reference Identity Drift Detection

Outcome:

- Hardened the HPD-050 Dagre JS/Rust reference adapter after the Graphlib JSON consumer slice.
- `compare_graph_to_js_reference(...)` now tracks node and edge identity sets separately from
  coordinate/point comparison, so Rust-only and JS-only graph entries can no longer be hidden by a
  zero-delta intersection.
- JS nodes/edges that exist but lack layout coordinates or edge points now produce an infinite
  diagnostic delta instead of being skipped.
- `compare-dagre-layout` prints node and edge identity drift counts, plus concrete ids when drift
  exists.
- No renderer, solver, or layout behavior changed. The existing State `basic` Dagre JS/Rust
  comparison still reports zero geometry delta and zero identity drift.

Touched production surfaces:

- [crates/xtask/src/cmd/debug/dagre_reference.rs](/F:/SourceCodes/Rust/merman/crates/xtask/src/cmd/debug/dagre_reference.rs)
- [crates/xtask/src/cmd/debug/dagre.rs](/F:/SourceCodes/Rust/merman/crates/xtask/src/cmd/debug/dagre.rs)

Focused verification:

- `cargo fmt --check -p xtask` - passed.
- `cargo nextest run -p xtask dagre_reference` - passed, `5` tests run.
- `cargo run -p xtask -- compare-dagre-layout --diagram state --fixture basic --out-dir target\compare\dagre-layout-hpd050-reference-identity` -
  passed with max node delta `0.000000`, max edge delta `0.000000`, node identity drift
  `rust-only=0 js-only=0`, and edge identity drift `rust-only=0 js-only=0`.

Residual note:

- This is a reference-harness truth seam, not an Architecture root residual closure. It makes future
  Dagre/Graphlib audits harder to fool before broadening the adapter beyond State producers.

## HPD-050 - Architecture Child Label Bounds Seam

Outcome:

- Continued HPD-050 from the rejected Architecture child source-phase experiments. The safe move
  was a phase-boundary cleanup, not another root-width tune.
- Renamed the shared Architecture Cytoscape label seam from the generic service-label extension
  shape to `ArchitectureCytoscapeChildLabelBounds`.
- Added an explicit `bounds_for_icon(...)` helper so the Cytoscape compound child-label phase is
  represented as bounds that can be unioned with service icon bounds.
- FCoSE node `BoundsExtras` and SVG/group service-bounds estimation still use the same existing
  half-width and bottom-extension values; no production layout constants changed.
- Architecture structural parity stayed green, and Architecture `parity-root` remained the existing
  `25` mismatch diagnostic queue.

Touched production surfaces:

- [crates/merman-render/src/architecture_metrics.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/architecture_metrics.rs)

Focused verification:

- `cargo fmt --check` - passed.
- `git diff --check` - passed.
- `cargo nextest run -p merman-render architecture` - passed, `27` tests run.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture_report_parity_hpd050_child_bounds_seam.md` -
  passed.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_report_parity_root_hpd050_child_bounds_seam.md` -
  expected failure with the existing `25` Architecture root-only mismatches. The leading rows remain
  `junction_fork_join_026` (`+13.976px`), `batch5_long_titles_and_punct_076` (`+5.000px`), and
  `html_titles_and_escapes_041` (`+5.000px`).

Residual note:

- This is a behavior-preserving seam cleanup. It makes the source phase explicit but does not
  replace the headless measurement model or claim root residual closure.

## HPD-080 - All-Supported Raster Audit Gate Calibration

Outcome:

- Continued HPD-080 visible renderability scanning with raster-enabled
  `all_supported_fixtures_render_headless_resvg_safe_audit` batches.
- The first batch exposed a test-gate issue, not a production renderer defect:
  `fixtures/treemap/upstream_treemap_classdef_and_css_compiled_styles_db.mmd` has an error golden
  because pinned Mermaid 11.15 rejects the bare `classDef ... color;` token, but the manual audit
  still tried to render it through strict public rendering as a normal contentful Treemap fixture.
- Added the two Treemap classDef bare-token error-golden fixtures to the audit skip list while
  preserving strict Treemap parser behavior.
- After that calibration, supported-family filtered raster audits passed for Architecture, Block,
  C4, Class, ER, Flowchart, Gantt, GitGraph, Journey, Kanban, Mindmap, Packet, Pie, QuadrantChart,
  Radar, Requirement, Sankey, Sequence, State, Timeline, Treemap, and XYChart.
- No new production visible rendering defect was found in this pass.

Touched surfaces:

- [crates/merman/tests/resvg_safe_fixture_smoke.rs](/F:/SourceCodes/Rust/merman/crates/merman/tests/resvg_safe_fixture_smoke.rs)

Focused verification:

- `cargo fmt --check` - passed.
- `cargo nextest run -p merman --features render,raster known_error_golden_fixtures_are_skipped_by_manual_audit source_content_gate_distinguishes_accessibility_only_from_visible_content` -
  passed, `2` tests run.
- `$env:MERMAN_RESVG_SAFE_AUDIT_FAMILY='timeline,journey,requirement,gantt,treemap'; cargo nextest run -p merman --features render,raster --run-ignored ignored-only all_supported_fixtures_render_headless_resvg_safe_audit` -
  passed.
- `$env:MERMAN_RESVG_SAFE_AUDIT_FAMILY='c4,packet,pie,quadrantchart,radar,sankey,xychart'; cargo nextest run -p merman --features render,raster --run-ignored ignored-only all_supported_fixtures_render_headless_resvg_safe_audit` -
  passed.
- `$env:MERMAN_RESVG_SAFE_AUDIT_FAMILY='block,er,kanban,mindmap'; cargo nextest run -p merman --features render,raster --run-ignored ignored-only all_supported_fixtures_render_headless_resvg_safe_audit` -
  passed.
- `$env:MERMAN_RESVG_SAFE_AUDIT_FAMILY='architecture'; cargo nextest run -p merman --features render,raster --run-ignored ignored-only all_supported_fixtures_render_headless_resvg_safe_audit` -
  passed.
- `$env:MERMAN_RESVG_SAFE_AUDIT_FAMILY='class'; cargo nextest run -p merman --features render,raster --run-ignored ignored-only all_supported_fixtures_render_headless_resvg_safe_audit` -
  passed.
- `$env:MERMAN_RESVG_SAFE_AUDIT_FAMILY='sequence'; cargo nextest run -p merman --features render,raster --run-ignored ignored-only all_supported_fixtures_render_headless_resvg_safe_audit` -
  passed.
- `$env:MERMAN_RESVG_SAFE_AUDIT_FAMILY='state'; cargo nextest run -p merman --features render,raster --run-ignored ignored-only all_supported_fixtures_render_headless_resvg_safe_audit` -
  passed.
- `$env:MERMAN_RESVG_SAFE_AUDIT_FAMILY='gitgraph'; cargo nextest run -p merman --features render,raster --run-ignored ignored-only all_supported_fixtures_render_headless_resvg_safe_audit` -
  passed.
- `$env:MERMAN_RESVG_SAFE_AUDIT_FAMILY='flowchart'; cargo nextest run -p merman --features render,raster --run-ignored ignored-only all_supported_fixtures_render_headless_resvg_safe_audit` -
  passed.

Residual note:

- HPD-080 remains active, but after this pass further visible-rendering changes should be driven by
  a failing gate, a source-backed emitted-surface gap, or concrete consumer evidence rather than by
  broad speculative CSS/raster work.

## HPD-080 - C4 Headless-Shell Text Measurement

Outcome:

- Fixed a C4 text measurement environment drift between the pinned upstream SVG baselines and local
  vendored measurement.
- The stored C4 baselines match `mmdc + chrome-headless-shell` text bboxes, not Edge-backed text
  bboxes or the generic vendored default font stack.
- Added a generated C4 text lookup table keyed by normalized font family, font size, font weight,
  and exact text, generated by `xtask gen-c4-text-overrides` from upstream C4 SVG text nodes.
- C4 layout now defaults its measurement font family to Mermaid's emitted C4 default
  `"Open Sans", sans-serif`, then uses the generated headless-shell width table before falling
  back to deterministic SVG bbox measurement.
- The key `SystemAA` description in `upstream_docs_c4_c4_diagrams_001` now measures
  `532.484375px`, yielding the expected `552px` C4 box width and `1059px` root width in the layout
  golden.
- C4 layout goldens were refreshed to the pinned headless-shell baseline.
- `report-overrides --check-no-growth` now counts the generated C4 table as `201` text lookup
  entries instead of a hand-curated helper.

Touched production surfaces:

- [crates/merman-render/src/c4.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/c4.rs)
- [crates/merman-render/src/generated/c4_text_overrides_11_12_2.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/generated/c4_text_overrides_11_12_2.rs)
- [crates/xtask/src/cmd/overrides/c4.rs](/F:/SourceCodes/Rust/merman/crates/xtask/src/cmd/overrides/c4.rs)
- [crates/xtask/src/cmd/overrides/report.rs](/F:/SourceCodes/Rust/merman/crates/xtask/src/cmd/overrides/report.rs)
- `fixtures/c4/*.layout.golden.json`

Focused verification:

- `cargo fmt --check` - passed.
- `cargo nextest run -p merman-render c4` - passed, `5` tests run.
- `cargo nextest run -p merman --features render c4` - passed, `2` tests run.
- `$env:MERMAN_RESVG_SAFE_AUDIT_FAMILY='c4'; cargo nextest run -p merman --features render --run-ignored ignored-only all_supported_fixtures_render_headless_resvg_safe_audit` -
  passed, `1` filtered C4 fixture audit run.
- `cargo nextest run -p merman-render fixtures_match_layout_golden_snapshots_when_present` -
  passed, full layout snapshot gate run.
- `cargo run -p xtask -- compare-c4-svgs --check-dom --dom-decimals 3 --out target\compare\c4_report_parity_after_hpd080_text_measurement.md` -
  passed, all C4 fixtures matched structurally.
- `cargo run -p xtask -- compare-c4-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --out target\compare\c4_report_parity_root_after_hpd080_text_measurement.md` -
  passed, all C4 fixtures matched in root mode.
- `cargo run -p xtask -- report-overrides --check-no-growth` - passed.
- `cargo nextest run -p xtask` - passed, `84` tests run.
- `git diff --check` - passed.

Residual note:

- This is a C4-scoped measurement seam. It should not be generalized into shared font metrics or
  broad browser-emulation constants unless a future shared text backend can reproduce the pinned
  headless-shell widths across families without exact text lookup rows.

## HPD-080 - Mindmap Look/Theme Data Seam And Neo DOM

Outcome:

- Fixed the source-backed Mindmap `look` seam across parser data, typed render data, and SVG
  output instead of only adding a renderer-side attribute.
- Pinned Mermaid 11.15 `MindmapDb.getData()` copies `conf.look` into node and edge layout data.
  Local Mindmap previously hardcoded `"default"` into both compatibility JSON and typed render
  models, so official `look: "neo"` data never reached the final render seam.
- Local Mindmap now projects `MermaidConfig.look` into nodes and edges. Default snapshots move from
  `"default"` to Mermaid's configured default `"classic"`; `look: "neo"` reaches both JSON and
  typed render models.
- Restored the adjacent upstream default-shape rule: default Mindmap nodes use `rounded` under
  `redux*` themes and `defaultMindmapNode` otherwise.
- Local SVG now emits `data-look="neo"` on Mindmap nodes and edges only for the `neo` look. This
  keeps default/classic structural DOM parity clean while letting Mermaid 11.15's
  `[data-look="neo"]` selectors reach matching current DOM.
- Mindmap CSS now emits the source-backed `neo` node, root, edge, drop-shadow, and gradient
  branches from `mindmap/styles.ts`; scoped gradient defs are emitted only when
  `useGradient`, `gradientStart`, and `gradientStop` are present, matching `mindmapRenderer.ts`.
- Golden refresh was intentionally narrowed after rejecting broad order-only fixture churn: the
  final fixture diff is limited to `fixtures/mindmap/*.golden.json`, where recursive JSON
  comparison found only `model.nodes[].look` and `model.edges[].look` changing from `"default"` to
  `"classic"`.
- Public renderability smoke now proves real SVG output has matching `data-look="neo"` node/edge
  DOM, source-backed `neo` CSS selectors, drop-shadow CSS, and gradient defs.

Source evidence:

- `repo-ref/mermaid/packages/mermaid/src/diagrams/mindmap/mindmapDb.ts`
- `repo-ref/mermaid/packages/mermaid/src/diagrams/mindmap/styles.ts`
- `repo-ref/mermaid/packages/mermaid/src/diagrams/mindmap/mindmapRenderer.ts`

Touched production surfaces:

- [crates/merman-core/src/diagrams/mindmap/db.rs](/F:/SourceCodes/Rust/merman/crates/merman-core/src/diagrams/mindmap/db.rs)
- [crates/merman-core/src/diagrams/mindmap/parse.rs](/F:/SourceCodes/Rust/merman/crates/merman-core/src/diagrams/mindmap/parse.rs)
- [crates/merman-render/src/svg/parity/mindmap.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/mindmap.rs)
- [crates/merman/tests/theme_renderability_smoke.rs](/F:/SourceCodes/Rust/merman/crates/merman/tests/theme_renderability_smoke.rs)
- `fixtures/mindmap/*.golden.json`

Focused verification:

- `cargo nextest run -p merman-core mindmap` - passed, `33` tests run.
- `cargo nextest run -p merman-render mindmap` - passed, `9` tests run.
- `cargo nextest run -p merman --features render --test theme_renderability_smoke mindmap_neo_theme_smoke_counts_data_look_dom_and_neo_css_as_visible` -
  passed, `1` test run.
- `cargo nextest run -p merman --features render --test theme_renderability_smoke` - passed, `12`
  tests run.
- `cargo nextest run -p merman-core --test snapshots` - passed, `1` fixture-snapshot gate run.
- `cargo run -p xtask -- compare-mindmap-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\mindmap_report_parity_after_hpd080_look_theme.md` -
  passed, all Mindmap fixtures matched structurally.
- `cargo run -p xtask -- compare-mindmap-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\mindmap_report_parity_root_after_hpd080_look_theme.md` -
  expected-failed on the existing `4` Mindmap root residual rows:
  `zed_pr_57644_mindmap`, `upstream_docs_example_icons_br`,
  `upstream_examples_mindmap_basic_mindmap_001`, and
  `upstream_docs_tidy_tree_example_usage_002`.
- `cargo fmt -p merman-core -p merman-render -p merman --check` - passed.
- `git diff --check` - passed.

Residual note:

- This is a Mindmap `look/theme -> render` source seam fix, not a root-bounds tune. The known
  Mindmap `parity-root` residuals remain diagnostic and should not be forced through `look`,
  gradient, or CSS logic.
- `classic` is now the semantic model default because that is Mermaid's configured default. The
  final SVG intentionally omits `data-look` for classic/default output to preserve current
  structural parity and avoid adding inert attributes.

## HPD-080 - Timeline Redux Visible DOM Theme Consumption

Outcome:

- Fixed a real Timeline visible-DOM seam for official `redux*` themes.
- Pinned Mermaid 11.15 `timeline/styles.js` switches `redux*` themes into
  `genReduxSections(...)`, where current node paths consume `mainBkg`, `nodeBorder`, and
  `strokeWidth`, labels consume `nodeBorder` and `fontWeight`, and `.lineWrapper line` consumes
  `nodeBorder` plus `strokeWidth`.
- Pinned `timeline/svgDraw.js` also changes redux node geometry to sharp-corner paths and skips the
  classic node divider line. Local output previously kept the classic CSS/DOM branch, so
  `theme: "redux"` could parse the right theme values while visible Timeline nodes and lines still
  behaved like classic sections.
- Local Timeline now switches its CSS and node DOM rendering on the active `redux*` theme branch,
  while keeping Mermaid's presentational `stroke="black"` / `stroke-width="2"` line attributes that
  the stylesheet overrides.
- Public dark-theme smoke now counts Timeline redux node and line colors only when matching current
  `.timeline-node section-*`, `.node-bkg`, and `.lineWrapper line` DOM exists.

Source evidence:

- `repo-ref/mermaid/packages/mermaid/src/diagrams/timeline/styles.js`
- `repo-ref/mermaid/packages/mermaid/src/diagrams/timeline/svgDraw.js`
- `repo-ref/mermaid/packages/mermaid/src/diagrams/timeline/timelineRenderer.ts`
- Fresh Mermaid CLI output in `target/compare/timeline_redux_hpd080_upstream.svg` showed the
  source-backed redux CSS branch and matching current DOM.

Touched production surfaces:

- [crates/merman-render/src/svg/parity/timeline.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/timeline.rs)
- [crates/merman-render/tests/timeline_svg_test.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/tests/timeline_svg_test.rs)
- [crates/merman/tests/theme_renderability_smoke.rs](/F:/SourceCodes/Rust/merman/crates/merman/tests/theme_renderability_smoke.rs)

Focused verification:

- `cargo nextest run -p merman-render --test timeline_svg_test` - passed, `2` tests run.
- `cargo nextest run -p merman --features render --test theme_renderability_smoke` - passed, `11`
  tests run.
- `cargo run -p xtask -- compare-timeline-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\timeline_report_parity_after_hpd080_redux_theme.md` -
  passed, all Timeline fixtures matched structurally.
- `cargo run -p xtask -- compare-timeline-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --out target\compare\timeline_report_parity_root_after_hpd080_redux_theme.md` -
  expected-failed on the existing `3` Timeline max-width/root residual rows:
  `timeline_stress_accdescr_block_multiline`, `timeline_stress_width_large_and_long_labels`, and
  `upstream_long_word_wrap`.
- `cargo fmt -p merman-render -p merman --check` - passed.

Residual note:

- Timeline arrowhead marker paths remain unthemed because pinned Mermaid 11.15 emits the same bare
  marker path in this branch; this slice fixes visible node/line branch selection, not marker-color
  policy.
- The Timeline `parity-root` rows remain browser/root-width diagnostic tails and should not be
  tuned through redux CSS.

## HPD-080 - State Visible Rough-Path Theme Consumption

Outcome:

- Re-audited State theme coverage against pinned Mermaid 11.15 source and current local SVG DOM.
- Confirmed Mermaid 11.15 `state/styles.js` expects themed `.node rect`, `.node polygon`,
  `.node .fork-join`, `.node circle.state-end`, and `.statediagram-note rect` surfaces, but current
  local State output renders many of those visible shapes as rough inline `<path>` pairs instead.
- Confirmed the stylesheet already emitted the right Mermaid 11.15 tokens, but ordinary State,
  choice, fork/join, end, and note visible surfaces still used stale hardcoded inline
  fill/stroke/stroke-width defaults, so CSS/provider parity alone did not recolor the current DOM.
- Added `StateThemeDefaults` sourced from `effective_config`, threaded it through
  `StateRenderCtx`, and applied those defaults only at final visible SVG attribute emission for the
  current rough State surfaces.
- Kept rough geometry caches color-free: `StateRoughCacheKey` and cached rough path/circle `d`
  values still depend only on geometry and seed, not theme colors.
- Preserved existing baseline and override behavior: the default Mermaid rough stroke still stays at
  `1.3` when `strokeWidth` remains the default `1`, explicit `style` / `classDef` overrides still
  serialize as `!important` style attributes on themed rough paths, and focused State compare
  parity stayed green.

Source evidence:

- `repo-ref/mermaid/packages/mermaid/src/diagrams/state/styles.js`
- `crates/merman-render/src/svg/parity/state/style.rs`
- `crates/merman-render/src/svg/parity/state/node.rs`

Focused verification:

- `cargo fmt --check` - passed.
- `cargo nextest run -p merman-render --test state_svg_test state_svg_honors_theme_options_on_visible_rough_paths` -
  passed, `1` test run.
- `cargo nextest run -p merman-render --test state_svg_test` - passed, `3` tests run.
- `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\state_report_parity_after_hpd080_state_inline_theme.md` -
  passed, all fixtures matched.
- `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\state_report_parity_root_after_hpd080_state_inline_theme.md` -
  passed, no structural root regression.
- `cargo nextest run -p merman --features render --test theme_renderability_smoke` - passed, `10`
  tests run.
- `git diff --check` - passed.

Residual note:

- State public dark-theme smoke still proves top-level renderability signals, but honest protection
  for the rough render-path seam now lives in focused `state_svg_test` assertions over the final
  visible `<path>` / `<circle>` attributes.
- Neo gradient/drop-shadow and dependency-marker rules remain deferred until local State output
  emits the corresponding support DOM.

## HPD-080 - ER Visible Signal Boundary

Outcome:

- Re-audited ER public theme smoke coverage against pinned Mermaid 11.15 source and current local
  SVG DOM.
- Confirmed current ER labels in the compact sample are XHTML `span` labels inside
  `foreignObject`, not native `<text>` labels.
- Confirmed `tertiaryColor` still emits `.relationshipLabelBox` provider CSS, but current compact
  output has no `relationshipLabelBox` DOM. The visible tertiary path is the `.labelBkg` rgba fade.
- Confirmed `textColor` still emits native edge-label text CSS, but current edge labels are XHTML
  spans. The visible label color path in this sample is `nodeTextColor` through `.label`.
- Tightened the public smoke so ER visible colors are counted through current DOM-consumed
  surfaces: relationship lines/markers, current node shapes, XHTML node labels, `.labelBkg`, and
  XHTML edge labels.
- No production renderer change was needed. This is a smoke-honesty calibration for an already
  covered style-provider family.

Source evidence:

- `repo-ref/mermaid/packages/mermaid/src/diagrams/er/styles.ts`
- `repo-ref/mermaid/packages/mermaid/src/diagrams/er/erRenderer.js`
- `repo-ref/mermaid/packages/mermaid/src/diagrams/er/erDb.ts`

Focused verification:

- `cargo fmt` - passed.
- `cargo fmt --check` - passed.
- `cargo nextest run -p merman --features render --test theme_renderability_smoke er_theme_smoke_counts_current_xhtml_label_and_edge_dom_as_visible` -
  passed, `1` test run.
- `cargo nextest run -p merman --features render --test theme_renderability_smoke` - passed, `10`
  tests run.
- `Get-Content ... CONTEXT.jsonl | ConvertFrom-Json` - passed, `364` JSONL lines parsed.
- `git diff --check` - passed with only the existing `CONTEXT.jsonl` LF/CRLF working-copy warning.

Residual note:

- ER remains covered for current node, relationship, marker, XHTML label, and edge-label DOM. Future
  public-smoke additions should count `.relationshipLabelBox`, native `.edgeLabel .label text`, or
  `data-color-id` rules as visible only when the fixture emits matching current DOM.

## HPD-080 - Mindmap Visible Signal Boundary

Outcome:

- Re-audited Mindmap public theme smoke coverage against pinned Mermaid 11.15 source and current
  local SVG DOM.
- Confirmed the compact sample's current labels are XHTML `span` nodes, not native `<text>` nodes,
  so `gitBranchLabel0` root native-text CSS is provider coverage rather than a current visible
  signal.
- Confirmed `cScale0` / `cScaleLabel0` root-section CSS is emitted for `.section--1`, but the
  compact root node also has `section-root`, and the later `.section-root` rules override the root
  fill/span path for the current sample.
- Tightened the public smoke so Mindmap visible colors are counted through current DOM-consumed
  surfaces: root `git0` fill, redux root `nodeBorder` via `.section-root span`, and child
  `cScale1` / `cScaleLabel1` / `cScaleInv1` through `.section-0` shape/span/line DOM.
- No production renderer change was needed. This is a smoke-honesty calibration for an already
  covered style-provider family.

Source evidence:

- `repo-ref/mermaid/packages/mermaid/src/diagrams/mindmap/styles.ts`
- `repo-ref/mermaid/packages/mermaid/src/diagrams/mindmap/mindmapDb.ts`
- `repo-ref/mermaid/packages/mermaid/src/diagrams/mindmap/mindmapRenderer.ts`

Focused verification:

- `cargo fmt` - passed.
- `cargo fmt --check` - passed.
- `cargo nextest run -p merman --features render --test theme_renderability_smoke mindmap_theme_smoke_counts_current_span_and_child_section_dom_as_visible` -
  passed, `1` test run.
- `cargo nextest run -p merman --features render --test theme_renderability_smoke` - passed, `9`
  tests run.
- `Get-Content ... CONTEXT.jsonl | ConvertFrom-Json` - passed, `361` JSONL lines parsed.
- `git diff --check` - passed with only the existing `CONTEXT.jsonl` LF/CRLF working-copy warning.

Residual note:

- Mindmap remains covered for current root/span/section DOM. Future public-smoke additions should
  count `cScale0` / `cScaleLabel0`, `gitBranchLabel0`, or `data-look` rules as visible only when
  the fixture emits a matching current DOM surface and the rule is not overwritten by a later
  same-specificity rule.

## HPD-080 - Packet And Sankey Visible Signal Boundary

Outcome:

- Re-audited Packet and Sankey public theme smoke coverage against pinned Mermaid 11.15 source and
  current local SVG DOM.
- Confirmed Packet visible style signals are source-backed CSS selectors with matching current DOM:
  `.packetBlock`, `.packetLabel`, `.packetByte.start`, `.packetByte.end`, and `.packetTitle`.
- Confirmed Sankey visible style signals are source-backed CSS/inline paths with matching current
  DOM: outlined `.sankey-label-bg` / `.sankey-label-fg`, node `<rect>` fills from
  `sankey.nodeColors`, and `.link` groups.
- Added a public `HeadlessRenderer` smoke test that fails if those colors are counted without the
  matching current DOM classes/elements.
- No production renderer change was needed. This is a smoke-honesty calibration for two already
  covered style-provider families.

Source evidence:

- `repo-ref/mermaid/packages/mermaid/src/diagrams/packet/styles.ts`
- `repo-ref/mermaid/packages/mermaid/src/diagrams/packet/renderer.ts`
- `repo-ref/mermaid/packages/mermaid/src/diagrams/sankey/styles.js`
- `repo-ref/mermaid/packages/mermaid/src/diagrams/sankey/sankeyRenderer.ts`

Focused verification:

- `cargo fmt` - passed.
- `cargo fmt --check` - passed.
- `cargo nextest run -p merman --features render --test theme_renderability_smoke packet_and_sankey_theme_smoke_count_dom_consumed_selectors_as_visible` -
  passed, `1` test run.
- `cargo nextest run -p merman --features render --test theme_renderability_smoke` - passed, `8`
  tests run.
- `Get-Content ... CONTEXT.jsonl | ConvertFrom-Json` - passed, `358` JSONL lines parsed.
- `git diff --check` - passed with only the existing `CONTEXT.jsonl` LF/CRLF working-copy warning.

Residual note:

- Packet and Sankey remain covered for their current DOM shapes. Future public-smoke additions
  should only count config/theme colors when the fixture renders the corresponding class or inline
  node fill surface.

## HPD-080 - C4 Visible Signal Boundary

Outcome:

- Re-audited C4 theme/renderability after the hardcoded-color scan found current-output C4 shapes
  using inline colors rather than provider CSS.
- Confirmed pinned Mermaid 11.15 `c4/styles.js` emits only `.person`, while
  `svgDraw.js` renders current C4 shapes under `class="person-man"` and visible shape colors are
  inline `c4.*_bg_color` / `c4.*_border_color` values or per-shape `UpdateElementStyle(...)`
  values.
- Added a public `HeadlessRenderer` smoke test that proves:
  - `themeVariables.personBkg` / `personBorder` still appear in the source-backed `.person`
    provider CSS,
  - current C4 output does not emit `class="person"`, so that provider rule is not counted as a
    visible renderability signal,
  - `c4` inline config colors reach visible system shapes,
  - `UpdateElementStyle(...)` and `UpdateRelStyle(...)` colors reach visible shape, label, line,
    and relationship-label output.
- No production renderer change was needed. This is a visible-signal calibration that prevents
  future C4 smoke tests from pretending inert provider CSS proves user-visible theme coverage.

Source evidence:

- `repo-ref/mermaid/packages/mermaid/src/diagrams/c4/styles.js`
- `repo-ref/mermaid/packages/mermaid/src/diagrams/c4/svgDraw.js`
- `repo-ref/mermaid/packages/mermaid/src/diagrams/common/svgDrawCommon.ts`

Focused verification:

- `cargo nextest run -p merman --features render --test theme_renderability_smoke c4_theme_smoke_counts_inline_config_and_style_macros_as_visible`
- `cargo nextest run -p merman --features render --test theme_renderability_smoke`
- `cargo fmt`

Residual note:

- C4 remains covered through its current inline config/style-macro render path. Do not promote
  generic Mermaid `themeVariables.personBkg` / `personBorder` into C4 shapes unless upstream C4
  source or emitted DOM changes to make `.person` a current visible selector.

## HPD-080 - Sequence Activation Geometry Seam

Outcome:

- Refactored the source-backed Sequence activation geometry rules introduced by the autonumber and
  nested-endpoint fixes into shared helpers.
- `sequence_activation_start_x(...)` now owns Mermaid's stacked activation start offset formula.
- `sequence_activation_stack_bounds(...)` now owns Mermaid's full-stack min-left / max-right bounds
  fold with the actor-center fallback.
- Layout activation state, SVG activation-rectangle planning, and SVG autonumber marker placement
  now consume the same helpers instead of repeating the formulas.
- Added helper unit tests for empty, single, and stacked activation bounds.

Source evidence:

- `repo-ref/mermaid/packages/mermaid/src/diagrams/sequence/sequenceRenderer.ts`

Focused verification:

- `cargo nextest run -p merman-render activation_start_x_matches_mermaid_stack_offsets activation_stack_bounds_fold_full_active_stack sequence_autonumber_anchors_to_current_activation_bounds_like_mermaid_11_15 sequence_layout_nested_activation_bounds_include_full_stack_like_mermaid_11_15`
- `cargo nextest run -p merman-render --test sequence_svg_test`
- `cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo nextest run -p merman-render`

Residual note:

- This is a seam consolidation, not a new visual-diff claim. It exists to keep the already
  source-backed Sequence activation rules from diverging across layout and SVG render phases.

## HPD-080 - Sequence Nested Activation Bounds

Outcome:

- Continued the Sequence activation audit after the autonumber marker fix and found the same
  Mermaid 11.15 bounds rule was missing from the layout pass.
- Reproduced a visible endpoint defect with nested activations: when a left-side actor targets a
  participant with two active activation rectangles, local layout used only the innermost left edge
  while Mermaid uses the minimum left edge across the full active stack.
- Updated `SequenceActivationState::actor_bounds(...)` to fold all active activation rectangles and
  return the min-left / max-right pair used by Mermaid `activationBounds(...)`.
- Added a focused layout regression that fails on the old `center - 3px` nested endpoint and passes
  on the source-backed outer-left-bound endpoint.

Source evidence:

- `repo-ref/mermaid/packages/mermaid/src/diagrams/sequence/sequenceRenderer.ts`

Focused verification:

- `cargo nextest run -p merman-render sequence_layout_nested_activation_bounds_include_full_stack_like_mermaid_11_15`
- `cargo nextest run -p merman-render --test sequence_svg_test`
- `cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo run -p xtask -- update-layout-snapshots --filter ...` for the five affected Sequence
  activation fixtures
- `cargo nextest run -p merman-render`
- `cargo fmt -p merman-render --check`
- `git diff --check`

Residual note:

- This is Sequence endpoint geometry parity, not a text measurement or root-bounds change.
  Existing Sequence measurement residuals remain open and should not be tuned through activation
  endpoint code.

## HPD-080 - Sequence Autonumber Activation Bounds

Outcome:

- Reproduced the user-visible Sequence autonumber defect with an activation sample: local numbers
  `2` and `4` anchored at the right edge of the Server activation rect, while `5` anchored at the
  left edge; Mermaid 11.15 anchors those three numbers in the opposite left/right positions.
- Confirmed the source rule in pinned Mermaid 11.15: `autonumberX` is computed from current
  `activationBounds(...)`, `fromBounds` / `toBounds`, arrow direction, and reverse-arrow type.
  It is not the message line's first point.
- Updated the Sequence SVG renderer to keep a render-pass activation-bounds stack while iterating
  messages. `ACTIVE_START` / `ACTIVE_END` directives update the stack, and ordinary message
  autonumber markers now use the Mermaid 11.15 bounds formula.
- Centralized SVG `activationWidth` parsing in `SequenceRenderSettings` so activation rectangles
  and autonumber marker placement share one config value.
- Added a focused regression proving numbers `2` and `4` sit at `activationLeft + 1`, while `5`
  sits at `activationRight - 1`, for the reported sample.

Source evidence:

- `repo-ref/mermaid/packages/mermaid/src/diagrams/sequence/sequenceRenderer.ts`

Focused verification:

- `cargo nextest run -p merman-render sequence_autonumber_anchors_to_current_activation_bounds_like_mermaid_11_15`
- `cargo nextest run -p merman-render --test sequence_svg_test`
- `cargo nextest run -p merman-render`
- `cargo fmt -p merman-render --check`
- `git diff --check`

Residual note:

- This is a visible marker-position fix, not a Sequence root-width or font-metric parity claim.
  Sequence measurement/root residuals remain governed by the residual taxonomy instead of being
  forced through marker-coordinate changes.

## HPD-080 - Info Raster Font Fallback

Outcome:

- Diagnosed the Ubuntu-only `boundary_fixtures_render_headless_resvg_safe` failure for the bare
  `info` fixture with `fontFamily: courier`.
- Confirmed against pinned Mermaid source that `info` is a visible diagram, not metadata-only:
  Mermaid's `infoRenderer.ts` appends version text and configures `width="100%"` plus
  `max-width: 400px` without a root `viewBox`.
- Fixed the raster integration path instead of weakening the source-content gate. PNG/JPEG `usvg`
  options now install browser-like font fallback over loaded system fonts and bind missing generic
  aliases to real faces when possible.
- No-`viewBox` SVGs with `max-width: Npx` now parse with a matching default viewport width, reducing
  platform-specific clipping/bounds behavior for Mermaid's `configureSvgSize(..., true)` output.
- Added a regression test proving a missing requested font family still rasterizes visible text.
- The later HPD-090 test-hygiene follow-up changed only that regression test's synthetic visible
  version text from a stale hardcoded `v11.12.2` to `PINNED_MERMAID_BASELINE_VERSION`; the raster
  fallback behavior and assertion stayed unchanged.

Source evidence:

- `repo-ref/mermaid/packages/mermaid/src/diagrams/info/infoRenderer.ts`
- `repo-ref/mermaid/packages/mermaid/src/setupGraphViewbox.js`
- `fontdb` documentation/source notes that it provides matching, not browser-style fallback.

Focused verification:

- `cargo test -p merman --features raster render::raster::tests -- --nocapture`
- `cargo nextest run -p merman --features render,raster --test resvg_safe_fixture_smoke boundary_fixtures_render_headless_resvg_safe`

Residual note:

- This keeps raster output host-font-backed. It does not claim exact browser font metrics or full
  host-font independence. The deeper version would bundle an explicit fallback font; this slice
  fixes the CI-visible blank output without changing parity SVG output.

## HPD-080 - GitGraph Official Theme Color Generation

Outcome:

- Re-audited the user-provided multi-branch GitGraph merge sample and found the default render was
  readable, finite, and covered by existing DOM/fixture smoke.
- The source-backed gap was in official theme handling: Mermaid 11.15's `git/styles.js` does not
  use the classic `git0` / `gitBranchLabel0` / `gitInv0` rules for `neo` and `redux*` themes. It
  switches to `genColor(...)`, with separate rules for `redux`, `redux-color`, `neo`, and dark
  variants.
- Updated local GitGraph CSS generation so:
  - classic/default themes keep the existing per-branch `git0..7` behavior;
  - `redux` / `redux-dark` use `nodeBorder`, `mainBkg`, redux font weight, `strokeWidth`, and the
    `4 2` branch dash pattern;
  - `redux-color` / `redux-dark-color` use `borderColorArray` for colored branches and the
    Mermaid dark-theme `mainBkg` label-fill rule;
  - `neo` / `neo-dark` use the first-branch `nodeBorder` rule, subsequent `git*` colors, scoped
    gradient-backed label backgrounds, `mainBkg` merge/reverse/highlight-inner fills, and the
    color-generation dash pattern.
- Added scoped GitGraph gradient defs when the active theme variables require them, matching the
  current Mermaid CLI `neo` output and avoiding a broken `url(#...-gradient)` reference.
- Added public `HeadlessRenderer` coverage for `redux` and `neo` GitGraph theme output.

Source evidence:

- `repo-ref/mermaid/packages/mermaid/src/diagrams/git/styles.js`
- Fresh Mermaid CLI evidence in `target/compare/gitgraph_redux_audit_upstream.svg` showed
  `redux` consumes `nodeBorder`, `mainBkg`, `noteFontWeight`, `strokeWidth`, and the `4 2` branch
  dash pattern.
- Fresh Mermaid CLI evidence in `target/compare/gitgraph_neo_audit_upstream.svg` showed `neo`
  emits `<defs><linearGradient id="...-gradient" ...>` after `<g/>` and consumes that gradient
  through `.label*` branch-label background rules.

Focused verification:

- `cargo nextest run -p merman-render gitgraph_css`
- `cargo nextest run -p merman --features render --test theme_renderability_smoke gitgraph_official_themes_use_mermaid_11_15_color_generation`
- `cargo run -p xtask -- compare-gitgraph-svgs --check-dom --dom-mode parity --dom-decimals 3`

Residual note:

- This slice does not change GitGraph layout geometry, branch indexing, commit ids, root bounds, or
  font measurement. Those remain separate parity surfaces. It only fixes source-backed CSS/defs
  that current GitGraph DOM can consume.

## HPD-080 - Gantt Visible Signal Smoke Calibration

Outcome:

- Re-audited the public Gantt dark-theme smoke after the visible-signal tightening found that the
  compact sample had only a `done` task while still counting ordinary task colors as visible.
- Confirmed local Gantt output already emits source-backed Mermaid 11.15 selectors for ordinary
  task state, done task state, and outside task labels. The issue was sample representativeness, not
  a production renderer defect.
- Tightened the public smoke source so one compact Gantt diagram now includes:
  - a wide ordinary task consuming `taskBkgColor`, `taskBorderColor`, and `taskTextColor`;
  - a narrow long-label ordinary task emitting `taskTextOutsideRight taskTextOutside0`, consuming
    `taskTextOutsideColor`;
  - a done task consuming `doneTaskBkgColor` and `doneTaskBorderColor`.
- Added a focused public render test documenting that Gantt visible theme signals should be counted
  only when matching state/label DOM exists.

Source evidence:

- `repo-ref/mermaid/packages/mermaid/src/diagrams/gantt/styles.js`
- Local rendered evidence in `target/compare/gantt_visible_audit3.svg` showed
  `class="task task0"`, `class="taskTextOutsideRight taskTextOutside0 ..."`, and
  `class="task done0"` in the same compact sample.

Focused verification:

- `cargo nextest run -p merman --features render --test theme_renderability_smoke gantt_theme_smoke_counts_normal_and_done_task_dom_as_visible`
- `cargo nextest run -p merman --features render --test theme_renderability_smoke`
- `cargo run -p xtask -- compare-gantt-svgs --check-dom --dom-mode parity --dom-decimals 3`

Residual note:

- This is a measurement-quality fix. Gantt's provider CSS remains source-backed; future public
  smoke additions should keep `taskTextOutsideColor` and state-specific task colors out of the
  visible-signal list unless the sample actually emits matching outside/state DOM.

## HPD-080 - Requirement Visible Signal Audit And Neo Node Border

Outcome:

- Re-audited Requirement theme renderability after the public dark-theme smoke was still counting
  provider CSS colors that current Requirement DOM does not consume.
- Confirmed against pinned Mermaid 11.15 source and a fresh Mermaid CLI render that `.reqBox`,
  `.reqTitle`, `.reqLabel`, `.reqLabelBox`, and `.relationshipLabel` can be emitted by
  `requirement/styles.js` without matching current node or edge-label DOM in the ordinary
  `requirementDiagram` render path.
- Tightened the public smoke so Requirement now counts only current visible surfaces in the compact
  sample: relationship line/marker color, edge-label background, `look: neo` node border, and
  stroke width. It no longer treats legacy Requirement provider colors as visible just because the
  stylesheet contains them.
- Fixed a real local visible gap found during that audit: `look: neo` Requirement output now emits
  the `data-look="neo"` / `outer-path` / `divider` DOM surfaces needed for the Mermaid 11.15
  `nodeBorder` selector to affect current node and divider paths. The DOM attributes are limited
  to the `neo` path so default Requirement structural parity remains green.

Source evidence:

- `repo-ref/mermaid/packages/mermaid/src/diagrams/requirement/styles.js`
- `repo-ref/mermaid/packages/mermaid/src/rendering-util/rendering-elements/shapes/requirementBox.ts`
- Fresh Mermaid CLI evidence in `target/compare/requirement_theme_audit_upstream.svg` showed
  `data-look="neo"`, `outer-path`, `.relationshipLine`, `.labelBkg`, and the neo node path selector
  are consumed, while `.reqBox` / `.reqTitle` / `.relationshipLabel` remain provider-only for the
  current DOM shape.

Focused verification:

- `cargo nextest run -p merman-render requirement_css_honors_mermaid_11_15_theme_options`
- `cargo nextest run -p merman --features render --test theme_renderability_smoke`
- `cargo run -p xtask -- compare-requirement-svgs --check-dom --dom-mode parity --dom-decimals 3`

Residual note:

- Requirement still emits several upstream provider rules that are not visible in the current DOM.
  Keep them as provider coverage, not public renderability signals, unless a future renderer change
  emits matching elements/classes or a source-backed Mermaid change makes those rules visible.

## HPD-080 - Journey And Timeline Visible Signal Audit Tightening

Outcome:

- Re-audited Journey theme renderability after a source-backed hardcoded-color scan found that
  `user-journey/styles.js` emits several inherited Flowchart-like rules that current Journey DOM
  does not consume.
- Tightened the public dark-theme smoke so Journey no longer counts `lineColor`,
  `edgeLabelBackground`, `mainBkg`, `nodeBorder`, or `titleColor` CSS tokens as visible signals
  merely because they appear in the stylesheet.
- Kept visible Journey coverage focused on the emitted surfaces that current SVG actually consumes:
  generic line/label/legend text color, face fill, task/section fill types, and actor colors.
- Added a focused public render test documenting the boundary: Mermaid 11.15 still emits a black
  presentation attribute on the Journey activity line, but the visible plain-line stroke is driven
  by the scoped `line { stroke: textColor }` rule; `.flowchart-link`, `.edgeLabel`, and
  `.arrowheadPath` remain inert without matching Journey DOM.
- Re-audited the Timeline case in the same public smoke and found the same measurement-quality
  issue: it counted `.disabled` CSS colors even though the compact Timeline source emitted no
  disabled DOM. The public Timeline smoke now counts visible section colors (`cScale0`,
  `cScaleLabel0`, `cScaleInv0`) instead, while a focused test keeps disabled CSS emission covered
  as provider coverage rather than visible renderability coverage.

Source evidence:

- `repo-ref/mermaid/packages/mermaid/src/diagrams/user-journey/styles.js` emits generic Journey
  rules plus inherited `.edgePath .path`, `.flowchart-link`, `.edgeLabel`, `.cluster text`, `.node`
  and `.arrowheadPath` rules.
- `repo-ref/mermaid/packages/mermaid/src/diagrams/user-journey/journeyRenderer.ts` emits the final
  activity line with `stroke="black"` and no `flowchart-link` / `edgePath` / `edgeLabel` class.
- `repo-ref/mermaid/packages/mermaid/src/diagrams/user-journey/svgDraw.js` emits task lines,
  actor circles, faces, task/section classes, and marker paths without an `.arrowheadPath` class.
- `repo-ref/mermaid/packages/mermaid/src/diagrams/timeline/styles.js` emits `.disabled` rules, but
  compact ordinary Timeline output emits `timeline-node section-*`, `node-bkg`, `node-line-*`, and
  `lineWrapper` elements rather than `class="disabled"`.

Focused verification:

- `cargo nextest run -p merman --features render --test theme_renderability_smoke journey_theme_smoke_does_not_count_inert_flowchart_rules_as_visible timeline_theme_smoke_counts_section_dom_not_disabled_css_as_visible representative_dark_theme_diagrams_keep_visible_theme_signals`

Residual note:

- This is a measurement-quality fix rather than a renderer DOM change. If merman later chooses to
  make any currently inert Journey provider rule visible as a deliberate headless improvement, that
  should be tracked as an explicit DOM/support change instead of being hidden inside the public
  smoke.

## HPD-080 - Pie Theme Merge and Treemap Error Semantics

Outcome:

- Confirmed the user-reported CI failures were stale relative to current HEAD:
  `sequence_default_message_widths_match_mermaid_default_font_family` has already been replaced by
  the calibrated `sequence_default_message_widths_use_current_sequence_svg_bbox_facts`, and
  `fixtures_match_golden_snapshots` now includes the Class namespace facade snapshot update.
- Fixed a fresh Pie structural compare regression where frontmatter with an unrelated
  `themeVariables` override caused local slice fill to use `hsl(240, 100%, 86.275%)` instead of
  Mermaid 11.15's raw `#ECECFF`.
- Corrected the Treemap classDef bare-token assumption. Pinned upstream renders
  `classDef c fill:#ff0000, stroke:rgb(1\,2\,3), color;` as an error diagram, so local parsing now
  rejects bare style tokens rather than accepting DB-layer `addClass` tolerance as parser parity.
- Refreshed only the affected Pie layout goldens and Treemap semantic/layout goldens.

Source evidence:

- `repo-ref/mermaid/packages/mermaid/src/themes/theme-default.js` sets `this.pie1 =
  this.pie1 || this.primaryColor` and `this.pie2 = this.pie2 || this.secondaryColor`, so merman
  must preserve the raw user/default color strings for those two slots.
- `repo-ref/mermaid/packages/mermaid/src/diagrams/treemap/db.ts` has tolerant style splitting, but
  the pinned upstream SVG baseline for
  `fixtures/upstream-svgs/treemap/upstream_treemap_classdef_and_css_compiled_styles_db.svg`
  is an `aria-roledescription="error"` diagram. The parser/render result, not DB helper tolerance,
  is the parity contract for this fixture.

Focused verification:

- `cargo nextest run -p merman-render sequence_default_message_widths_use_current_sequence_svg_bbox_facts`
- `cargo nextest run -p merman-core --test snapshots fixtures_match_golden_snapshots`
- `cargo nextest run -p merman-core default_theme_merges_unrelated_theme_variable_overrides_without_hsl_rewriting_pie_base default_theme_preserves_user_overrides_after_derivation supported_theme_defaults_match_upstream_snapshot`
- `cargo nextest run -p merman-core treemap_classdef_rejects_bare_label_style_tokens_like_mermaid_parser`
- `cargo nextest run -p merman-render --test treemap_svg_test treemap_classdef_bare_label_style_token_renders_error_like_mermaid_parser`
- `cargo nextest run -p merman-render --test layout_snapshots_test fixtures_match_layout_golden_snapshots_when_present`
- `cargo run -p xtask -- compare-pie-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo run -p xtask -- compare-treemap-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo nextest run --workspace --all-features`

Verification notes:

- Full SVG structural parity is green after the Pie/Treemap fixes.
- Workspace tests passed: `1681/1681` run, `1681` passed, `6` skipped.
- The `cargo nextest run --workspace --all-features` warning about `svg v0.7.2` future
  incompatibility is pre-existing dependency noise, not a test failure.

## HPD-020 - Baseline Registry

Outcome:

- Added [crates/merman-core/src/baseline.rs](/F:/SourceCodes/Rust/merman/crates/merman-core/src/baseline.rs)
  as the explicit baseline truth seam for:
  - pinned Mermaid tag
  - pinned Mermaid version
  - pinned Mermaid version suffix
  - explicit legacy generated suffix
- `Engine::default()` now uses pinned-baseline registry constructors rather than
  `default_mermaid_11_12_2*`.
- `DetectorRegistry`, `DiagramRegistry`, and `RenderDiagramRegistry` now expose pinned-baseline
  constructors as the live API, with `default_mermaid_11_12_2*` retained only as deprecated
  compatibility aliases.
- xtask importers, bench entrypoints, and baseline report labeling now route through the pinned
  baseline path instead of presenting `11.12.x` names as current truth.
- Historical generated filenames and lookup modules are still legacy-suffixed; that is now explicit
  provenance, not the live baseline identity.

Touched production surfaces:

- [crates/merman-core/src/lib.rs](/F:/SourceCodes/Rust/merman/crates/merman-core/src/lib.rs)
- [crates/merman-core/src/detect/mod.rs](/F:/SourceCodes/Rust/merman/crates/merman-core/src/detect/mod.rs)
- [crates/merman-core/src/diagram/mod.rs](/F:/SourceCodes/Rust/merman/crates/merman-core/src/diagram/mod.rs)
- [crates/xtask/src/cmd/overrides/report.rs](/F:/SourceCodes/Rust/merman/crates/xtask/src/cmd/overrides/report.rs)
- [crates/xtask/src/cmd/root_override_audit.rs](/F:/SourceCodes/Rust/merman/crates/xtask/src/cmd/root_override_audit.rs)
- [crates/xtask/src/cmd/import/docs.rs](/F:/SourceCodes/Rust/merman/crates/xtask/src/cmd/import/docs.rs)
- [crates/xtask/src/cmd/import/examples.rs](/F:/SourceCodes/Rust/merman/crates/xtask/src/cmd/import/examples.rs)
- [crates/xtask/src/cmd/import/html.rs](/F:/SourceCodes/Rust/merman/crates/xtask/src/cmd/import/html.rs)
- [crates/xtask/src/cmd/import/pkg_tests.rs](/F:/SourceCodes/Rust/merman/crates/xtask/src/cmd/import/pkg_tests.rs)
- [crates/xtask/src/cmd/import/cypress.rs](/F:/SourceCodes/Rust/merman/crates/xtask/src/cmd/import/cypress.rs)
- [crates/merman/benches/pipeline.rs](/F:/SourceCodes/Rust/merman/crates/merman/benches/pipeline.rs)

Focused verification:

- `cargo fmt --all`
- `cargo test -p merman-core baseline --lib`
- `rg -n "default_mermaid_11_12_2|default_mermaid_11_12_2_full|default_mermaid_11_12_2_tiny" crates -g '!target/**'`

Verification notes:

- The remaining `default_mermaid_11_12_2*` hits are compatibility alias definitions only; live
  call sites have been moved to the pinned-baseline constructors.
- An attempted `cargo test -p xtask ... --lib` check was invalid because `xtask` has no library
  target. The baseline-report path was instead verified by code inspection plus the existing
  `pinned_mermaid_baseline_label_reads_lockfile_ref` unit test in
  `crates/xtask/src/cmd/overrides/report.rs`.

## HPD-030 - Residual Taxonomy

Outcome:

- Added an explicit six-class residual taxonomy to
  [docs/workstreams/headless-parity-deepening/DESIGN.md](/F:/SourceCodes/Rust/merman/docs/workstreams/headless-parity-deepening/DESIGN.md):
  - source-backed behavior gap
  - generated measurement gap
  - browser lattice tail
  - stale baseline or stale override
  - solver / phase residual
  - scope boundary
- Applied that taxonomy to the active root residual lane in
  [docs/workstreams/mermaid-11-15-root-viewport-residuals/DESIGN.md](/F:/SourceCodes/Rust/merman/docs/workstreams/mermaid-11-15-root-viewport-residuals/DESIGN.md).
- Froze the intended interpretation of the current active buckets:
  - Flowchart: mostly browser lattice tails
  - Architecture: main solver/phase residual front, plus some measurement tails
  - Sequence: mixed generated-measurement gaps and browser tails
  - Class: generated-measurement gap plus stale-table audit front
  - Timeline/Journey: browser tails unless stronger evidence emerges

Validation basis:

- Current active counts and classifications were derived from:
  - [docs/workstreams/mermaid-11-15-root-viewport-residuals/HANDOFF.md](/F:/SourceCodes/Rust/merman/docs/workstreams/mermaid-11-15-root-viewport-residuals/HANDOFF.md)
  - [docs/workstreams/mermaid-11-15-root-viewport-residuals/DESIGN.md](/F:/SourceCodes/Rust/merman/docs/workstreams/mermaid-11-15-root-viewport-residuals/DESIGN.md)
  - [docs/alignment/STATUS.md](/F:/SourceCodes/Rust/merman/docs/alignment/STATUS.md)
  - [docs/quality/ARCHITECTURE_ISSUES_2026-06-01.md](/F:/SourceCodes/Rust/merman/docs/quality/ARCHITECTURE_ISSUES_2026-06-01.md)

Verification notes:

- This task intentionally does not claim new residual counts beyond the current authoritative
  workstream state.
- The deliverable is a durable classification system and queue-shaping mapping, not a fresh one-off
  report or a pseudo-precise completion metric.

Gate-tier follow-up:

- Added an explicit headless parity gate tier policy to
  [docs/workstreams/headless-parity-deepening/DESIGN.md](/F:/SourceCodes/Rust/merman/docs/workstreams/headless-parity-deepening/DESIGN.md)
  and
  [docs/workstreams/mermaid-11-15-root-viewport-residuals/DESIGN.md](/F:/SourceCodes/Rust/merman/docs/workstreams/mermaid-11-15-root-viewport-residuals/DESIGN.md).
- Hard gates are parser/semantic/error behavior, theme/CSS readability, structural DOM parity, and
  no blank/hidden/clipped/miscolored output.
- Strong alignment targets are source-backed layout topology and reusable measurement/root-bounds
  seams that improve a family as a whole.
- `parity-root` is a diagnostic and regression sensor for browser-derived numeric tails; it becomes
  a production-fix requirement only when the residual is source-backed, visible/user-facing, stale,
  or explained by a reusable seam that survives family verification.
- This policy was written after the Architecture child source-phase experiment proved that a raw
  Cytoscape source formula can improve two rows while expanding the full Architecture root report
  from `25` to `100` mismatches.

## HPD-040 - Measurement / Root Bounds Platform

Outcome:

- Moved the SVG emitted-bounds scanner from the State renderer submodule into
  [crates/merman-render/src/svg/parity/emitted_bounds.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/emitted_bounds.rs).
  This matches its real ownership: it is shared root-bounds infrastructure for State,
  Architecture, and GitGraph, not State-specific rendering logic.
- Extracted Sequence note final wrapping into `sequence_note_final_wrapped_lines(...)` and final
  wrapped-text measurement into `measure_sequence_note_final_text(...)`.
- Reused the Sequence note final wrap path from layout, root-bounds, and SVG rendering so Mermaid
  source-backed note wrapping is not re-derived in three places.
- Removed the now-unneeded crate-level re-export of Sequence note slack constants. The constants
  remain internal to the Sequence seam and its tests.
- Added no new fixture-keyed text tables, root overrides, or ad hoc parity constants.

Touched production surfaces:

- [crates/merman-render/src/svg/parity.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity.rs)
- [crates/merman-render/src/svg/parity/emitted_bounds.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/emitted_bounds.rs)
- [crates/merman-render/src/svg/parity/state/mod.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/state/mod.rs)
- [crates/merman-render/src/sequence.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/sequence.rs)
- [crates/merman-render/src/sequence/notes.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/sequence/notes.rs)
- [crates/merman-render/src/sequence/root_bounds.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/sequence/root_bounds.rs)
- [crates/merman-render/src/svg/parity/sequence/notes.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/sequence/notes.rs)

Focused verification:

- `cargo fmt --all`
- `cargo test -p merman-render svg_emitted_bounds --lib`
- `cargo test -p merman-render sequence_long_leftof_notes_keep_mermaid_11_15_note_width --test sequence_svg_test`
- `cargo test -p merman-render architecture_text_constants_match_mermaid --lib`
- `cargo run -p xtask -- report-overrides --check-no-growth`

Negative / residual evidence:

- `cargo nextest run -p merman-render svg_emitted_bounds` was attempted first but the local toolchain
  does not have `cargo-nextest` installed, so verification used `cargo test`.
- `cargo test -p merman-render sequence_long_leftof_notes_keep_mermaid_11_15_root_width --test sequence_svg_test`
  still fails. This is intentional evidence that HPD-040 did not claim forced root parity closure.
- Deterministic CLI render for
  `upstream_cypress_sequencediagram_spec_should_render_long_notes_wrapped_inline_left_of_actor_026`
  produced `max-width: 570px` / `viewBox="-170 -10 570 412"` while the upstream target remains
  `566px`.
- `cargo run -p xtask -- compare-sequence-svgs --filter upstream_cypress_sequencediagram_spec_should_render_long_notes_wrapped_inline_left_of_actor_026 --report-root --report-root-all`
  reported headless vendored local `585.000px` vs upstream `566.000px` (`+19.000px`). Treat this as
  a Sequence measurement/root residual for later classification, not as a reason to add a local
  width override.

## HPD-050 - Architecture Layout Engine Audit

First slice outcome:

- Extracted Architecture's FCoSE node-bounds adapter from the main Architecture layout function.
  After the later HPD-050 seam cleanup this adapter is named
  `architecture_fcose_node_bounds_extras(...)` and owns only the part the renderer actually feeds
  into `manatee`: per-node `BoundsExtras` for Cytoscape
  `compound-sizing-wrt-labels: include` approximation.
- Removed the layout-view group-title field. Current source/evidence says group titles are rendered
  inside compound bounds and do not participate in the pre-layout center used for FCoSE relocation;
  final SVG rendering still reads titles from semantic model data.
- Added direct unit coverage for the node-bounds helper so service label border/bottom extras stay
  explicit.
- This is an adapter-boundary refactor, not a residual-count claim. It keeps Mermaid/Cytoscape
  approximation policy in `merman-render` instead of leaking another diagram-specific rule into
  `manatee`.

Focused verification:

- `cargo fmt --all`
- `cargo test -p merman-render architecture_fcose_node_bounds_extras_feed_label_bounds --lib`
- `cargo test -p merman-render architecture_relative_constraints_preserve_mermaid_duplicate_bfs_pops --lib`
- `cargo test -p merman-render --test architecture_layout_test`
- `cargo run -p xtask -- report-overrides --check-no-growth`
- `git diff --check`

Residual evidence:

- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_batch5_long_titles_and_punct_076 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_batch5_after_prelayout_adapter.md`
  still fails with the known root-only tail: upstream `542.926px`, local `547.926px`, delta
  `+5.000px`.
- The unchanged focused tail is intentional evidence that this pass moved ownership boundaries
  without silently tuning Architecture root widths.

Second slice outcome:

- Re-audited `stress_architecture_batch4_init_small_icons_061` against Mermaid source and the
  existing browser-probe evidence instead of treating it as a service-label scale problem.
- Mermaid's `svgDraw.ts` renders Architecture edge labels through `createText(...)` and then
  rotates Y-axis labels with `transform="translate(... ) rotate(-90)"`; the root viewport comes
  from `setupGraphViewbox(svg.getBBox() + padding)`.
- The old local root-bounds model treated edge-label bboxes as centered AABBs. That missed the
  positive local `createText` y-range, which becomes a rightward x-extension after `rotate(-90)`.
- Extracted `architecture_create_text_bbox_y_range_px(...)` in
  [crates/merman-render/src/architecture_metrics.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/architecture_metrics.rs)
  and made Architecture edge-label plans carry transformed `Bounds` instead of only centered
  width/height pairs.
- Corrected `architecture_create_text_compound_label_extra_bottom_px(...)` to the source-backed
  `fontSize + 1px` rule. The previous `fontSize * 17 / 16` formula was only equivalent at the
  default `16px` font size and undercounted custom Architecture font sizes such as `12px`.
- Added regression coverage for the small-icon fixture: service/group sizing remains icon-floor
  dominated, but the vertical edge label now contributes to the root width and the compound label
  bottom follows `architecture.fontSize + 1px`.

Focused verification:

- `cargo fmt --all`
- `cargo test -p merman-render architecture_text_constants_match_mermaid --lib`
- `cargo test -p merman-render architecture_vertical_edge_label_bounds_use_create_text_y_offsets --test architecture_svg_test`
- `cargo test -p merman-render --test architecture_svg_test`
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_batch4_init_small_icons_061 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_batch4_small_icons_hpd050_edge_label_bounds.md`
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target/compare/architecture_report_parity_after_hpd050_edge_label_bounds.md`
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_report_parity_root_after_hpd050_edge_label_bounds.md`
- `cargo run -p xtask -- report-overrides --check-no-growth`
- `git diff --check`

Residual evidence after the second slice:

- The focused small-icon row is now root-green: upstream and local both report
  `187.859x191.571`.
- The full Architecture structural `parity` gate remains green.
- Full Architecture `parity-root` still fails, but the mismatch count dropped from `29` to `26`.
  `stress_architecture_batch4_init_small_icons_061`,
  `stress_architecture_batch4_init_fontsize_wrap_063`, and
  `stress_architecture_edge_label_corner_cases_012` are now `+0.000` root delta rows.
- The remaining top Architecture residuals are still led by
  `stress_architecture_junction_fork_join_026` (`+13.976px`),
  `stress_architecture_batch5_long_titles_and_punct_076` (`+5.000px`), and
  `stress_architecture_html_titles_and_escapes_041` (`+5.000px`). These remain open and should not
  be closed by constants without new source-backed evidence.

Third slice classification:

- Rechecked `stress_architecture_junction_fork_join_026`, the largest remaining Architecture
  root residual, after the edge-label bounds fix.
- Local debug still feeds the source-backed FCoSE inputs already documented in M15RV-089:
  ungrouped junction parents, 9 relative-placement constraints including duplicate `join -> db`
  and `join -> cache`, configured group padding, and the current `eles.boundingBox()` relocation
  approximation.
- The old saved Mermaid browser probe
  [target/compare/arch_junction_fork_join_probe_m15rv089.json](/F:/SourceCodes/Rust/merman/target/compare/arch_junction_fork_join_probe_m15rv089.json)
  has final service positions that match the current local SVG to floating-point noise.
- A fresh `check-upstream-svgs` run using Edge as `PUPPETEER_EXECUTABLE_PATH` reproduced the stored
  upstream SVG fixture exactly: both report `max-width: 2808.126708984375px` and
  `viewBox="-1362.063232421875 -1213.2674560546875 2808.126708984375 2557.534912109375"`.
- Therefore the previous "stored upstream baseline drift" reading was too broad. The old saved debug
  browser probe is the divergent path here: its service positions match local output, but differ
  from the current CLI/Edge baseline by about `7-10px` on X and `6-12px` on Y.
- Treat the remaining `+13.976px` root tail as a probe-harness / CLI-harness divergence plus a
  solver/phase residual candidate. Do not tune `manatee` against the saved probe alone, and do not
  refresh or discard the stored fixture on the basis of that probe.

Focused verification:

- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_junction_fork_join_026 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_junction_fork_join_hpd050_debug.md`
- `$env:PUPPETEER_EXECUTABLE_PATH='C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'; cargo run -p xtask -- check-upstream-svgs --diagram architecture --filter stress_architecture_junction_fork_join_026 --check-dom --dom-mode parity-root --dom-decimals 3`
- `$env:PUPPETEER_EXECUTABLE_PATH='C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'; cargo run -p xtask -- check-upstream-svgs --diagram architecture --filter stress_architecture_junction_fork_join_026 --check-dom --dom-mode parity --dom-decimals 3`
- PowerShell JSON/SVG comparison of the old saved
  `target/compare/arch_junction_fork_join_probe_m15rv089.json`
  final positions against the local SVG showed deltas at floating-point noise level.
- The same comparison against
  `fixtures/upstream-svgs/architecture/stress_architecture_junction_fork_join_026.svg` showed the
  debug probe / CLI baseline split: e.g. probe-minus-fixture deltas are `auth.x=+10.376px`,
  `cache.x=+10.376px`, `api.y=-12.358px`, and `db.y=-12.358px`.

Fourth slice seam cleanup:

- Audited the remaining `+5px` Architecture root rows
  `stress_architecture_batch5_long_titles_and_punct_076` and
  `stress_architecture_html_titles_and_escapes_041` against saved Mermaid browser probes and the
  current upstream/local SVGs.
- For both rows, upstream service positions match the saved browser probe while local service
  positions differ only by about `0.5px` in X. The root-width delta is controlled by the final
  group rectangle:
  - `batch5_long_titles`: upstream group width `462.925633px`, local `467.925633px`
  - `html_titles`: upstream group width `399.925633px`, local `404.925633px`
- The shared old name `architecture_compound_bbox_padding_px(...)` implied one padding policy for
  multiple Cytoscape phases. That was misleading. Mermaid's final group rect path reads
  `node.boundingBox()` in `svgDraw.ts`, while manatee's relocation/element bbox approximation is a
  separate layout-engine phase.
- Renamed the renderer helper to `architecture_svg_group_bbox_padding_px(...)` and removed the
  unused renderer-side `initial_center` / pre-layout group bbox model. Relocation-centering remains
  owned by `manatee`'s indexed graph adapter, where the actual layout consumes it.
- This was an honesty/refactor slice, not a root-width tune. The two focused `+5px` rows remain
  open as group/service Cytoscape bbox measurement residuals until generated browser evidence or a
  better deterministic canvas-bbox seam justifies narrowing the approximation.

Focused verification:

- `cargo fmt --all`
- `cargo test -p merman-render architecture_fcose_node_bounds_extras_feed_label_bounds --lib`
- `cargo test -p merman-render architecture_svg_group_bbox_padding_adds_headless_cytoscape_extra --lib`
- `cargo test -p merman-render architecture_text_constants_match_mermaid --lib`
- `cargo test -p merman-render --test architecture_svg_test`
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_batch5_long_titles_and_punct_076 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_batch5_hpd050_bounds_extras_refactor.md`
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_html_titles_and_escapes_041 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_html_titles_hpd050_bounds_extras_refactor.md`
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target/compare/architecture_report_parity_after_hpd050_bounds_extras_refactor.md`
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_report_parity_root_after_hpd050_bounds_extras_refactor.md`
- `cargo run -p xtask -- report-overrides --check-no-growth`
- `git diff --check`

Fifth slice probe-harness correction:

- Re-audited the Architecture FCoSE browser probe after `junction_fork_join_026` showed a
  probe/fixture split.
- The actual installed baseline package
  `tools/mermaid-cli/node_modules/mermaid/package.json` is `mermaid@11.15.0`, and the generated
  `dist/mermaid.js` used by `check-upstream-svgs` does not contain the later `withSeededRandom`
  Architecture source path seen in `repo-ref/mermaid/packages/mermaid/src/diagrams/architecture`.
  It still uses the xtask page-level deterministic prelude and the Architecture FCoSE config fields
  `randomize`, `nodeSeparation`, `idealEdgeLengthMultiplier`, `edgeElasticity`, and `numIter`.
- Updated
  [tools/debug/arch_fcose_browser_probe_fixture_025.js](/F:/SourceCodes/Rust/merman/tools/debug/arch_fcose_browser_probe_fixture_025.js)
  so it:
  - documents that it is a manual diagnostic reconstruction rather than a full Mermaid CLI render
    replacement,
  - mirrors the xtask deterministic page prelude more closely by also patching
    `crypto.getRandomValues`, and
  - reads the same currently shipped Architecture FCoSE config fields instead of hard-coding
    same-group ideal length and elasticity.
- A rejected exploratory patch changed `manatee` from the current xorshift deterministic baseline
  to the later repo-ref `mulberry32` seed helper. It was reverted before commit because the shipped
  npm `mermaid@11.15.0` baseline does not contain that path. Do not repeat that change unless the
  baseline package changes or `dist/mermaid.js` confirms the source path.
- A refreshed probe
  `target/compare/arch_junction_fork_join_probe_hpd050_debug_tool_refresh.json` still does not
  reproduce the CLI fixture. It reports probe-minus-fixture deltas such as
  `auth.x=+12.684px`, `cache.x=+12.684px`, `api.y=-15.004px`, and `db.y=-15.004px`. It is closer
  to local output than to the fixture, but no longer exactly identical after the config/prelude
  cleanup. Treat it as diagnostic evidence only.

Focused verification:

- `cargo fmt --all`
- `cargo test -p manatee xorshift64star_next_f64_unit_matches_seeded_upstream_baseline --lib`
- `cargo test -p merman-render architecture_fcose_node_bounds_extras_feed_label_bounds --lib`
- `$env:PUPPETEER_EXECUTABLE_PATH='C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'; node tools/debug/arch_fcose_browser_probe_fixture_025.js stress_architecture_junction_fork_join_026 > target/compare/arch_junction_fork_join_probe_hpd050_debug_tool_refresh.json`
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_junction_fork_join_026 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_junction_fork_join_hpd050_debug_tool_refresh.md`
  expected failure remains `2808.127px` upstream vs `2822.102px` local (`+13.976px`).

Sixth slice source-checkout guard:

- Checked the local reference checkout before continuing source-backed Architecture work:
  - `git -C repo-ref/mermaid rev-parse HEAD` => `9bae92cd3214f9ec99369ab314ef41ffb283f6b6`
  - `git -C repo-ref/mermaid status --short --branch` => `develop...origin/develop`
  - `tools/upstreams/REPOS.lock.json` pins Mermaid to
    `41646dfd43ac83f001b03c70605feb036afae46d` (`mermaid@11.15.0`)
- The repo-ref checkout is therefore ahead of the active baseline. This explains why reading
  `repo-ref/mermaid/packages/mermaid/src/diagrams/architecture/architectureRenderer.ts` directly
  exposed a later `withSeededRandom` path that is absent from the installed
  `tools/mermaid-cli/node_modules/mermaid/dist/mermaid.js` and from the locked
  `41646dfd...` source.
- For HPD-050 source-backed claims, use one of:
  - `git -C repo-ref/mermaid show 41646dfd43ac83f001b03c70605feb036afae46d:<path>` for source,
  - `tools/mermaid-cli/node_modules/mermaid/dist/mermaid.js` for the actual baseline renderer
    bundle, or
  - fresh `check-upstream-svgs` output for rendered behavior.
- Do not use the current `repo-ref/mermaid` working tree path as baseline truth unless it has first
  been verified against `tools/upstreams/REPOS.lock.json`.

Focused verification:

- `git -C repo-ref/mermaid show --no-patch --oneline 41646dfd43ac83f001b03c70605feb036afae46d`
- `git -C repo-ref/mermaid show 41646dfd43ac83f001b03c70605feb036afae46d:packages/mermaid/src/diagrams/architecture/architectureRenderer.ts`
  confirmed locked source has `gap: 1.5 * db.getConfigField('iconSize')`, reads
  `idealEdgeLengthMultiplier`, `edgeElasticity`, `randomize`, `nodeSeparation`, and `numIter`, and
  has no `withSeededRandom` path.

Residual evidence after the fourth slice:

- Full Architecture structural `parity` remains green.
- Full Architecture `parity-root` remains an expected failure with `26` mismatches.
- `stress_architecture_batch5_long_titles_and_punct_076` remains upstream `542.926px` vs local
  `547.926px` (`+5.000px`).
- `stress_architecture_html_titles_and_escapes_041` remains upstream `479.926px` vs local
  `484.926px` (`+5.000px`).
- Override growth remains unchanged.

Seventh slice Cytoscape bbox phase split:

- Enhanced
  [tools/debug/arch_fcose_browser_probe_fixture_025.js](/F:/SourceCodes/Rust/merman/tools/debug/arch_fcose_browser_probe_fixture_025.js)
  so pre-layout node diagnostics include Cytoscape `labelWidth`, `labelHeight`, `labelBounds`,
  `bodyBounds`, `autoWidth`, `autoHeight`, and `autoPadding`.
- The refreshed diagnostic probe for
  `stress_architecture_batch6_init_fontsize_icon_size_wrap_093` reports:
  - `api` service `labelWidth=95`, `labelBounds=99x22`, `bodyBounds=42x42`,
    `node.boundingBox()=101x62`
  - `db` service `labelWidth=78`, `labelBounds=82x22`, `node.boundingBox()=84x62`
  - `left` group `autoWidth=99`, `autoHeight=61`, `outerWidth=160x122`,
    `node.boundingBox()=162x124`
- This proves the row needs separate handling for leaf default `node.boundingBox()`, child
  `updateCompoundBounds()` contribution, final group `node.boundingBox()`, and manatee relocation
  bbox approximation.
- A source-shaped exploratory production patch changed Architecture bbox math to
  `ceil(canvas)+labelBounds` and group extra `+1.5`. It was rejected before commit:
  - `batch6_init_fontsize_icon_size_wrap_093` became root-exact (`325.105x380.479`)
  - `batch4_init_small_icons_061` stayed root-exact (`187.859x191.571`)
  - full Architecture root mismatches increased from `26` to `47`
  - `batch5_long_titles_and_punct_076` worsened from `+5.000px` to `+7.500px`
  - `html_titles_and_escapes_041` worsened from `+5.000px` to `+7.500px`
- The production patch was reverted. Keep the diagnostic probe output, but do not apply a single
  global Cytoscape bbox formula until the renderer/manatee seam can represent the separate phases.

Focused verification:

- `$env:PUPPETEER_EXECUTABLE_PATH='C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'; node tools/debug/arch_fcose_browser_probe_fixture_025.js stress_architecture_batch6_init_fontsize_icon_size_wrap_093 > target/compare/arch_batch6_init_fontsize_icon_size_wrap_probe_hpd050_metrics.json`
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_batch6_init_fontsize_icon_size_wrap_093 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_batch6_init_fontsize_icon_size_wrap_hpd050_cytoscape_bbox_seam_y.md`
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_batch4_init_small_icons_061 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_batch4_small_icons_hpd050_cytoscape_bbox_seam_y.md`
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_report_parity_root_after_hpd050_cytoscape_bbox_seam.md`
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_report_parity_root_after_hpd050_probe_metrics_only.md`
- `cargo test -p merman-render architecture_text_constants_match_mermaid --lib`

Eighth slice service bounds phase-name refactor:

- Renamed Architecture service bounds estimate fields so the renderer explicitly distinguishes:
  - `emitted_icon_bounds`
  - `svg_root_bounds`
  - `cytoscape_group_child_bounds`
- This is a behavior-preserving seam cleanup after the rejected bbox formula. It keeps the current
  broad approximation while preventing future work from treating root SVG getBBox, emitted icon
  bounds, and group child bounds as one interchangeable `compound_bounds` phase.

Focused verification:

- `cargo fmt --all`
- `cargo test -p merman-render architecture_text_constants_match_mermaid --lib`
- `cargo test -p merman-render --test architecture_svg_test`
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_batch6_init_fontsize_icon_size_wrap_093 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_batch6_init_fontsize_icon_size_wrap_hpd050_phase_names_refactor.md`
  expected failure remains upstream `325.105px` vs local `322.605px` (`-2.500px`).
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target/compare/architecture_report_parity_after_hpd050_phase_names_refactor.md`
  passed.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_report_parity_root_after_hpd050_phase_names_refactor.md`
  expected failure remains `26` Architecture root mismatches.

Ninth slice Dugong / Graphlib audit:

- Cloned `repo-ref/dagre` and `repo-ref/graphlib`, then checked them out to the lockfile commits:
  - Dagre: `ba986662394f8f3ed608717194e5958f3386ce01`
  - Graphlib: `380d5efa1f4ab0904539f046bdba583d14ac2add`
- Added
  [docs/dugong/GRAPHLIB_UPSTREAM_TEST_COVERAGE.md](/F:/SourceCodes/Rust/merman/docs/dugong/GRAPHLIB_UPSTREAM_TEST_COVERAGE.md)
  so Graphlib test coverage is no longer only an implicit assumption behind Dagre coverage.
- Ported the currently exposed Graphlib helper algorithm cases from upstream:
  `components`, `findCycles`, `preorder`, and `postorder`.
- Tightened `dugong_graphlib::alg::{preorder, postorder}` so missing roots panic instead of
  silently traversing a non-existent root, matching upstream Graphlib's invalid-root throw
  behavior in Rust form.
- Fixed
  [tools/dagre-harness/run.mjs](/F:/SourceCodes/Rust/merman/tools/dagre-harness/run.mjs)
  to import `dagre-d3-es` from `tools/mermaid-cli/node_modules`, which makes the reference runner
  executable in the current repository layout.
- Updated
  [tools/dagre-harness/README.md](/F:/SourceCodes/Rust/merman/tools/dagre-harness/README.md)
  so it describes the pinned Mermaid `11.15.0` / `dagre-d3-es@7.0.14` toolchain instead of the old
  11.12-era package facts.

Focused verification:

- `cargo test -p dugong-graphlib --tests`
- `cargo test -p dugong --tests`
- `node tools/dagre-harness/run.mjs --help`
- `cargo run -p xtask -- compare-dagre-layout --fixture basic --out-dir target/compare/dagre-layout-hpd050-graphlib-audit`

Verification notes:

- `cargo run -p xtask -- compare-dagre-layout --help` still returns `Error: Usage`; that is the
  command's existing lack of help output, not a harness import failure.
- The focused `compare-dagre-layout` run for `fixtures/state/basic.mmd` completed and reported
  `max node delta: 0.000000` and `max edge delta: 0.000000`.
- This slice does not claim full Graphlib parity. The next useful audit target is the public
  `Graph` API subset consumed by `dugong` and Mermaid-facing renderers, not unused shortest-path
  algorithms.

Tenth slice Graphlib Graph core coverage:

- Added
  [crates/dugong-graphlib/tests/graph_core_test.rs](/F:/SourceCodes/Rust/merman/crates/dugong-graphlib/tests/graph_core_test.rs)
  as the first direct source-test slice from `repo-ref/graphlib/test/graph-test.js`.
- Covered current public Rust API equivalents for initial options, graph labels, node defaults,
  source queries, edge creation/update, named multiedges, path edges, parent/child moves, root
  children, and remove-node cleanup.
- Tightened `Graph::set_parent_ix(...)` so assigning a node under its own descendant panics with
  `set_parent would create a cycle`, matching upstream Graphlib's tree-invariant throw in Rust
  form.
- Updated
  [docs/dugong/GRAPHLIB_UPSTREAM_TEST_COVERAGE.md](/F:/SourceCodes/Rust/merman/docs/dugong/GRAPHLIB_UPSTREAM_TEST_COVERAGE.md)
  to classify `test/graph-test.js` as partially ported and list the mapped cases.

Focused verification:

- `cargo test -p dugong-graphlib --tests`
- `cargo test -p dugong --tests`
- `cargo test -p merman-render --test flowchart_layout_test`
- `cargo test -p merman-render --test state_layout_test`
- `cargo test -p merman-render --test class_layout_test`
- `cargo test -p merman-render --test er_layout_test`
- `cargo test -p dugong --tests`

Verification notes:

- The invalid non-compound `setParent(...)` upstream throw remains a deliberate open API-shape
  decision; current Rust methods still no-op on non-compound graphs. Do not change that casually
  without auditing downstream callers.

Eleventh slice Graphlib edge-query coverage:

- Extended the direct `repo-ref/graphlib/test/graph-test.js` coverage in
  [crates/dugong-graphlib/tests/graph_core_test.rs](/F:/SourceCodes/Rust/merman/crates/dugong-graphlib/tests/graph_core_test.rs)
  to cover `sinks`, `predecessors`, `successors`, `neighbors`, `isLeaf`, `inEdges`, `outEdges`,
  `nodeEdges`, and remove-edge adjacency updates.
- Added Rust API seams for source-backed Graphlib behavior that previously had no public equivalent:
  `Graph::sinks(...)`, `Graph::is_leaf(...)`, and `Graph::node_edges_between(...)`.
- Updated
  [docs/dugong/GRAPHLIB_UPSTREAM_TEST_COVERAGE.md](/F:/SourceCodes/Rust/merman/docs/dugong/GRAPHLIB_UPSTREAM_TEST_COVERAGE.md)
  so this slice is mapped to pinned upstream case names and the remaining JS/Rust API-shape
  differences are explicit.

Focused verification:

- `cargo test -p dugong-graphlib --tests`
- `cargo test -p dugong --tests`

Verification notes:

- Missing-node query behavior is intentionally not claimed as identical: upstream JS returns
  `undefined` for several query methods, while the current Rust collection API returns empty
  vectors.
- Upstream chainability for `removeEdge(...)` is not copied into Rust; tests cover the state and
  adjacency effects that matter to consumers.

Twelfth slice Graphlib edge invariant coverage:

- Tightened [crates/dugong-graphlib/src/graph/core.rs](/F:/SourceCodes/Rust/merman/crates/dugong-graphlib/src/graph/core.rs)
  so `set_edge_named(..., Some(name), ...)` on a non-multigraph panics instead of silently
  discarding the name.
- Edge lookup/removal views now keep the supplied name even for non-multigraphs, so
  `has_edge("a", "b", Some("name"))`, `edge(...)`, and `remove_edge(...)` no longer alias the
  unnamed simple edge.
- Added direct graph-test coverage for edge-key listing, directed vs. undirected edge lookup,
  missing edge lookup, named-edge rejection, named edge removal, and undirected remove-edge
  endpoint normalization.
- Production Mermaid-facing named-edge graph construction had already been audited as multigraph
  based, so this is an invariant fix rather than a forced renderer behavior change.

Focused verification:

- `cargo test -p dugong-graphlib --tests`
- `cargo test -p dugong --tests`
- `cargo test -p merman-render --test flowchart_layout_test`
- `cargo test -p merman-render --test state_layout_test`
- `cargo test -p merman-render --test class_layout_test`
- `cargo test -p merman-render --test er_layout_test`

Thirteenth slice Dagre reference adapter extraction:

- Extracted
  [crates/xtask/src/cmd/debug/dagre_reference.rs](/F:/SourceCodes/Rust/merman/crates/xtask/src/cmd/debug/dagre_reference.rs)
  as the Rust-side adapter for the Dagre JS reference harness.
- The adapter now owns the reference input JSON schema, Rust output snapshots, JS harness
  invocation, JS output parsing, max node/edge delta calculation, and the compound-edge endpoint
  normalization mirrored from `tools/dagre-harness/run.mjs`.
- `compare-dagre-layout` remains State-only in this slice. It now acts as a graph producer plus
  command wrapper, which keeps future Dagre-backed audits from copying the reference machinery.
- Added a unit test for compound-edge normalization so the extracted adapter is covered below the
  command-smoke level.

Focused verification:

- `cargo fmt --all`
- `cargo check -p xtask`
- `cargo test -p xtask compound_edge_normalization_moves_edges_to_non_cluster_child`
- `cargo test -p xtask`
- `node tools/dagre-harness/run.mjs --help`
- `cargo run -p xtask -- compare-dagre-layout --fixture basic --out-dir target\compare\dagre-layout-hpd050-reference-adapter`
- `cargo run -p xtask -- compare-dagre-layout --fixture stress_state_composite_with_external_edges_028 --out-dir target\compare\dagre-layout-hpd050-reference-adapter-composite`
- `cargo run -p xtask -- compare-dagre-layout --fixture stress_state_composite_with_external_edges_028 --cluster state-Big-7 --out-dir target\compare\dagre-layout-hpd050-reference-adapter-cluster`

Verification notes:

- The three focused layout comparisons all reported `max node delta: 0.000000` and
  `max edge delta: 0.000000`.
- This is an ARCH-022 seam cleanup, not a claim that the Dagre reference adapter now supports every
  diagram family. Add non-State graph producers only when a source-backed residual audit needs one.

Fourteenth slice Architecture Cytoscape label-extension seam:

- Added `ArchitectureCytoscapeServiceLabelExtension` in
  [crates/merman-render/src/architecture_metrics.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/architecture_metrics.rs)
  so FCoSE node `BoundsExtras` and SVG root/group service-bounds estimation share the same
  Cytoscape service-label half-width and compound-label bottom-extension calculation.
- The current code has since narrowed this phase name to `ArchitectureCytoscapeChildLabelBounds`.
- Kept SVG root `createText(...)` measurement separate from Cytoscape compound-child label
  measurement. This is a phase split, not a root-width tune.
- Added focused unit coverage for the shared extension and empty-title behavior.

Focused verification:

- `cargo fmt --all`
- `cargo test -p merman-render architecture_cytoscape_service_label_extension_centralizes_compound_label_phase --lib`
- `cargo test -p merman-render architecture_text_constants_match_mermaid --lib`
- `cargo test -p merman-render architecture_fcose_node_bounds_extras_feed_label_bounds --lib`
- `cargo test -p merman-render architecture_node_bbox_extras_convert_to_manatee_bounds_extras --lib`
- `cargo test -p merman-render --test architecture_svg_test`
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture_report_parity_after_hpd050_cy_label_extension.md`
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_report_parity_root_after_hpd050_cy_label_extension.md`

Verification notes:

- Architecture structural parity remained green.
- Architecture parity-root remained the expected 26 mismatches.
- The root report top rows remained the known residual front, led by `junction_fork_join_026`,
  `batch5_long_titles_and_punct_076`, `html_titles_and_escapes_041`, and
  `batch6_init_fontsize_icon_size_wrap_093`.

Fifteenth slice Architecture disconnected-islands root-bounds audit:

- Re-audited `stress_architecture_disconnected_islands_046`, a height-only residual where width is
  already aligned: upstream `823.346x768.460`, current local `823.346x775.647`.
- Source check: pinned Mermaid 11.15 `setupGraphViewbox(...)` uses browser `svg.getBBox()` plus
  padding for both size attributes and `viewBox`.
- Fresh `debug-svg-bbox` evidence shows local emitted geometry alone is too short
  (`823.346x751.460` with padding), while the final local root is too tall only after the root
  viewport code unions synthetic `content_bounds` for label extents.
- A temporary experiment that used `cytoscape_group_child_bounds` for top-level services fixed this
  single fixture, but full Architecture `parity-root` mismatches grew from `26` to `84`. The
  experiment was rejected and reverted.
- Conclusion: the next fix must be a phase-specific root label contribution model. Do not globally
  collapse top-level services from `svg_root_bounds` to Cytoscape group-child bounds.

Focused verification:

- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_disconnected_islands_046 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_disconnected_islands_current_hpd050_audit.md`
  failed as expected on the current residual.
- `cargo run -p xtask -- debug-svg-bbox --svg fixtures\upstream-svgs\architecture\stress_architecture_disconnected_islands_046.svg --padding 40`
- `cargo run -p xtask -- debug-svg-bbox --svg target\compare\architecture\stress_architecture_disconnected_islands_046.svg --padding 40`
- `Select-String` counts confirmed current full root report has `26` mismatches, while the rejected
  top-level Cytoscape experiment report has `84`.

Sixteenth slice Architecture isolated top-level service root-bounds seam:

- Added `architecture_top_level_service_root_bounds(...)` in
  [crates/merman-render/src/architecture_metrics.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/architecture_metrics.rs)
  so the top-level service root contribution decision is explicit instead of hidden in the SVG
  renderer loop.
- The rule is deliberately narrow: `cytoscape_group_child_bounds` is used only for isolated
  top-level services in diagrams that also contain groups. Connected top-level services and
  no-group singleton/iconText rows keep `svg_root_bounds`.
- This closes `stress_architecture_disconnected_islands_046` without applying the rejected global
  top-level-service switch that regressed full Architecture root mismatches from `26` to `84`.
- Full Architecture structural parity remains green. Full Architecture `parity-root` remains an
  expected failure, but its mismatch count moved from `26` to `25`.
- This is a phase-specific root contribution seam, not a browser-exact text measurement claim.
  Remaining residuals still need source/evidence-backed audits rather than constants.

Focused verification:

- `cargo fmt -p merman-render`
- `cargo test -p merman-render architecture_top_level_service_root_bounds_splits_isolated_group_component_phase --lib`
- `cargo fmt --check -p merman-render`
- `cargo test -p merman-render --test architecture_svg_test`
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture_report_parity_after_hpd050_isolated_root_bounds.md`
  passed.
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_disconnected_islands_046 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_disconnected_islands_isolated_service_experiment.md`
  passed with upstream/local root `823.346x768.460`.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_report_parity_root_after_hpd050_isolated_root_bounds.md`
  remains an expected failure with `25` dom mismatches.

Architecture residual classification refresh (2026-06-03):

- Re-ran the Architecture family reports after the isolated-service seam and after subsequent
  renderability work. Structural `parity` remains green.
- The fresh Architecture `parity-root` report remains an expected failure with `25` dom
  mismatches, confirming that the older `29`-row M15RV queue is stale.
- Rows no longer in the active Architecture root queue include
  `stress_architecture_batch4_init_small_icons_061`,
  `stress_architecture_batch4_init_fontsize_wrap_063`,
  `stress_architecture_edge_label_corner_cases_012`, `stress_architecture_fan_in_out_021`,
  `stress_architecture_deep_nesting_013`,
  `stress_architecture_batch6_junctions_multi_split_with_group_edges_087`, and
  `stress_architecture_disconnected_islands_046`.
- The remaining larger audit front is source/input matched but not root-green:
  `stress_architecture_junction_fork_join_026` (`+13.976px`),
  `stress_architecture_batch5_long_titles_and_punct_076` (`+5px`),
  `stress_architecture_html_titles_and_escapes_041` (`+5px`),
  `stress_architecture_unicode_and_xml_escapes_019` (`+3px`),
  `stress_architecture_batch6_init_fontsize_icon_size_wrap_093` (`-2.5px`),
  `stress_architecture_nested_groups_002` (`+2.5px`), and
  `stress_architecture_group_port_edges_017` (`+1.468px`).
- Treat the smaller icon/default/reasonable-height rows as browser/Cytoscape bbox lattice tails
  unless a source rule or generated measurement path explains the whole class. Do not add root
  pins, one-off metric constants, or broad solver rewrites to make the count smaller.

Architecture group bbox phase audit (2026-06-03):

- Re-audited `stress_architecture_batch5_long_titles_and_punct_076` and
  `stress_architecture_html_titles_and_escapes_041` on current HEAD.
- The focused reports remain expected failures:
  - `target/compare/architecture_batch5_hpd050_current_debug.md`: upstream `542.926px`, local
    `547.926px`.
  - `target/compare/architecture_html_titles_hpd050_current_debug.md`: upstream `479.926px`, local
    `484.926px`.
- Structured SVG inspection confirms the deltas are final group rect width:
  - `batch5_long_titles`: upstream group rect width `462.925633px`, local `467.925633px`.
  - `html_titles`: upstream group rect width `399.925633px`, local `404.925633px`.
- A temporary experiment changing `ARCHITECTURE_SVG_GROUP_BBOX_EXTRA_PADDING_PX` from `2.5` to
  `0.0` made those two rows width-exact but made their heights `5px` too short and expanded many
  group-heavy root mismatches. The experiment report is
  `target/compare/architecture_report_parity_root_experiment_group_extra_0.md`; the code change was
  reverted before commit.
- Conclusion: do not globally remove the final SVG group bbox extra. The right follow-up is a
  phase-specific Cytoscape bbox model, not a root pin, one-off width constant, or single global
  group-padding formula.

Architecture group bbox source-formula follow-up (2026-06-03):

- Re-audited the same two `+5px` rows with browser `finalElements`, local group-rect debug, and
  Cytoscape source:
  - `stress_architecture_batch5_long_titles_and_punct_076`
  - `stress_architecture_html_titles_and_escapes_041`
- Cytoscape `updateCompoundBounds()` sizes a parent from
  `children.boundingBox({ includeLabels, includeOverlays: false, useCache: false })`; parent
  `width()` / `height()` then expose `_p.autoWidth` / `_p.autoHeight`, `padding()` exposes
  `_p.autoPadding`, and `outerWidth()` / `outerHeight()` add border plus `2 * padding()`. The final
  default `node.boundingBox()` body path also applies the browser inaccuracy / anti-alias expansion.
- Browser `finalElements` metrics show:
  - `batch5` group `pipeline`: `autoWidth=379.926`, `outerWidth=460.926`,
    `node.boundingBox().w=462.926`.
  - `html_titles` group `ui`: `autoWidth=316.926`, `outerWidth=397.926`,
    `node.boundingBox().w=399.926`.
- Local group debug shows:
  - `batch5` group `pipeline`: content width `382.926`, final width `467.926`.
  - `html_titles` group `ui`: content width `319.926`, final width `404.926`.
- The `+5px` rows therefore split into a `+3px` child-contribution mismatch and a `+2px` final
  group formula mismatch. This is useful, but not safe to patch piecemeal.
- Two temporary experiments were rejected and reverted:
  - Split-axis group padding (`x=padding`, `y=padding+2.5`) made both focused rows root-green but
    reopened many group-heavy Architecture rows as too narrow.
  - Standalone `ARCHITECTURE_SVG_GROUP_BBOX_EXTRA_PADDING_PX=1.5` improved the two focused rows
    from `+5px` to `+3px` but still reopened many rows.
- Conclusion: keep `ARCHITECTURE_SVG_GROUP_BBOX_EXTRA_PADDING_PX=2.5` for now. The source-backed
  next implementation is a proper model of Cytoscape child contribution into
  `children.boundingBox(...)`, followed by the final group `outerWidth + body expansion` formula.

Architecture children bbox probe follow-up (2026-06-03):

- Enhanced `tools/debug/arch_fcose_browser_probe_fixture_025.js` again so parent nodes emit:
  - `childrenBoundingBoxIncludeLabels`
  - `childrenBoundingBoxBodyOnly`
- This directly confirms the source formula instead of inferring it from parent dimensions:
  - `batch5` group `pipeline`: `childrenBoundingBoxIncludeLabels.w=379.926`,
    `autoWidth=379.926`, `childrenBoundingBoxBodyOnly.w=282.926`,
    `node.boundingBox().w=462.926`.
  - `html_titles` group `ui`: `childrenBoundingBoxIncludeLabels.w=316.926`,
    `autoWidth=316.926`, `childrenBoundingBoxBodyOnly.w=282.926`,
    `node.boundingBox().w=399.926`.
- Browser leaf service `labelBounds.w` follows the Cytoscape source rule `labelWidth + 4`:
  `Runner Linux amd64=153`, `Container Registry=137`,
  `Artifacts Storage retention 30d=221`, `Production=81`,
  `Web Front Line 2=127`, `CDN Cache=90`, `Origin primary=105`.
- Current Rust child-label contributions for the same labels are not a uniform offset:
  `154`, `139`, `225`, `81`, `129`, `82`, and `109` respectively. This rejects a
  uniform subtract-N fix and also explains why a padding-only correction overfits the two focused
  rows.
- Conclusion: the next real implementation should be a phase-specific helper for Cytoscape service
  child label/body contribution. Do not change the shared canvas label scale or final group padding
  unless the helper survives the full Architecture root suite.

Architecture child source-phase experiments (2026-06-03):

- Current HEAD baseline for Architecture `parity-root` is still `25` DOM mismatches.
- Experiment A used the Cytoscape labelBounds source formula
  `ceil(headless_measured_width) / 2 + 2` only for `cytoscape_group_child_bounds`, while leaving the
  old body and final group padding compensation in place:
  - focused `batch5_long_titles`: `+5.000` improved to `+4.500`;
  - focused `html_titles`: `+5.000` improved to `+3.500`;
  - full Architecture `parity-root` DOM mismatches improved from `25` to `24`.
- Experiment A was rejected because it is a half-source model and worsened several already-small
  residuals, e.g. `batch3_long_group_titles_wrapping_055` from `-1.000` to `-2.500` and
  `long_labels_006` from `-0.500` to `-1.500`.
- Experiment B used the fuller source phase model: child body `+1px`, child labelBounds
  `ceil(headless_measured_width) / 2 + 2`, and final group padding `padding + 1.5px`:
  - focused `batch5_long_titles`: `+5.000` improved to `+2.500`;
  - focused `html_titles`: `+5.000` improved to `+1.500`;
  - Architecture structural `parity` stayed green;
  - full Architecture `parity-root` DOM mismatches expanded from `25` to `100`.
- Experiment B was rejected and reverted because it made many group-heavy/nested rows too small,
  including `deep_group_chain_027=-7.000`, `batch6_deep_group_chain_crosslinks_094=-6.000`, and
  `batch6_nested_groups_group_edges_and_ports_086=-5.000`.
- Conclusion: `parity-root` should stay a diagnostic/regression sensor here, not a mandate to
  force every browser-derived pixel tail closed. The next real implementation needs a broader
  headless measurement model before raw Cytoscape source phases can replace the current mixed
  compensation.

Architecture final bbox probe enhancement (2026-06-03):

- Enhanced `tools/debug/arch_fcose_browser_probe_fixture_025.js` to emit `finalElements` after the
  second FCoSE run while preserving the existing `final` service-position summary.
- `finalElements` uses the same node/edge dump shape as `preLayout`, including final
  `node.boundingBox()`, `labelBounds`, `bodyBounds`, classes, data, and metrics.
- Verified the new output against two active residual fixtures:
  - `target/compare/arch_unicode_xml_probe_hpd050_final_elements.json`: group `i`
    `node.boundingBox().x1=-209.91096759368116,w=389.8219351873623`; adding `iconSize/2` matches
    the pinned upstream group rect `x=-169.91096759368116,w=389.8219351873623`.
  - `target/compare/arch_batch5_long_titles_probe_hpd050_final_elements.json`: group `pipeline`
    `node.boundingBox().x1=-273.4628163140578,w=462.92563262811564`; adding `iconSize/2` matches
    the pinned upstream group rect `x=-233.4628163140578,w=462.92563262811564`.
- Use this probe output for future Architecture bbox-phase audits before changing renderer or
  manatee math.

Architecture junction finalElements audit (2026-06-03):

- Re-audited `stress_architecture_junction_fork_join_026`, still the largest active Architecture
  root residual, with current `finalElements`, Edge-backed upstream regeneration, and Rust-side
  FCoSE constraint / edge-length debug.
- Current focused compare remains expected-fail at upstream `2808.127x2557.535` vs local
  `2822.102x2545.033`; root width is wider by `13.976px` and height is shorter by `12.502px`.
- Fresh Edge-backed `check-upstream-svgs` reproduced the stored upstream SVG, so this is not stale
  fixture drift.
- Browser probe config matches Mermaid 11.15 Architecture defaults for this row:
  `iconSize=80`, `fontSize=16`, `padding=40`, `randomize=false`, `nodeSeparation=75`,
  `idealEdgeLengthMultiplier=1.5`, `edgeElasticity=0.45`, and `numIter=2500`.
- Browser and Rust constraints match pinned `architectureRenderer.ts`: junction parents come only
  from `junction.in`, the fixture's junctions are unparented, horizontal and vertical alignment
  arrays include the duplicated `join` entries, and relative-placement output keeps duplicate
  `join -> db` / `join -> cache` rows.
- Rust edge debug confirms the source-backed FCoSE callback inputs: one same-parent edge uses base
  ideal length `120` with elasticity `0.45`, while the remaining cross-group edges use base ideal
  length `40` with elasticity `0.001`. Manatee then performs its internal intergraph ideal-length
  phase; auditing that phase requires a source-backed `cytoscape-fcose` / `cose-base` reference
  harness, not a fixture constant.
- Classification stays source-input-matched manatee-vs-Cytoscape FCoSE solution/internal phase
  residual with a manual-probe / CLI-harness split. Do not tune junction parents, duplicate
  relative constraints, group rect translation, edge path emission, root finalization, or text/group
  constants for this row alone.

Architecture group-port finalElements audit (2026-06-03):

- Re-audited `stress_architecture_group_port_edges_017` with the enhanced finalElements probe.
- Current focused compare remains expected-fail at upstream `707.769x542.448` vs local
  `709.238x524.603`.
- Browser final service positions are `in1/out1.x=-6.611`, `in2.x=193.385`, `ext.x=-270.385`,
  top row `y=-121.724`, bottom row `y=117.224`.
- Local SVG positions are `in1/out1.x=-5.907`, `in2.x=194.119`, `ext.x=-271.119`, top row
  `y=-112.801`, bottom row `y=108.301`.
- The local X spread is wider by about `1.468px`, and local Y spacing is compressed by about
  `17.845px`, matching the root width/height deltas. Probe bboxes confirm ordinary icon-floor
  service nodes (`82x100`) and source-side group bboxes.
- Classification stays source-input-matched manatee-vs-Cytoscape FCoSE solution / compound-bound
  drift. Do not tune group-edge shifts, SVG path emission, service label measurement, or root
  viewBox logic for this row alone.

Architecture custom-init finalElements audit (2026-06-03):

- Re-audited `stress_architecture_batch6_init_fontsize_icon_size_wrap_093` with finalElements.
- Current focused compare remains expected-fail at upstream `325.105x380.479` vs local
  `322.605x380.479`; height is exact and only root width is short by `2.5px`.
- Browser finalElements reports effective config `iconSize=40`, `fontSize=18`, `padding=30`.
- Browser final group bboxes are:
  - `left` `node.boundingBox().w=162,h=124`
  - `right` `node.boundingBox().w=236.605,h=160.924`
- Pinned upstream SVG group rects match those final bboxes after Mermaid's SVG group rect
  translation. Local SVG group rects are `left.w=159,h=124` and
  `right.w=235.605,h=160.924`.
- Browser final service bboxes are `api.w=101,labelBounds.w=99`,
  `db.w=84,labelBounds.w=82`, and `disk.w=42,labelBounds.w=39`.
- Classification stays custom-init Cytoscape service/group child bbox phase residual. The earlier
  exploratory global formula that made this row exact expanded full Architecture root mismatches
  from `26` to `47`, so do not change global group padding, add a single service label scale, or
  pin root bounds for this row. A valid fix needs a reusable phase-specific bbox model.

Architecture nested-groups finalElements audit (2026-06-03):

- Re-audited `stress_architecture_nested_groups_002` with finalElements and local group debug.
- Pinned Mermaid source confirms the relevant source rules: Cytoscape `.node-group` padding comes
  from `db.getConfigField('padding')`, and SVG group rects render from final
  `node.boundingBox()` with `x = x1 + iconSize / 2`, `y = y1 + iconSize / 2`, `width = w`, and
  `height = h`.
- Current focused compare remains expected-fail at upstream `727.924x622.658` vs local
  `730.424x622.658`; height is exact and root width is wider by `2.5px`.
- Browser final group bboxes are `platform.w=459.154,h=542.658`,
  `runtime.w=365.654,h=182`, and `data.w=376.154,h=182`. Pinned upstream SVG group rects match
  those bboxes after Mermaid's `iconSize / 2` rect translation.
- Local SVG group rects are `platform.w=458.654,h=542.658`,
  `runtime.w=365.654,h=182`, and `data.w=375.654,h=182`.
- Browser final service positions and local service positions have matching Y values, while local
  X values are uniformly about `+1.25px`. Local group debug confirms the existing configured
  `pad=42.5` path and nested child-group inset propagation.
- Classification stays nested compound-bounds phase residual. Do not change SVG group-rect
  translation, configured padding, root finalization, or edge path emission for this row alone.

Architecture unicode-label finalElements audit (2026-06-03):

- Re-audited `stress_architecture_unicode_and_xml_escapes_019` with finalElements, local group
  debug, and focused text measurements.
- Current focused compare remains expected-fail at upstream `469.822x463.593` vs local
  `472.822x463.593`; height is exact and root width is wider by `3px`.
- Browser final group `i` bbox is `node.boundingBox().w=389.822,h=383.593`, and the pinned
  upstream SVG group rect matches it after Mermaid's `iconSize / 2` rect translation. Local group
  rect is `w=392.822,h=383.593`.
- Browser final service bboxes are `metrics.w=123,labelBounds.w=121`,
  `logs.w=101,labelBounds.w=99`, `store.w=93,labelBounds.w=91`, and
  `alert.w=97,labelBounds.w=95`.
- Local SVG service positions are all about `-1.5px` on X compared with the browser probe, while Y
  values match. Local group debug confirms configured `pad=42.5` and final width `392.822`.
- Focused vendored text widths (`Metrics Exporter=118.055`, `Log Collector=94.945`,
  `Store Query=84.773`, `Alert Service=91.961`) do not map to browser `labelBounds` or local
  group-child bounds by one stable offset or scale.
- The stored upstream and local SVGs emit the same decoded label words. This row is not an XML
  entity or label-decode issue; classification stays service label / group child bbox phase
  residual.

Focused verification:

- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target/compare/architecture_report_parity_hpd050_residual_classification_refresh.md`
  passed.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_report_parity_root_hpd050_residual_classification_refresh.md`
  remained an expected failure with `25` dom mismatches.

Seventeenth slice Graphlib filter/default-label API coverage:

- Added source-backed `Graph::filter_nodes(...)` in
  [crates/dugong-graphlib/src/graph/core.rs](/F:/SourceCodes/Rust/merman/crates/dugong-graphlib/src/graph/core.rs).
  The method copies selected nodes, graph label, options, edge labels whose endpoints remain in the
  filtered graph, and Graphlib's compound parent promotion behavior when an intermediate parent is
  filtered out.
- Deepened Graphlib default-label parity without adding a global graph type constraint:
  `set_default_node_label(...)` and `set_default_edge_label(...)` keep the existing no-arg Rust API,
  while `set_default_node_label_with_id(...)` and
  `set_default_edge_label_with_endpoints(...)` expose Graphlib's source-backed callback arguments.
- `filter_nodes(...)` has method-level `Clone` bounds only for the copied graph labels. Ordinary
  Dagre graph construction and layout mutation remain unconstrained.
- Updated
  [docs/dugong/GRAPHLIB_UPSTREAM_TEST_COVERAGE.md](/F:/SourceCodes/Rust/merman/docs/dugong/GRAPHLIB_UPSTREAM_TEST_COVERAGE.md)
  to map the newly ported `repo-ref/graphlib/test/graph-test.js` cases.
- This is public Graph API parity work, not a renderer tune and not a move toward unused Graphlib
  shortest-path algorithms.

Focused verification:

- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo test -p dugong-graphlib --test graph_core_test`
  passed with `52` tests.
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib`
  passed with `78` tests.
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong`
  passed with `267` tests.

Eighteenth slice Graphlib children/root API coverage:

- Added `Graph::children_opt(...)` in
  [crates/dugong-graphlib/src/graph/core.rs](/F:/SourceCodes/Rust/merman/crates/dugong-graphlib/src/graph/core.rs)
  as a narrow optional-return seam for Graphlib's `children(v)` shape. It returns `None` for a
  missing queried node, `Some([])` for an existing node with no children, and direct children for
  compound nodes.
- Kept the existing ergonomic `children(parent) -> Vec<&str>` behavior unchanged so current Rust
  callers that expect empty vectors do not regress.
- Reused the existing `children_root()` API as the Rust mapping for Graphlib's no-argument
  `children()` root query. For non-compound graphs it returns all nodes; for compound graphs it
  returns nodes without a parent.
- Updated the Graphlib coverage ledger so pinned `repo-ref/graphlib/test/graph-test.js`
  `children` cases map to concrete Rust tests instead of remaining implicit under parent tests.
- This is a public Graph API shape seam. It does not force JS overloads, `undefined`, chainability,
  ID stringification, or non-compound `setParent(...)` throws into the existing Rust APIs.

Focused verification:

- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib children_opt`
  failed first because `Graph::children_opt(...)` did not exist.
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib children_opt`
  passed after the seam was implemented.
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib children`
  passed with `3` tests.
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib`
  passed with `80` tests.
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong`
  passed with `267` tests.

Nineteenth slice Graphlib setPath label API coverage:

- Added `Graph::set_path_with_label(...)` in
  [crates/dugong-graphlib/src/graph/core.rs](/F:/SourceCodes/Rust/merman/crates/dugong-graphlib/src/graph/core.rs)
  as the Rust mapping for Graphlib's `setPath(nodes, value)` behavior.
- The method sets the same label on every edge in the path and updates existing edge labels,
  matching pinned `repo-ref/graphlib/lib/graph.js` `setPath(...)`.
- The `Clone` bound is method-scoped to this batch-label API. Ordinary graph construction and
  layout mutation still do not require cloneable edge labels.
- Updated the Graphlib coverage ledger so `setPath / can set a value for all of the edges` maps to
  a concrete Rust regression.

Focused verification:

- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib set_path_with_label`
  failed first because `Graph::set_path_with_label(...)` did not exist.
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib set_path_with_label`
  passed after the seam was implemented.
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib`
  passed with `81` tests.
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong`
  passed with `267` tests.

Twentieth slice Graphlib setNodes API coverage:

- Added `Graph::set_nodes(...)` and `Graph::set_nodes_with_label(...)` in
  [crates/dugong-graphlib/src/graph/core.rs](/F:/SourceCodes/Rust/merman/crates/dugong-graphlib/src/graph/core.rs)
  as the Rust mapping for Graphlib's `setNodes(nodes)` and `setNodes(nodes, value)` behavior.
- `set_nodes(...)` reuses the existing default node label seam and preserves existing node labels,
  matching `setNode(v)` no-value behavior in pinned `repo-ref/graphlib/lib/graph.js`.
- `set_nodes_with_label(...)` sets and updates one label across every listed node with only a
  method-scoped `N: Clone` bound.
- Updated the Graphlib coverage ledger so the source `setNodes` cases map to concrete Rust
  regressions. This is a small graph-construction seam, not JS argument overloading or chainable API
  porting.

Focused verification:

- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib set_nodes`
  failed first because `Graph::set_nodes(...)` and `Graph::set_nodes_with_label(...)` did not
  exist.
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib set_nodes`
  passed after the seam was implemented.
- `cargo fmt --check -p dugong-graphlib`
  passed.
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib`
  passed with `83` tests.
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong`
  passed with `267` tests.

Twenty-first slice Graphlib parent/clear-parent coverage:

- Added direct regression coverage in
  [crates/dugong-graphlib/tests/graph_core_test.rs](/F:/SourceCodes/Rust/merman/crates/dugong-graphlib/tests/graph_core_test.rs)
  for Graphlib's `parent(v)` optional query shape: non-compound graphs, missing nodes, unassigned
  compound nodes, and assigned compound parents.
- Extended `clear_parent_returns_node_to_root_children` to cover idempotent parent removal, mapping
  Graphlib's `setParent(v)` / `setParent(v, undefined)` clear-parent state behavior onto Rust's
  explicit `clear_parent(v)` API.
- Updated the Graphlib coverage ledger so source `setParent` parent-removal and `parent` query cases
  are no longer implicit assumptions behind Dugong/renderer usage.
- No production code changed in this slice. Current Rust APIs already matched the state behavior,
  while JS chainability, optional-argument overloading, and ID stringification remain explicit shape
  differences.

Focused verification:

- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib parent`
  passed with `9` tests.
- `cargo fmt --check -p dugong-graphlib`
  passed.
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib`
  passed with `84` tests.
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong`
  passed with `267` tests.

Twenty-second slice Graphlib setEdge optional-label / EdgeKey coverage:

- Added direct regressions in
  [crates/dugong-graphlib/tests/graph_core_test.rs](/F:/SourceCodes/Rust/merman/crates/dugong-graphlib/tests/graph_core_test.rs)
  for Graphlib's `setEdge(..., undefined)` state behavior. Rust maps this through explicit
  `Option<T>` edge labels: `Some(None)` clears an existing optional label while keeping the edge
  present.
- Added `set_edge_key_sets_simple_and_named_edge_labels` so Graphlib's edge-object and multi-edge
  object `setEdge({ v, w, name }, value)` cases map to the Rust `EdgeKey` API.
- Updated the Graphlib coverage ledger so these source `setEdge` cases are no longer implicit.
- No production code changed in this slice. JS stringification, chainability, and argument
  overloading remain explicit Rust/JS API-shape differences.

Focused verification:

- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib set_edge`
  failed first on an unnamed `EdgeKey::new(..., None)` type-inference issue in the new test.
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib set_edge`
  passed after spelling the unnamed edge object as `None::<String>`.

Full verification:

- `cargo fmt --check -p dugong-graphlib` passed.
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib`
  passed with `87` tests.
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong`
  passed with `267` tests.
- JSONL validation passed for `CONTEXT.jsonl`, `TASKS.jsonl`, and `CAMPAIGNS.jsonl`.
- `git diff --check` passed.

Twenty-third slice Graphlib JSON seam coverage:

- Added
  [crates/dugong-graphlib/src/json.rs](/F:/SourceCodes/Rust/merman/crates/dugong-graphlib/src/json.rs)
  and exposed `dugong_graphlib::json::{write, read}` as a public Graphlib-shaped seam mirroring
  upstream `repo-ref/graphlib/lib/json.js` options/value/nodes/edges structure. The primary seam
  operates on `Graph<Option<N>, Option<E>, Option<G>>`, mapping upstream `undefined` to `None` and
  preserving explicit JSON `null` as `Some(null)`.
- Added direct regressions in
  [crates/dugong-graphlib/tests/json_test.rs](/F:/SourceCodes/Rust/merman/crates/dugong-graphlib/tests/json_test.rs)
  for all six upstream `repo-ref/graphlib/test/json-test.js` cases: graph options, graph value,
  nodes, simple edges, multiedges, and compound parent/child relationships.
- This resolves the old "Graphlib JSON remains undecided" warning from the earlier HPD-050 journal.
  Future Graphlib-shaped serializers should reuse this seam instead of inventing another ad hoc
  format.
- Added a focused regression so explicit `null` graph/node/edge labels are written as present
  `value: null` fields, while `None` labels are omitted like upstream `undefined`.
- Kept Rust default-label collapsing behind explicit `write_with_defaults` / `read_with_defaults`
  helpers so downstream callers can opt into the weaker bridge without weakening the main
  source-backed seam.
- No renderer or layout logic changed. Structural parity risk is limited to compile/test surface,
  and Dugong plus Dugong-Graphlib suites remained green.

Focused verification:

- `cargo nextest run -p dugong-graphlib --test json_test`
  passed with `8` tests.

Full verification:

- `cargo nextest run -p dugong-graphlib --tests`
  passed with `95` tests.
- `cargo nextest run -p dugong --tests`
  passed with `267` tests.
- `cargo fmt --check --package dugong-graphlib`
  passed.
- JSONL validation passed for `CONTEXT.jsonl` (`407` lines), `TASKS.jsonl` (`8` lines), and
  `CAMPAIGNS.jsonl` (`4` lines).
- `git diff --check`
  passed with only the existing LF/CRLF working-copy warnings for `CONTEXT.jsonl` and
  `TASKS.jsonl`.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`
  passed. Implemented-matrix structural parity stayed green after the container-only Graphlib JSON
  seam landed.

Twenty-fourth slice Dagre reference Graphlib JSON consumer seam:

- Reused the new `dugong_graphlib::json` seam in the real Dagre reference adapter instead of
  leaving xtask with another Graphlib-shaped serializer.
- [crates/xtask/src/cmd/debug/dagre_reference.rs](/F:/SourceCodes/Rust/merman/crates/xtask/src/cmd/debug/dagre_reference.rs)
  now projects Dagre labels into `Graph<Option<JsonValue>, Option<JsonValue>, Option<JsonValue>>`
  and serializes reference input / Rust output through Graphlib JSON `options`, top-level `value`,
  node `v` / `value`, edge `v` / `w` / `name` / `value`, and optional `parent`.
- [tools/dagre-harness/run.mjs](/F:/SourceCodes/Rust/merman/tools/dagre-harness/run.mjs)
  accepts both the older `graph` / `id` / `label` debug input and the new Graphlib JSON shape, then
  writes JS output through the installed `dagre-d3-es` Graphlib `json.write(...)` helper.
- The comparison reader remains backward compatible with older debug artifacts while preferring
  Graphlib JSON `value` labels when present.
- This is a consumer-relevance cleanup for HPD-050: it removes an ad hoc debug JSON shape from the
  active Dagre reference path without changing renderer or solver behavior.

Focused verification:

- `cargo nextest run -p xtask dagre_reference_input_uses_graphlib_json_shape`
  first failed against the old `graph` / `id` / `label` shape, then passed after the adapter moved
  to Graphlib JSON.
- `cargo nextest run -p xtask dagre_reference`
  passed with `3` tests, covering compound-edge normalization plus Graphlib JSON input and Rust
  output artifact shapes.
- `cargo run -p xtask -- compare-dagre-layout --diagram state --fixture basic --out-dir target\compare\dagre-layout-hpd050-graphlib-json`
  passed with max node delta `0.000000` and max edge delta `0.000000`.
- `cargo nextest run -p dugong-graphlib --test json_test`
  passed with `8` tests.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`
  passed on the post-merge worktree; implemented-matrix structural parity remains green.

## HPD-080 - Visible Rendering Defect Triage

First slice outcome:

- Promoted visible rendering defects above fine numeric root residuals in the workstream policy.
  DOM structural parity can be green while an SVG is still functionally broken if text is invisible,
  branch labels become dark blocks, diagram cards lose theme rules, or semantic color cues are not
  emitted.
- Audited pinned Mermaid `11.15.0` diagram style providers at commit
  `41646dfd43ac83f001b03c70605feb036afae46d`:
  - `packages/mermaid/src/diagrams/kanban/styles.ts`
  - `packages/mermaid/src/diagrams/packet/styles.ts`
  - `packages/mermaid/src/diagrams/sankey/styles.js`
  - `packages/mermaid/src/diagrams/c4/styles.js`
  - `packages/mermaid/src/diagrams/git/styles.js`
- Kanban now emits Mermaid 11.15 section/ticket/icon/label theme CSS. The user-provided metadata
  example renders readable cards and labels.
- Packet now maps Mermaid 11.15 `packet.*` style options into emitted CSS rather than hardcoding
  defaults.
- Sankey now emits config-aware info CSS plus source-backed label, node-label, outlined-label, and
  link rules.
- C4 now emits config-aware base CSS and source-backed `.person` theme colors.
- GitGraph now emits Mermaid 11.15 classic/default per-branch theme rules, including
  `.branch-labelN`, `.commitN`, `.commit-highlightN`, `.labelN`, `.arrowN`, and merge/reverse/
  highlight-inner colors. The user-provided three-branch merge graph now has readable branch labels
  and visible colored branch/merge paths.

Touched production surfaces:

- [crates/merman-render/src/svg/parity/packet.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/packet.rs)
- [crates/merman-render/src/svg/parity/css.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/css.rs)
- [crates/merman-render/src/svg/parity/sankey.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/sankey.rs)
- [crates/merman-render/src/svg/parity/c4.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/c4.rs)
- [crates/merman-render/src/svg/parity/gitgraph.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/gitgraph.rs)

Focused verification:

- `cargo test -p merman-render kanban`
- `cargo test -p merman-render packet_css_honors_mermaid_11_15_packet_style_options`
- `cargo test -p merman-render sankey_css_honors_mermaid_11_15_theme_options`
- `cargo test -p merman-render c4_css_honors_mermaid_11_15_person_theme_options`
- `cargo test -p merman-render gitgraph_css_includes_mermaid_11_15_branch_theme_rules`
- `cargo run -p xtask -- compare-kanban-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo run -p xtask -- compare-packet-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo run -p xtask -- compare-sankey-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo run -p xtask -- compare-c4-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo run -p xtask -- compare-gitgraph-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo fmt --check -p merman-render`
- `git diff --check`

Manual render evidence:

- `target/compare/kanban_user_metadata.fixed.png`
- `target/compare/gitgraph_user_merge.png`

Negative / residual evidence:

- A broad-filter `cargo test -p merman-render mermaid_11_15` run also matched the pre-existing
  Sequence root-width residual test
  `sequence_long_leftof_notes_keep_mermaid_11_15_root_width`, which still fails for the documented
  note/root measurement tail. The new HPD-080 CSS tests passed; this slice does not claim Sequence
  root closure.

Second slice outcome:

- Audited additional pinned Mermaid 11.15 style providers with the HPD-080 visible-rendering lens:
  - `packages/mermaid/src/diagrams/gantt/styles.js`
  - `packages/mermaid/src/diagrams/treemap/styles.ts`
  - `packages/mermaid/src/diagrams/requirement/styles.js`
- Gantt previously discarded `effective_config` when emitting CSS. It now reads source-backed Gantt
  theme variables for section backgrounds, grid/today colors, task text, task bars, active/done/
  critical states, vertical markers, and title text.
- Gantt now emits Mermaid 11.15 outside done/doneCrit text contrast rules, preventing labels that
  move outside bars from inheriting the wrong bar-text color.
- Treemap now maps Mermaid 11.15 `treemap.*` style options and theme title/text colors for section,
  leaf, label, value, and title CSS.
- Requirement now maps Mermaid 11.15 requirement theme variables for requirement boxes, labels,
  relationship lines/labels, edge-label backgrounds, node text, and divider colors.
- Requirement intentionally did not emit the upstream `[data-look][data-color-id]` color scale
  rules because current local Requirement SVGs do not emit those attributes; inert CSS would not
  improve renderability.

Touched production surfaces:

- [crates/merman-render/src/svg/parity/css.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/css.rs)
- [crates/merman-render/src/svg/parity/gantt.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/gantt.rs)
- [crates/merman-render/src/svg/parity/treemap.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/treemap.rs)

Focused verification:

- `cargo test -p merman-render css_honors_mermaid_11_15`
- `cargo run -p xtask -- compare-gantt-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo run -p xtask -- compare-treemap-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo run -p xtask -- compare-requirement-svgs --check-dom --dom-mode parity --dom-decimals 3`

Third slice outcome:

- Audited pinned Mermaid 11.15 Mindmap style provider:
  - `packages/mermaid/src/diagrams/mindmap/styles.ts`
- Mindmap previously emitted the default section palette as fixed CSS and only colored
  `.section-2 span`, while the local renderer actually emits label text through
  `<span class="nodeLabel">...`. Custom `themeVariables.cScaleLabel*` values therefore did not
  apply to most rendered labels.
- Mindmap now reads Mermaid 11.15 `THEME_COLOR_LIMIT`, `cScale*`, `cScaleLabel*`, `cScaleInv*`,
  `git0`, `gitBranchLabel0`, `nodeBorder`, `theme`, and `look` when emitting section/root CSS.
- Section `span` labels and node icons now follow source-backed `cScaleLabel*` rules. Root text and
  root `span` labels now follow `gitBranchLabel0`, or `nodeBorder` for redux-style themes, matching
  the upstream visible color rule.
- `look: neo` now uses Mermaid 11.15's source-backed Mindmap edge-depth stroke width formula.
- The upstream `[data-look="neo"]` gradient/drop-shadow rules were intentionally not emitted in
  this slice because current local Mindmap SVG nodes do not emit the required `data-look`
  attributes. Adding those inert rules would not improve renderability and would create a false
  parity signal.

Touched production surfaces:

- [crates/merman-render/src/svg/parity/mindmap.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/mindmap.rs)

Focused verification:

- `cargo test -p merman-render mindmap_css_honors_mermaid_11_15_theme_sections`
- `cargo run -p xtask -- compare-mindmap-svgs --check-dom --dom-mode parity --dom-decimals 3`

Fourth slice outcome:

- Audited pinned Mermaid 11.15 Pie style provider:
  - `packages/mermaid/src/diagrams/pie/pieStyles.ts`
- Pie previously emitted fixed default CSS through `pie_css(diagram_id)` and discarded
  `effective_config`, so Mermaid 11.15 pie style variables could not affect slice stroke/opacity,
  outer ring stroke, title text, slice labels, or legend text. This is a visible theme/readability
  defect, especially for dark or custom themes.
- Pie CSS now reads Mermaid 11.15 `themeVariables` for `pieStrokeColor`, `pieStrokeWidth`,
  `pieOpacity`, `pieOuterStrokeColor`, `pieOuterStrokeWidth`, `pieTitleTextSize`,
  `pieTitleTextColor`, `pieSectionTextSize`, `pieSectionTextColor`, `pieLegendTextSize`, and
  `pieLegendTextColor`, while preserving the default-theme fallback values from the upstream theme.
- `pie_css` now accepts `effective_config`, uses config-aware base CSS parts, and keeps `:root`
  last like the other HPD-080 style emitters.
- Removed the now-unused fixed `info_css(...)` wrapper; remaining emitters use either
  `info_css_into(...)` directly or config-aware CSS parts.

Touched production surfaces:

- [crates/merman-render/src/svg/parity/css.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/css.rs)
- [crates/merman-render/src/svg/parity/pie.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/pie.rs)

Focused verification:

- `cargo test -p merman-render pie_css_honors_mermaid_11_15_theme_options`
- `cargo run -p xtask -- compare-pie-svgs --check-dom --dom-mode parity --dom-decimals 3`

Fifth slice outcome:

- Audited pinned Mermaid 11.15 Journey style provider:
  - `packages/mermaid/src/diagrams/user-journey/styles.js`
- Journey previously mixed layout-derived presentation attributes with fixed CSS fallback colors.
  This is visible because class CSS such as `.task-type-*` and `.section-type-*` overrides SVG
  `fill` presentation attributes, so fixed CSS can make configured section/task colors appear to
  be ignored.
- Journey CSS now reads Mermaid 11.15 theme variables for `faceColor`, `mainBkg`, `nodeBorder`,
  `arrowheadColor`, `edgeLabelBackground`, `titleColor`, `tertiaryColor`, `border2`, `fillType0`
  through `fillType7`, and optional `actor0` through `actor5`.
- The generic `line` rule now follows upstream `textColor`; edge/flowchart link rules still use
  source-backed `lineColor`.
- Actor color CSS is only emitted when the corresponding `themeVariables.actorN` exists. This keeps
  default layout-derived actor colors intact while allowing theme variables to override the visible
  SVG when they are provided.

Touched production surfaces:

- [crates/merman-render/src/svg/parity/journey.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/journey.rs)

Focused verification:

- `cargo test -p merman-render journey_css_honors_mermaid_11_15_theme_options`
- `cargo run -p xtask -- compare-journey-svgs --check-dom --dom-mode parity --dom-decimals 3`

Sixth slice outcome:

- Audited pinned Mermaid 11.15 ER style provider:
  - `packages/mermaid/src/diagrams/er/styles.ts`
- ER previously emitted older hardcoded default-theme CSS for entity boxes, relationship labels,
  node shapes, relationship lines, markers, and label text. This could make custom/dark-theme ER
  diagrams structurally valid but visually stale or low contrast.
- ER CSS now reads Mermaid 11.15 theme variables for `mainBkg`, `nodeBorder`, `nodeTextColor`,
  `textColor`, `lineColor`, `errorBkgColor`, `errorTextColor`, `tertiaryColor`,
  `edgeLabelBackground`, optional `erEdgeLabelBackground`, and `strokeWidth` when `look: neo`.
- Added a narrow render-side `css_rgba_fade(...)` utility for the ER `fade(tertiaryColor, 0.5)`
  rule. It uses the existing `svgtypes` CSS color parser and returns `None` for unresolved runtime
  CSS values rather than pretending browser expression support.
- The upstream ER `[data-look][data-color-id]` color-theme rules and `[data-look=neo].labelBkg`
  rule were intentionally not emitted because current local ER SVGs do not emit the required
  attributes on those elements. Adding those inert rules would not improve visible renderability.

Touched production surfaces:

- [crates/merman-render/src/svg/parity.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity.rs)
- [crates/merman-render/src/svg/parity/css.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/css.rs)
- [crates/merman-render/src/svg/parity/util.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/util.rs)

Focused verification:

- `cargo test -p merman-render er_css_honors_mermaid_11_15_theme_options`
- `cargo test -p merman-render css_rgba_fade_parses_css_colors`
- `cargo run -p xtask -- compare-er-svgs --check-dom --dom-mode parity --dom-decimals 3`

Seventh slice outcome:

- Audited pinned Mermaid 11.15 Radar style provider:
  - `packages/mermaid/src/diagrams/radar/styles.ts`
- Radar already consumed `themeVariables.radar.*`, but ignored top-level `radar.*` style overrides.
  Mermaid 11.15 merges `themeVariables.radar` with the top-level `radar` config before emitting
  CSS, so user overrides such as `radar.axisColor`, `radar.curveOpacity`, and
  `radar.graticuleStrokeWidth` should affect visible output.
- Radar CSS now resolves `radar.<styleKey>` before `themeVariables.radar.<styleKey>` for axis,
  graticule, curve, and legend style options while keeping `cScale*` palette rules sourced from
  theme variables.

Touched production surfaces:

- [crates/merman-render/src/svg/parity/radar.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/radar.rs)

Focused verification:

- `cargo test -p merman-render radar_css_honors_top_level_style_overrides`
- `cargo run -p xtask -- compare-radar-svgs --check-dom --dom-mode parity --dom-decimals 3`

Eighth slice outcome:

- Audited pinned Mermaid 11.15 Block style provider:
  - `packages/mermaid/src/diagrams/block/styles.ts`
- Block already consumed most theme variables, but composite cluster CSS used raw `clusterBkg` and
  `clusterBorder` values. Mermaid 11.15 fades those colors for `.node .cluster`, which affects the
  visual weight and readability of nested block regions.
- Block cluster CSS now applies `css_rgba_fade(clusterBkg, 0.5)` and
  `css_rgba_fade(clusterBorder, 0.2)` when colors are parseable, with fallback to the configured
  value for unresolved runtime CSS expressions.

Touched production surfaces:

- [crates/merman-render/src/svg/parity/block.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/block.rs)
- [crates/merman-render/tests/block_svg_test.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/tests/block_svg_test.rs)

Focused verification:

- `cargo test -p merman-render block_svg_fades_cluster_theme_colors --test block_svg_test`
- `cargo run -p xtask -- compare-block-svgs --check-dom --dom-mode parity --dom-decimals 3`

Ninth slice outcome:

- Audited pinned Mermaid 11.15 Sequence style provider:
  - `packages/mermaid/src/diagrams/sequence/styles.js`
- Sequence CSS previously still mirrored an older hardcoded stylesheet: actor boxes, actor labels,
  lifelines, signal lines, message labels, label boxes, loop/section titles, notes, activation
  bars, base markers, and root text color ignored effective Mermaid theme variables.
- Sequence CSS now receives `effective_config`, uses the shared `SvgTheme` seam, and maps the
  source-backed Mermaid 11.15 options for `actorBorder`, `actorBkg`, `strokeWidth`, `actorTextColor`,
  `actorLineColor`, `signalColor`, `sequenceNumberColor`, `signalTextColor`,
  `labelBoxBorderColor`, `labelBoxBkgColor`, `labelTextColor`, `loopTextColor`,
  `noteBorderColor`, `noteBkgColor`, `noteTextColor`, optional numeric/string
  `noteFontWeight`, `activationBkgColor`, `activationBorderColor`, `nodeBorder`, `dropShadow`,
  `textColor`, `lineColor`, and error colors.
- Added `SvgTheme::optional_value(...)` so non-color CSS values such as `noteFontWeight` are not
  read through the misleading `optional_color(...)` API.
- Upstream Sequence `data-look` / `outer-path` neo selectors remain intentionally un-emitted in
  this slice because current local Sequence SVG elements do not emit those attributes; emitted CSS
  is limited to selectors that affect current render output.

Touched production surfaces:

- [crates/merman-render/src/svg/parity/sequence/css.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/sequence/css.rs)
- [crates/merman-render/src/svg/parity/sequence/render.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/sequence/render.rs)
- [crates/merman-render/src/svg/parity/util.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/util.rs)
- [crates/merman-render/tests/sequence_svg_test.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/tests/sequence_svg_test.rs)

Focused verification:

- `cargo fmt -p merman-render`
- `cargo fmt --check -p merman-render`
- `cargo test -p merman-render sequence_css_uses_configured_font_size`
- `cargo test -p merman-render sequence_css_honors_mermaid_11_15_theme_options`
- `cargo test -p merman-render sequence_svg_honors_mermaid_11_15_theme_css_options --test sequence_svg_test`
- `cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-mode parity --dom-decimals 3`

Residual note:

- This slice fixes Sequence theme/readability CSS emission only. It does not claim to close the
  known Sequence generated-measurement/root-width residuals documented under HPD-040 and HPD-060.

Tenth slice outcome:

- Audited pinned Mermaid 11.15 State style provider:
  - `packages/mermaid/src/diagrams/state/styles.js`
- State CSS previously emitted a mostly hardcoded 11.12-era stylesheet for state nodes, clusters,
  transitions, labels, notes, start/end markers, special states, and title text. This made dark and
  custom themes visually stale even when DOM structural parity stayed green.
- State CSS now reads Mermaid 11.15 theme variables through the shared `SvgTheme` seam for root
  text, error colors, `lineColor`, `transitionColor`, `nodeBorder`, `stateLabelColor`, `mainBkg`,
  `background`, `altBackground`, `strokeWidth`, `note*`, `labelBackgroundColor`,
  `edgeLabelBackground`, `transitionLabelColor`/`tertiaryTextColor`, `specialStateColor`,
  `innerEndBackground`, `compositeBackground`, `stateBkg`, `stateBorder`, and
  `compositeTitleBackground`.
- The local State marker id is prefixed (`<diagram>_stateDiagram-barbEnd`), so the stylesheet now
  uses source-backed suffix selectors (`[id$="-barbEnd"]`) instead of the old exact
  `#statediagram-barbEnd` selector that did not hit current output.
- The old `dependencyStart` / `dependencyEnd` CSS rule was removed from local State CSS because
  current local State SVG output does not emit those dependency marker ids.
- Upstream State neo cluster gradient/drop-shadow selectors remain intentionally un-emitted in this
  slice because local State rendering does not emit the corresponding gradient/drop-shadow defs.

Touched production surfaces:

- [crates/merman-render/src/svg/parity/state/style.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/state/style.rs)
- [crates/merman-render/tests/state_svg_test.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/tests/state_svg_test.rs)

Focused verification:

- `cargo fmt -p merman-render`
- `cargo fmt --check -p merman-render`
- `cargo test -p merman-render state_css_honors_mermaid_11_15_theme_options`
- `cargo test -p merman-render state_svg_honors_mermaid_11_15_theme_css_options --test state_svg_test`
- `cargo test -p merman-render --test state_svg_test`
- `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity --dom-decimals 3`

Residual note:

- This slice fixes State theme/readability CSS only. It does not claim State layout or root-bounds
  closure.

Eleventh slice outcome:

- Audited pinned Mermaid 11.15 Flowchart style provider:
  - `packages/mermaid/src/diagrams/flowchart/styles.ts`
- Flowchart CSS previously hardcoded node strokes to `1px` and edge-path strokes to `2.0px`.
  Mermaid 11.15 drives both from `themeVariables.strokeWidth` and appends `px` in the Flowchart
  stylesheet. This is visible for `theme: neo`, redux themes, and custom numeric
  `themeVariables.strokeWidth` values.
- Flowchart CSS now reads `strokeWidth` through `SvgTheme::css_value(...)`, preserving numeric
  theme defaults and user overrides instead of treating the value as a color or ignoring it.
- A focused Class audit during this slice found an unrelated structural namespace issue:
  `Outer.Foo --> Bar` style relations still differ from the pinned upstream fixture because local
  Class rendering collapses namespace-qualified relation ids that upstream keeps fully qualified
  and, in some cases, as a separate top-level class. That is recorded as a future Class semantic
  slice, not hidden inside this Flowchart CSS change.

Touched production surfaces:

- [crates/merman-render/src/svg/parity/flowchart/css.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/flowchart/css.rs)
- [crates/merman-render/tests/flowchart_svg_test.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/tests/flowchart_svg_test.rs)

Focused verification:

- `cargo fmt -p merman-render`
- `cargo fmt --check -p merman-render`
- `cargo test -p merman-render flowchart_svg_honors_mermaid_11_15_numeric_stroke_width_theme --test flowchart_svg_test`
- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3`

Residual note:

- This slice fixes Flowchart theme/readability CSS only. It does not claim Flowchart root-bounds
  closure.
- `cargo run -p xtask -- compare-class-svgs --check-dom --dom-mode parity --dom-decimals 3` was
  run as part of the adjacent Class audit and exposed three namespace structural mismatches
  (`stress_class_comments_inside_namespaces_024`,
  `stress_class_nested_namespaces_many_levels_021`, and `stress_class_unicode_namespace_mix_017`).
  Those mismatches were not caused by the Flowchart change and were handled by the dedicated
  source-backed Class namespace/qualified-id slice below.

Twelfth slice outcome:

- Audited pinned Mermaid 11.15 Class DB source:
  - `packages/mermaid/src/diagrams/class/classDb.ts`
- Mermaid 11.15 `addRelation(...)` calls `addClass(...)` for each relation endpoint and then keeps
  `classRelation.id1/id2` as `splitClassNameAndType(endpoint).className`. It does not resolve a
  namespace-qualified relation endpoint such as `Outer.Foo` back to an existing namespace member
  `Foo`.
- Local Class core had an ASCII-oriented shortcut that collapsed namespace-qualified relation
  endpoints to the namespace member id when that member existed. That made ASCII output concise, but
  broke pinned SVG structural parity for relation ids and implicit namespace-qualified facade class
  nodes.
- Class core now preserves Mermaid's namespace-qualified facade semantics for relation endpoints.
  The focused parser test documents the extra facade classes and fully-qualified relation endpoints.
- ASCII rendering keeps its user-friendly output by folding only empty namespace facade classes back
  to their declared namespace member as a view-layer alias. This avoids duplicate terminal boxes
  without changing the core semantic model or SVG parity surface.
- The Class SVG HTML-cap fixture was updated to the current deterministic headless output after the
  surrounding renderer changes; `compare-class-svgs` remains the authority for pinned upstream DOM
  structure.

Touched production surfaces:

- [crates/merman-core/src/diagrams/class/db.rs](/F:/SourceCodes/Rust/merman/crates/merman-core/src/diagrams/class/db.rs)
- [crates/merman-ascii/src/class/render.rs](/F:/SourceCodes/Rust/merman/crates/merman-ascii/src/class/render.rs)
- [crates/merman-core/src/diagrams/class/tests.rs](/F:/SourceCodes/Rust/merman/crates/merman-core/src/diagrams/class/tests.rs)
- [crates/merman-render/tests/class_svg_test.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/tests/class_svg_test.rs)

Focused verification:

- `cargo fmt -p merman-core -p merman-render -p merman-ascii`
- `cargo fmt --check -p merman-core -p merman-render -p merman-ascii`
- `cargo test -p merman-core class --lib`
- `cargo test -p merman-render --test class_svg_test`
- `cargo test -p merman-ascii class --test class_model`
- `cargo run -p xtask -- compare-class-svgs --check-dom --dom-mode parity --dom-decimals 3`

Residual note:

- This slice fixes Class namespace-qualified relation semantic/structural parity only. It does not
  claim Class root-bounds or browser text-measurement closure.

Thirteenth slice outcome:

- Audited pinned Mermaid 11.15 Timeline style provider:
  - `packages/mermaid/src/diagrams/timeline/styles.js`
- Timeline CSS already consumed `cScale*`, `cScaleLabel*`, `cScaleInv*`, `git0`, and
  `gitBranchLabel0`, but still hardcoded `.disabled` fills to `lightgray` and `#efefef`.
- Mermaid 11.15 emits `.disabled` node fill from `themeVariables.tertiaryColor` and disabled text
  fill from `themeVariables.clusterBorder`, with those hardcoded values only as missing-option
  fallbacks. Default pinned SVG baselines therefore contain expanded theme values such as
  `hsl(80, 100%, 96.2745098039%)` and `#aaaa33`.
- Timeline CSS now reads both values through the shared theme lookup path. This fixes a visible
  theme/readability gap without emitting inert redux/neo/gradient rules that local Timeline SVG
  nodes do not currently support.

Touched production surfaces:

- [crates/merman-render/src/svg/parity/timeline.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/timeline.rs)
- [crates/merman-render/tests/timeline_svg_test.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/tests/timeline_svg_test.rs)

Focused verification:

- `cargo fmt -p merman-render`
- `cargo fmt --check -p merman-render`
- `cargo test -p merman-render timeline_svg_honors_mermaid_11_15_disabled_theme_colors --test timeline_svg_test`
- `cargo run -p xtask -- compare-timeline-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `git diff --check`

Residual note:

- Timeline redux/neo gradient/drop-shadow selectors remain intentionally un-emitted where local SVG
  does not emit the required `data-look`/defs support. This slice fixes only source-backed CSS that
  applies to the current local Timeline stylesheet.

Fourteenth slice outcome:

- Audited pinned Mermaid 11.15 Architecture style provider and option type:
  - `packages/mermaid/src/diagrams/architecture/architectureStyles.ts`
  - `packages/mermaid/src/diagrams/architecture/architectureTypes.ts`
- Mermaid 11.15 Architecture CSS consumes `archEdgeColor`, `archEdgeArrowColor`,
  `archEdgeWidth`, `archGroupBorderColor`, and `archGroupBorderWidth` directly. The local theme
  expansion already populated these values, but Architecture CSS still emitted generic
  `lineColor`, `primaryBorderColor`, and hardcoded `3` / `2px` widths.
- Architecture CSS now reads those source-backed `arch*` theme variables through the shared config
  CSS-token path. Custom Architecture edge/group styling therefore reaches the final SVG stylesheet
  instead of being parsed and then ignored.

Touched production surfaces:

- [crates/merman-render/src/svg/parity/css.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/css.rs)
- [crates/merman-render/tests/architecture_svg_test.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/tests/architecture_svg_test.rs)

Focused verification:

- `cargo fmt -p merman-render`
- `cargo fmt --check -p merman-render`
- `cargo test -p merman-render architecture_css_with_config_honors_font_and_theme_colors`
- `cargo test -p merman-render architecture_svg_honors_mermaid_11_15_style_theme_variables --test architecture_svg_test`
- `cargo test -p merman-render --test architecture_svg_test`
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3`

Residual note:

- This slice fixed Architecture theme CSS emission only. It did not change Architecture layout,
  Cytoscape/manatee phase modeling, SVG root bounds, or the then-known 26 Architecture
  `parity-root` residuals.

Fifteenth slice outcome:

- Audited pinned Mermaid 11.15 Class note construction:
  - `packages/mermaid/src/diagrams/class/classDb.ts`
  - `packages/mermaid/src/diagrams/class/styles.js`
- Mermaid 11.15 writes `themeVariables.noteBkgColor` and `noteBorderColor` into each Class note
  node's `cssStyles`, while Class stylesheet uses `noteTextColor` for `.noteLabel` text.
- Local Class CSS already consumed `noteTextColor`, but both HTML-label and `htmlLabels:false`
  note shape render paths still hardcoded `#fff5ad` / `#aaaa33`. Custom note backgrounds and
  borders were therefore parsed but ignored in final SVG output.
- Class note rendering now reads `noteBkgColor` and `noteBorderColor` from `effective_config` for
  both note branches, preserving the same inline style shape that Mermaid emits after theme
  expansion.

Touched production surfaces:

- [crates/merman-render/src/svg/parity/class/note.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/class/note.rs)
- [crates/merman-render/tests/class_svg_test.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/tests/class_svg_test.rs)

Focused verification:

- `cargo fmt -p merman-render`
- `cargo fmt --check -p merman-render`
- `cargo test -p merman-render class_svg_honors_configured_note_theme_colors --test class_svg_test`
- `cargo test -p merman-render --test class_svg_test`
- `cargo run -p xtask -- compare-class-svgs --check-dom --dom-mode parity --dom-decimals 3`

Residual note:

- This slice fixes Class note theme/readability only. It does not change Class namespace cluster
  inline styling, Class layout, browser text measurement, or root-bounds residuals.

Sixteenth slice outcome:

- Audited pinned Mermaid 11.15 Class style provider:
  - `packages/mermaid/src/diagrams/class/styles.js`
- Class CSS already covered the basic label, relation, marker, and note text color rules, but it
  still missed source-backed rules that apply to current local output: `g.classGroup text`, cluster
  labels/rects, node shape selectors, dividers, `g.classGroup` rect/line, `.classLabel`,
  `.edgeTerminals`, and the source-shaped `.classTitleText` rule.
- The same audit found that Class `strokeWidth` was read through `theme.color(...)`, so numeric
  `themeVariables.strokeWidth` values were silently ignored. Mermaid 11.15 treats `strokeWidth` as
  a CSS token in the Class stylesheet.
- Class CSS now reads `strokeWidth` through `SvgTheme::css_value(...)` and emits the source-backed
  rules above for selectors that hit the current local SVG surface. This makes custom node/relation
  stroke widths and Class theme colors visible without adding browser-dependent measurement logic.
- Upstream icon rules and neo-only rules remain intentionally deferred where local Class rendering
  does not yet emit `.label-icon`, `data-look="neo"`, or the matching neo support shapes.

Touched production surfaces:

- [crates/merman-render/src/svg/parity/class/css.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/class/css.rs)
- [crates/merman-render/tests/class_svg_test.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/tests/class_svg_test.rs)

Focused verification:

- `cargo fmt -p merman-render`
- `cargo fmt --check -p merman-render`
- `cargo test -p merman-render class_svg_honors_numeric_stroke_width_theme_css --test class_svg_test`
- `cargo test -p merman-render --test class_svg_test`
- `cargo run -p xtask -- compare-class-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `git diff --check`

Residual note:

- This slice fixes Class stylesheet/theme emission only. It does not claim Class root-bounds,
  namespace cluster inline style, icon label, or neo rendering support closure.

Seventeenth slice outcome:

- Audited Zed integration feedback from
  [zed-industries/zed#57967](https://github.com/zed-industries/zed/pull/57967), which updates Zed
  from `merman = "0.4"` to `0.6` and adopts `SvgPipeline::resvg_safe()`.
- The reviewer-visible color changes in
  [issuecomment-4598335939](https://github.com/zed-industries/zed/pull/57967#issuecomment-4598335939)
  and the follow-up
  [issuecomment-4599604388](https://github.com/zed-industries/zed/pull/57967#issuecomment-4599604388)
  are host theme override concerns, not evidence that merman should inject Zed's palette by
  default. Zed's `color cleanup` commit rewrites its own background, edge-label, and tag-label
  injection behavior to preserve Zed's current visual style.
- The same commit changed Zed's fallback cleanup from "drop all fallback groups when any native
  text exists" to "drop only fallback groups whose text duplicates native SVG text." That is a
  general `resvg_safe` integration need: some diagrams need fallback labels for raster safety, while
  others can double-render labels when native SVG text already exists.
- Added `DropNativeDuplicateFallbacksPostprocessor` as a public optional pipeline pass exported
  through both `merman_render::svg` and `merman::render`. It preserves the default `resvg_safe()`
  contract while allowing hosts to compose:
  `SvgPipeline::resvg_safe().with_postprocessor(DropNativeDuplicateFallbacksPostprocessor)`.
- The pass uses the existing `data-merman-foreignobject="fallback"` and
  `merman-foreignobject-fallback-text` marker contract, collects native non-fallback `<text>`
  contents, and removes only fallback `<g>` groups whose normalized text duplicates a native label.

Touched production surfaces:

- [crates/merman-render/src/svg/pipeline/builtin/foreign_object.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/pipeline/builtin/foreign_object.rs)
- [crates/merman-render/src/svg/pipeline/builtin/mod.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/pipeline/builtin/mod.rs)
- [crates/merman-render/src/svg/pipeline/mod.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/pipeline/mod.rs)
- [crates/merman-render/src/svg.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg.rs)
- [crates/merman/src/render/mod.rs](/F:/SourceCodes/Rust/merman/crates/merman/src/render/mod.rs)

Focused verification:

- `cargo fmt -p merman-render -p merman`
- `cargo fmt --check -p merman-render -p merman`
- `cargo test -p merman-render drop_native_duplicate_fallbacks --lib`
- `cargo test -p merman-render resvg_safe_can_optionally_drop_native_duplicate_fallbacks --lib`
- `cargo test -p merman-render svg::pipeline --lib`
- `cargo test -p merman-render foreign_object --lib`
- `cargo test -p merman render_svg_sync_applies_scoped_theme_css_once --lib` compiled the top-level
  `merman` crate but matched zero tests.
- `git diff --check`

Known non-slice gate:

- Historical note: this slice originally observed unrelated measurement-sensitive failures in
  `cargo test -p merman-render --lib`. That is no longer the current default-test state. On
  2026-06-03, `cargo nextest run --workspace --all-features` passed `1680/1680` tests, and the
  Sequence width check now compares against the current single-run SVG bbox fact instead of a
  platform-fragile literal.

Residual note:

- This slice does not change default `resvg_safe()` behavior and does not make Zed's host theme CSS
  a merman default. Host palette injection remains the consumer's responsibility, while fallback
  marker/de-duplication is now available as a reusable pipeline contract.

Eighteenth slice outcome:

- Added
  [docs/workstreams/headless-parity-deepening/THEME_RENDERING_COVERAGE.md](/F:/SourceCodes/Rust/merman/docs/workstreams/headless-parity-deepening/THEME_RENDERING_COVERAGE.md)
  as the durable HPD-080 ledger for Mermaid 11.15 style-provider coverage and host theme boundaries.
- Re-audited the pinned Mermaid `11.15.0` source tree at
  `41646dfd43ac83f001b03c70605feb036afae46d` for diagram style providers, including provider names
  that are not simply `styles.js` / `styles.ts`:
  - `architecture/architectureStyles.ts`
  - `pie/pieStyles.ts`
  - conventional providers such as `class/styles.js`, `flowchart/styles.ts`,
    `sequence/styles.js`, and `treemap/styles.ts`
- Recorded implemented-matrix boundaries:
  - covered source-backed providers for current local output,
  - inline-only diagrams such as QuadrantChart and XYChart,
  - absent-provider diagrams such as Info/Error,
  - unsupported-family providers that belong to the admission rubric rather than HPD-080.
- Recorded negative gates:
  - do not add inert neo/gradient/drop-shadow/data-look CSS when local SVG does not emit the
    required elements, attributes, defs, or filters,
  - do not copy Zed-specific palette cleanup into default merman output,
  - do not globally remove root `background-color: white;` until upstream SVG baselines and
    Mermaid source/capture behavior are reconciled,
  - do not claim exact browser font parity through fixture-specific widths.
- Updated
  [docs/alignment/ZED_MERMAID_ISSUE_AUDIT.md](/F:/SourceCodes/Rust/merman/docs/alignment/ZED_MERMAID_ISSUE_AUDIT.md)
  with PR `zed-industries/zed#57967`, distinguishing host palette policy from the reusable fallback
  duplicate-cleanup pipeline contract.

Focused verification:

- `git -C repo-ref/mermaid ls-tree -r --name-only 41646dfd43ac83f001b03c70605feb036afae46d packages/mermaid/src/diagrams`
- `git -C repo-ref/mermaid grep -n "styles" 41646dfd43ac83f001b03c70605feb036afae46d -- packages/mermaid/src/diagrams/*/*Diagram.ts packages/mermaid/src/diagrams/*/*definition.ts packages/mermaid/src/diagrams/*/diagram.ts`
- `git -C repo-ref/mermaid show 41646dfd43ac83f001b03c70605feb036afae46d:packages/mermaid/src/diagrams/architecture/architectureStyles.ts`
- `git -C repo-ref/mermaid show 41646dfd43ac83f001b03c70605feb036afae46d:packages/mermaid/src/diagrams/pie/pieStyles.ts`

Nineteenth slice outcome:

- Audited Mermaid 11.15 XYChart theme behavior at pinned source commit
  `41646dfd43ac83f001b03c70605feb036afae46d`:
  - `xychartDb.ts` merges default `themeVariables.xyChart` with configured
    `config.themeVariables.xyChart`,
  - `xychartRenderer.ts` writes those values directly to SVG `fill` / `stroke` attributes,
  - `chartBuilder/interfaces.ts` defines `XYChartThemeConfig`,
  - `theme-default.js` and `theme-base.js` define the `xyChart` theme variable surface.
- Added a render-path regression in
  [crates/merman-render/tests/xychart_svg_test.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/tests/xychart_svg_test.rs)
  using
  [fixtures/xychart/upstream_cypress_xychart_spec_render_all_the_theme_color_018.mmd](/F:/SourceCodes/Rust/merman/fixtures/xychart/upstream_cypress_xychart_spec_render_all_the_theme_color_018.mmd).
- The regression asserts that custom `themeVariables.xyChart` values reach the final SVG for:
  chart background, chart title, x/y axis titles, x/y labels, x/y ticks, x/y axis lines, and bar/line
  plot palette colors.
- No production change was needed. This slice confirms the correct headless parity boundary for
  XYChart: inline theme config is supported, and no CSS provider should be invented.

Focused verification:

- `cargo fmt -p merman-render`
- `cargo fmt --check -p merman-render`
- `cargo test -p merman-render xychart_svg_honors_mermaid_11_15_inline_theme_config --test xychart_svg_test`
- `cargo test -p merman-render --test xychart_svg_test`
- `cargo run -p xtask -- compare-xychart-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `git diff --check`

Twentieth slice outcome:

- Added a public API dark-theme renderability smoke in
  [crates/merman/tests/theme_renderability_smoke.rs](/F:/SourceCodes/Rust/merman/crates/merman/tests/theme_renderability_smoke.rs)
  for Flowchart, Sequence, Kanban, GitGraph, and XYChart. The smoke checks readable labels,
  absence of broken geometry, and source-backed theme colors in final SVG output through
  `HeadlessRenderer`.
- The smoke found a real Flowchart theme omission. Mermaid 11.15
  `packages/mermaid/src/diagrams/flowchart/styles.ts` uses `nodeTextColor || textColor` for
  labels, while local Flowchart CSS only used `textColor`.
- Flowchart CSS now reads `themeVariables.nodeTextColor` and applies it to `.label` and
  `.label text, span`; `themeVariables.textColor` continues to drive root text fill.
- Added a focused renderer regression in
  [crates/merman-render/tests/flowchart_svg_test.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/tests/flowchart_svg_test.rs).
- The same smoke clarified a test boundary for Kanban: pinned Mermaid 11.15 source/fixtures emit
  `class="cluster undefined ..."` and `class="node undefined"` placeholder classes, and priority
  metadata is rendered as a side-line rather than priority text. The smoke therefore permits those
  upstream placeholder class tokens but still rejects real `NaN` geometry and any remaining
  `undefined` leakage outside that placeholder shape.

Focused verification:

- `cargo fmt -p merman-render -p merman`
- `cargo test -p merman-render flowchart_svg_honors_node_text_color_theme_variable --test flowchart_svg_test`
- `cargo test -p merman-render --test flowchart_svg_test`
- `cargo test -p merman representative_dark_theme_diagrams_keep_visible_theme_signals --test theme_renderability_smoke --features render`
- `cargo fmt --check -p merman-render -p merman`
- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo run -p xtask -- compare-kanban-svgs --check-dom --dom-mode parity --dom-decimals 3`

Twenty-first slice outcome:

- Audited QuadrantChart inline theme behavior against pinned Mermaid 11.15:
  - `packages/mermaid/src/themes/theme-default.js`
  - `packages/mermaid/src/diagrams/quadrant-chart/quadrantRenderer.ts`
  - khroma `lighten`, `darken`, and `luminance` helpers from the installed baseline toolchain.
- Mermaid 11.15 intends default `quadrantPointFill` to be a lightened/darkened
  `quadrant1Fill`, but the shipped source calls khroma `lighten` / `darken` without the required
  amount argument. The saved upstream fixtures therefore contain invalid
  `hsl(240, 100%, NaN%)` point fill/stroke tokens.
- Local QuadrantChart output now treats that as an upstream invalid-token defect rather than a
  product requirement. When no valid `themeVariables.quadrantPointFill` is present, merman derives
  a valid 10% lightness-shift point color from `quadrant1Fill`; valid explicit
  `quadrantPointFill` values still win verbatim.
- Added renderer coverage in
  [crates/merman-render/tests/quadrantchart_svg_test.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/tests/quadrantchart_svg_test.rs)
  for default point renderability and explicit point color/text overrides.
- Extended the public API dark-theme smoke to cover QuadrantChart inline theme variables.
- Updated xtask DOM parity normalization so only QuadrantChart default data-point circle
  `fill`/`stroke` maps upstream `hsl(...NaN%)` and local `rgb(185, 185, 255)` to one comparison
  slot. Strict mode still preserves the real attr difference.

Focused verification:

- `cargo fmt -p merman-render -p merman -p xtask`
- `cargo test -p merman-render --test quadrantchart_svg_test`
- `cargo test -p merman representative_dark_theme_diagrams_keep_visible_theme_signals --test theme_renderability_smoke --features render`
- `cargo test -p xtask parity_normalizes_quadrantchart_invalid_default_point_color`
- `cargo fmt --check -p merman-render -p merman -p xtask`
- `cargo run -p xtask -- compare-quadrantchart-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo run -p xtask -- compare-quadrantchart-svgs --check-dom --dom-mode parity-root --dom-decimals 3`

Residual note:

- This is an intentional renderability-over-byte-parity correction for an invalid upstream CSS
  token. It should not be generalized into broad color normalization or cosmetic palette changes.

Twenty-second slice outcome:

- Audited the remaining raw SVG `undefined` hits from the renderability scan:
  - `fixtures/er/basic.mmd`
  - `fixtures/mindmap/basic.mmd`
  - corresponding pinned upstream SVG fixtures.
- ER relationship paths and Mindmap edge paths both emitted
  `style="undefined;;;undefined"`. Pinned upstream fixtures contain the same artifact, but the
  attribute carries no useful visual semantics; edge appearance is driven by CSS classes such as
  `relationshipLine`, `edge`, `section-edge-*`, and `edge-depth-*`.
- Removed the invalid inline `style` attribute from local ER relationship paths and Mindmap edge
  paths instead of preserving a useless upstream token for byte parity.
- Added focused regression checks so the affected raw SVG paths no longer leak `style="undefined"`.

Focused verification:

- `cargo fmt -p merman-render -p merman`
- `cargo test -p merman-render er_svg_renders_entities_and_relationships --test er_svg_test`
- `cargo test -p merman mindmap_br_variants_031_matches_upstream_node_geometry --test mindmap_br_variants_031 --features render`
- `cargo run -p xtask -- compare-er-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo run -p xtask -- compare-mindmap-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo fmt --check -p merman-render -p merman`

Manual raw-output sample:

- `fixtures/er/basic.mmd`: no `style="undefined"`, no `undefined`, no `NaN`
- `fixtures/mindmap/basic.mmd`: no `style="undefined"`, no `undefined`, no `NaN`

Residual note:

- This is a narrow raw-SVG cleanliness fix. It does not imply that all empty `style=""` attributes
  should be removed, and it does not change ER or Mindmap layout/root residuals.

Twenty-third slice outcome:

- Rechecked Mermaid 11.15 theme registry evidence after the Zed host-theme audit:
  - `repo-ref/mermaid/packages/mermaid/src/themes/index.js`
  - `repo-ref/mermaid/packages/mermaid/src/config.type.ts`
  - `tools/mermaid-cli/node_modules/mermaid/dist/config.type.d.ts`
- Corrected the previous `neo/redux*` interpretation. These names are official Mermaid 11.15
  config themes, not snapshot-only local artifacts:
  - `neo`
  - `neo-dark`
  - `redux`
  - `redux-dark`
  - `redux-color`
  - `redux-dark-color`
- Core, bindings, and `@merman/web` now expose all 11 official Mermaid 11.15 theme names:
  `default`, `base`, `dark`, `forest`, `neutral`, `neo`, `neo-dark`, `redux`, `redux-dark`,
  `redux-color`, and `redux-dark-color`.
- Extended theme defaults expand from the generated Mermaid 11.15 theme-variable snapshot. Explicit
  direct `themeVariables` overrides still win, and unknown theme names still fall back to the
  default theme.
- This does not claim exact browser/source-equivalent override derivation for every `neo/redux*`
  variable. That remains a narrower follow-up audit instead of a fake parity claim.
- The Zed PR color cleanup remains classified as host palette policy. The common merman integration
  need is still fallback marking and optional duplicate fallback cleanup, not default editor-color
  rewriting.

Focused verification:

- `cargo fmt -p merman-core -p merman-bindings-core -p merman`
- `cargo test -p merman-core theme`
- `cargo test -p merman-bindings-core supported_themes_exposes_core_theme_surface`
- `cargo test -p merman external_ --features render`
- `npm run build:ts --prefix platforms/web`

Residual note:

- The Rust API can already support common host theme workflows through composed postprocessors.
  Non-Rust/JSON option consumers still need a first-class way to opt into duplicate fallback
  cleanup and possibly scoped host CSS policy; do not silently change `resvg_safe()` defaults for
  that.

Twenty-fourth slice outcome:

- Closed the generic part of the non-Rust host integration gap from the Zed/theme audit by adding
  `svg.drop_native_duplicate_fallbacks` to the shared binding `options_json` surface.
- The option defaults to `false`, so existing `parity`, `readable`, and `resvg-safe` pipeline
  contracts stay unchanged.
- When enabled, binding renderers build the selected `SvgPipeline` and append
  `DropNativeDuplicateFallbacksPostprocessor`, removing only fallback groups whose text duplicates
  native SVG `<text>` while preserving fallback-only labels.
- Updated `@merman/web` TypeScript options and `docs/bindings/OPTIONS_JSON.md` so host consumers do
  not need private JSON strings or downstream duplicate-fallback hacks.

Focused verification:

- `cargo fmt -p merman-bindings-core`
- `cargo test -p merman-bindings-core svg_options_can_drop_native_duplicate_fallbacks`
- `cargo test -p merman-bindings-core render_svg_accepts_options_json`
- `npm run build:ts --prefix platforms/web`

Residual note:

- Host palette replacement, root background replacement, scoped CSS injection, and
  `!important` cleanup are still not JSON options. Those require an explicit cascade/security
  design rather than a quick binding flag.

Twenty-fifth slice outcome:

- Re-audited Zed PR 57967 after the binding fallback option landed, including the `color cleanup`
  commit. The remaining Zed color differences are still host palette policy: white-background
  replacement, edge-label background/text fixes, and GitGraph tag-label text color are editor
  compatibility rules, not default Mermaid output requirements.
- Confirmed the current merman surface covers common Rust host theme needs:
  `HeadlessRenderer::with_site_config(...)`, Mermaid `theme` / `themeVariables` / `themeCSS`,
  `SvgPipeline::resvg_safe()`, `ScopedCssPostprocessor`, `CssOverridePostprocessor`, and
  `DropNativeDuplicateFallbacksPostprocessor`.
- Confirmed the shared binding surface covers raster-safe output and duplicate fallback cleanup, but
  does not yet expose external Mermaid site config or host-scoped CSS injection. That is an API
  ergonomics gap, not evidence for silently changing `resvg_safe()` defaults.
- Updated `THEME_RENDERING_COVERAGE.md` with a common-host-needs table so future HPD-080 work does
  not conflate Mermaid theme support with product-specific palette rewriting.

Focused verification:

- `gh pr view 57967 --repo zed-industries/zed --comments --json title,url,mergeStateStatus,state,body,files,commits,comments,reviews`
- `gh api repos/zed-industries/zed/commits/c85f29cd2e78ec8a68b20349606d8298eecf37bb --jq ...`
- `cargo nextest run -p merman-core theme`
- `cargo nextest run -p merman-bindings-core supported_themes_exposes_core_theme_surface svg_options_can_drop_native_duplicate_fallbacks`
- `cargo nextest run -p merman --features render external_ render_svg_sync_applies_scoped_theme_css_once`
- `npm run build:ts --prefix platforms/web`

Residual note:

- The next useful binding split is Mermaid `site_config` first and host-scoped CSS later. Host CSS
  still needs an explicit security/cascade/raster-safety contract; do not add a quick Zed-specific
  palette flag.

Twenty-sixth slice outcome:

- Closed the Mermaid-config part of the binding host-theme gap by adding top-level
  `options_json.site_config` to [crates/merman-bindings-core/src/lib.rs](/F:/SourceCodes/Rust/merman/crates/merman-bindings-core/src/lib.rs).
- `site_config` is validated as a JSON object and mapped to `HeadlessRenderer::with_site_config(...)`
  / `HeadlessAsciiRenderer::with_site_config(...)`, so binding consumers can pass `theme`,
  `themeVariables`, diagram options, and Mermaid `themeCSS` without embedding init directives in
  the source text.
- Updated `@merman/web` `BindingOptions` and `docs/bindings/OPTIONS_JSON.md` with the same
  top-level `site_config` contract.
- Kept host palette CSS as a boundary. This slice does not add a Zed-specific background or label
  color option and does not change `resvg_safe()` defaults.
- Updated `THEME_RENDERING_COVERAGE.md` so common host theme needs now distinguish supported
  Mermaid site config from still-manual host-owned palette postprocessing.

Focused verification:

- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo test -p merman-bindings-core site_config --lib`
- `cargo fmt --check -p merman-bindings-core`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo test -p merman-bindings-core --lib`
- `npm run build:ts --prefix platforms/web`
- JSONL validation for `CONTEXT.jsonl`, `TASKS.jsonl`, and `CAMPAIGNS.jsonl`
- `git diff --check`

Residual note:

- Binding host-owned scoped CSS remains the next possible API split. Plain Mermaid `themeCSS` is
  already covered through `site_config`.
- The default MSVC linker was not on PATH in this shell (`link.exe` not found), so Rust tests were
  verified with the toolchain-provided `rust-lld` linker.

Twenty-seventh slice outcome:

- Closed the binding host-owned CSS half of the Zed/theme integration gap by adding `svg.scoped_css`
  and `svg.css_override_policy` to [crates/merman-bindings-core/src/lib.rs](/F:/SourceCodes/Rust/merman/crates/merman-bindings-core/src/lib.rs).
- The binding API maps `svg.scoped_css` to `ScopedCssPostprocessor`, scopes selectors to the root
  SVG id, and injects host CSS after Mermaid CSS for normal cascade order.
- `svg.css_override_policy` accepts `preserve`, `strip-existing-important`, and
  `strip_existing_important`. Invalid values return `MERMAN_INVALID_ARGUMENT`.
- For `svg.pipeline="resvg-safe"`, the binding pipeline now runs `SanitizeCssPostprocessor` after
  host CSS injection, preserving the raster-safe preset's CSS-sanitization contract for injected
  host CSS.
- Updated `@merman/web`, `docs/bindings/OPTIONS_JSON.md`,
  `docs/rendering/SVG_OUTPUT_PIPELINE.md`, and `THEME_RENDERING_COVERAGE.md`.
- This still does not add Zed-specific palette defaults, root background stripping, or any change
  to the default Mermaid parity SVG output.

Focused verification:

- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo test -p merman-bindings-core scoped_css --lib`
- `cargo fmt -p merman-bindings-core`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo test -p merman-bindings-core --lib`
- `npm run build:ts --prefix platforms/web`
- JSONL validation for `CONTEXT.jsonl`, `TASKS.jsonl`, and `CAMPAIGNS.jsonl`
- `git diff --check`

Residual note:

- Binding host CSS is now an explicit host-owned option. The remaining root white-background
  question is still a source/capture audit before any default output policy changes.

Twenty-eighth slice outcome:

- Reconciled the root white-background question against pinned Mermaid 11.15 source, installed
  Mermaid 11.15 dist, and local capture code.
- Source-backed finding:
  - pinned `packages/mermaid/src/setupGraphViewbox.js` sets root `width="100%"` and
    `style="max-width: ...px;"` when `useMaxWidth` is enabled,
  - installed Mermaid 11.15 dist has the same `calculateSvgSizeAttrs(...)` behavior,
  - `xtask` upstream capture injects `background-color: white` by default through
    `ensureSvgBackgroundColor(...)`,
  - local parity renderers preserve that capture-compatible white root background across many
    implemented diagram families.
- Added `RootBackgroundPostprocessor` as the product-neutral host seam for this common editor/raster
  integration need. It rewrites only the root `<svg>` inline `background-color` or adds one when
  missing; it does not rewrite Mermaid-owned node, edge, label, or semantic palette colors.
- Added shared binding option `svg.root_background_color`, plus `@merman/web` typing and binding
  validation. This lets non-Rust hosts set the root canvas color without depending on CSS cascade
  over an inline style.
- Updated `THEME_RENDERING_COVERAGE.md`, `docs/rendering/SVG_OUTPUT_PIPELINE.md`, and
  `docs/bindings/OPTIONS_JSON.md` to classify root canvas replacement as supported opt-in host
  policy, not a default Mermaid parity output change.

Focused verification:

- `git -C repo-ref/mermaid show 41646dfd43ac83f001b03c70605feb036afae46d:packages/mermaid/src/setupGraphViewbox.js`
- `Get-Content tools/mermaid-cli/node_modules/mermaid/dist/chunks/mermaid.core/chunk-CSCIHK7Q.mjs`
  around `calculateSvgSizeAttrs(...)`
- `rg "ensureSvgBackgroundColor|background_color" crates/xtask/src/cmd/generate.rs`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo test -p merman-render root_background --lib`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo test -p merman-bindings-core root_background --lib`
- `npm run build:ts --prefix platforms/web`
- JSONL validation for `CONTEXT.jsonl`, `TASKS.jsonl`, and `CAMPAIGNS.jsonl`
- `git diff --check`

Residual note:

- Do not globally strip or recolor default root backgrounds. They remain part of the current
  fixture/capture comparison surface. Host canvas color is now explicit opt-in policy through the
  Rust pipeline or binding options.

Twenty-ninth slice outcome:

- Re-audited Zed PR 57967 and the current 0.7 host-theme surface for common consumer needs after
  the `site_config`, scoped CSS, duplicate fallback, and root-background binding options landed.
- The current product-neutral support is sufficient for common host theme flows:
  - official Mermaid theme selection and `themeVariables` through Rust `with_site_config(...)` or
    binding `options_json.site_config`,
  - Mermaid diagram-owned `themeCSS`,
  - host-owned scoped CSS with optional `!important` stripping,
  - `resvg-safe` fallback insertion / `foreignObject` stripping / CSS and attribute cleanup,
  - optional duplicate native/fallback text cleanup,
  - optional root canvas color replacement.
- Kept Zed-style exact editor palette cleanup as a host boundary. Rust hosts can write custom
  `SvgPostprocessor` passes for arbitrary element/inline-style rewrites; shared bindings expose
  only product-neutral controls and intentionally do not provide a generic XML rewrite DSL.
- Tightened docs so `docs/bindings/OPTIONS_JSON.md` shows
  `svg.drop_native_duplicate_fallbacks` in the full JSON shape, and
  `THEME_RENDERING_COVERAGE.md` now calls out the optional exact-text nature of fallback
  de-duplication instead of implying semantic/geometric equivalence.

Focused verification:

- `gh pr view 57967 --repo zed-industries/zed --comments --json title,url,state,mergeStateStatus,body,files,commits,comments,reviews`
- `gh api repos/zed-industries/zed/commits/c85f29cd2e78ec8a68b20349606d8298eecf37bb --jq '.files[] | {filename,patch}'`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-bindings-core`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-render drop_native_duplicate_fallbacks`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-render root_background`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render external_`

Residual note:

- Exact `neo/redux*` override derivation remains an honest follow-up only if a fixture or consumer
  proves direct `themeVariables` plus generated snapshots are insufficient.
- The duplicate fallback cleanup is exact-text based and optional. It is useful for Zed-like
  raster duplicate labels, but hosts with intentionally repeated labels may still prefer a custom
  geometry-aware cleanup pass.

Thirtieth slice outcome:

- Expanded the public API dark-theme renderability smoke in
  [crates/merman/tests/theme_renderability_smoke.rs](/F:/SourceCodes/Rust/merman/crates/merman/tests/theme_renderability_smoke.rs)
  from the earlier representative set to cover Class, State, Architecture, Block, Journey, Radar,
  Requirement, Timeline, Gantt, Treemap, and Pie as well.
- The smoke remains semantic rather than pixel-based: it verifies SVG output, rejects `NaN`,
  rejects unexpected `undefined` tokens, and checks source-backed visible labels plus theme colors
  that the current local renderer should emit.
- Requirement labels in the smoke use the renderer's visible source-backed text shape
  (`Risk: High`, `Verification: Analysis`) rather than raw parser token spelling.
- Timeline's `class="node-bkg node-undefined"` is narrowly allowed after checking Mermaid 11.15
  upstream SVG fixtures. This is the same class of upstream placeholder as the earlier Kanban
  `cluster undefined` / `node undefined` class shape, not a local visible rendering failure.

Focused verification:

- `rg -n "node-undefined|undefined" fixtures\upstream-svgs\timeline repo-ref\mermaid\packages\mermaid\src\diagrams\timeline -S`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render representative_dark_theme_diagrams_keep_visible_theme_signals`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render --test theme_renderability_smoke`
- `cargo fmt --check -p merman`
- JSONL validation for `CONTEXT.jsonl`, `TASKS.jsonl`, and `CAMPAIGNS.jsonl`
- `git diff --check`

Residual note:

- This is a public renderability contract gate, not a full visual parity metric. It should catch
  blank output, unreadable labels, missing emitted theme colors, and invalid visible tokens without
  pretending to measure browser font or pixel parity exactly.

Thirty-first slice outcome:

- Extended the same public API dark-theme renderability smoke to the remaining supported diagram
  families with compact source-backed theme/config signals: ER, Mindmap, C4, Packet, and Sankey.
- This closes the earlier public-smoke gap where those diagrams had renderer-level CSS/config
  coverage but no public `HeadlessRenderer` route proving labels and configured colors survived the
  full parse/layout/render path.
- No production rendering change was needed. The only calibration was Mindmap: `nodeBorder` is a
  root span color only for redux-style output, so the smoke uses `theme: "redux"` before asserting
  that color.
- C4 is intentionally checked through visible C4 config colors rather than broad Mermaid
  `themeVariables`, because Mermaid 11.15's C4 style provider is narrow and most visible C4 palette
  behavior is C4 config or per-element style rather than generic theme-variable CSS.

Focused verification:

- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render representative_dark_theme_diagrams_keep_visible_theme_signals`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render --test theme_renderability_smoke`
- `cargo fmt --check -p merman`
- JSONL validation for `CONTEXT.jsonl`, `TASKS.jsonl`, and `CAMPAIGNS.jsonl`
- `git diff --check`

Residual note:

- Info and Error still have no Mermaid 11.15 diagram-specific style provider. ZenUML remains an
  external plugin compatibility boundary. They should be audited only for concrete visible failures,
  not forced into the same theme smoke.

Thirty-second slice Mermaid source-checkout audit:

- Re-audited HPD-080 style-provider discovery against the workstream's source authority after a
  suspicious `railroad` / `cynefin` provider hit. The local `repo-ref/mermaid` checkout had drifted
  to `develop` at `9bae92cd3214f9ec99369ab314ef41ffb283f6b6`, while
  `tools/upstreams/REPOS.lock.json` pins Mermaid `11.15.0` to
  `41646dfd43ac83f001b03c70605feb036afae46d`.
- Verified the pinned commit directly with `git -C repo-ref/mermaid ls-tree ...`: `railroad` and
  `cynefin` are absent from the locked `packages/mermaid/src/diagrams` tree, while the unsupported
  family set `treeView`, `ishikawa`, `eventmodeling`, `venn`, and `wardley` remains present.
- Restored `repo-ref/mermaid` to detached HEAD at the lockfile commit. This was a reference-state
  repair, not a renderer code change.
- Re-ran style-provider discovery after the restore. The supported-family coverage in
  `THEME_RENDERING_COVERAGE.md` remains consistent with the pinned Mermaid 11.15 source; no new
  HPD-080 renderer defect was found in this scan.

Focused verification:

- `git -C repo-ref/mermaid rev-parse HEAD`
  returned `41646dfd43ac83f001b03c70605feb036afae46d` after the restore.
- `git -C repo-ref/mermaid ls-tree -d --name-only 41646dfd43ac83f001b03c70605feb036afae46d:packages/mermaid/src/diagrams`
  listed no `railroad` or `cynefin` directory.
- `rg --files repo-ref/mermaid/packages/mermaid/src/diagrams | rg "(style|styles|architectureStyles|ishikawaStyles|pieStyles)\.(ts|js)$"`
  matched the expected pinned-provider inventory.

Thirty-third slice outcome:

- Re-audited Zed PR `57967` and confirmed the current 0.7 theme surface supports common
  product-neutral host needs: Mermaid `theme` / `themeVariables` / `themeCSS`,
  binding `options_json.site_config`, host `svg.scoped_css` / `css_override_policy`,
  `resvg-safe`, duplicate native/fallback cleanup, and root background replacement.
- Kept Zed's exact background, edge-label, tag-label, and accent cleanup classified as host palette
  policy. No default Mermaid theme behavior changed for this part.
- Followed the latest Zed PR thread to PR `58325`, where Zed fixed a stack overflow in deeply
  nested Flowchart subgraphs on their merman fork. Local 0.7 still had the same class of unbounded
  recursive traversal in Flowchart cluster handling.
- Converted Flowchart cluster traversal seams in
  [crates/merman-render/src/flowchart/layout.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/flowchart/layout.rs)
  from Rust call-stack recursion to explicit stacks:
  `compute_effective_dir_by_id`, `extract_descendants`, `flowchart_find_non_cluster_child`, and
  `copy_cluster`.
- Added a 512 KB stack-thread regression covering 10,000 nested subgraphs across those traversal
  seams. The bounded recursive cluster layout depth rule remains unchanged.

Focused verification:

- `gh pr view 57967 --repo zed-industries/zed --json title,url,commits,comments,reviews,files`
- `gh issue view 58325 --repo zed-industries/zed --json title,url,state,body,comments,labels`
- `gh api repos/zed-industries/merman/commits/1c765dcca2ef5092fcde7bebe8374819563623ef --jq '.files[] | {filename, patch}'`
- `npm run build:ts --prefix platforms/web`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-bindings-core`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-render drop_native_duplicate_fallbacks root_background scoped_css`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-render flowchart_cluster_traversals_handle_deep_subgraphs_with_small_stack`
- `cargo fmt --check -p merman-render`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-render --test flowchart_layout_test`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-render --test flowchart_svg_test`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render --test theme_renderability_smoke`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render --test zed_pr_57644_corpus`

Thirty-fourth slice outcome:

- Added a public API resvg-safe host-integration smoke in
  [crates/merman/tests/resvg_safe_fixture_smoke.rs](/F:/SourceCodes/Rust/merman/crates/merman/tests/resvg_safe_fixture_smoke.rs).
- The smoke covers the user-provided Kanban metadata and GitGraph merge samples, plus a compact
  dark-theme Flowchart case that proves visible theme colors survive
  `HeadlessRenderer::render_svg_resvg_safe_sync(...)`.
- It also samples supported fixture families deterministically: each available `basic.mmd`, any
  `zed_pr_57644_*.mmd` fixture, and a small sorted set of representative stress/upstream fixtures
  per family.
- The gate rejects host-visible/raster hazards: malformed XML, remaining `<foreignObject>`,
  unsupported CSS constructs, invalid visual values such as `NaN`, `Infinity`, and
  `fill="undefined"`, and empty style elements.
- When executed with the `raster` feature, the same gate converts each resvg-safe SVG to PNG bytes.
  This catches usvg/resvg-level failures that a string-only SVG check would miss.
- No production renderer defect was found in this slice. Treat this as a functional regression
  safety net, not a precise all-fixture or pixel parity metric.

Focused verification:

- `cargo fmt --check -p merman`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render --test resvg_safe_fixture_smoke`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features raster --test resvg_safe_fixture_smoke`

## HPD-080 - All-Supported Resvg-Safe Audit And Treemap ClassDef

Outcome:

- Resolved the Flowchart `layout.rs` conflict between the Zed PR `58325` backport shape and the
  local explicit-stack cluster traversal follow-up. The merged file keeps the
  `MAX_DIAGRAM_NESTING_DEPTH` model guard and the 512 KB stack-thread regressions for deep helper
  traversals.
- Rechecked Zed PR `57967`: Zed's background/text/accent cleanup is still host policy, while the
  current merman surface covers common product-neutral host needs through `site_config`,
  `themeCSS`, scoped host CSS, optional `!important` cleanup, root-background replacement, and
  duplicate native/fallback cleanup.
- Extended the resvg-safe smoke with a manual ignored all-supported audit. The audit intentionally
  skips parser-only fixtures, upstream docs placeholders such as `...`, and pinned upstream-invalid
  examples, then renders the supported fixture set through
  `HeadlessRenderer::render_svg_resvg_safe_sync(...)`.
- Fixed an upstream-source-backed Treemap input compatibility gap found by that audit. Mermaid
  `TreeMapDB.addClass(...)` accepts bare label-style tokens such as `color`; local Treemap parsing
  no longer rejects those tokens as parse errors.
- Kept headless output valid rather than copying invalid CSS. Treemap SVG style compilation now
  drops empty-valued declarations, so a Mermaid-compatible bare token like `color` does not leak
  `color: !important` or `fill: !important` into final SVG output.
- Fixed section-less Pie root output for headless/raster safety. Mermaid's browser/capture path can
  serialize `viewBox="0 0 -Infinity 450"` for empty Pie input; local headless output now emits the
  finite `viewBox="0 0 450 450"` instead of preserving a raster-hostile invalid token.

Focused verification:

- `cargo fmt --check -p merman-core -p merman-render -p merman`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-render flowchart_cluster_traversals_handle_deep_subgraphs_with_small_stack extract_descendants_handles_deeply_nested_subgraphs`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render --test theme_renderability_smoke`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-render drop_native_duplicate_fallbacks root_background scoped_css`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-bindings-core supported_themes_exposes_core_theme_surface svg_options_can_drop_native_duplicate_fallbacks svg_options_can_inject_host_scoped_css svg_options_can_set_root_background_color`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-render pie --test pie_svg_test`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-core treemap_classdef_allows_bare_label_style_tokens_like_mermaid`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-render treemap --test treemap_svg_test`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render --test resvg_safe_fixture_smoke`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render --test resvg_safe_fixture_smoke --run-ignored ignored-only all_supported_fixtures_render_headless_resvg_safe_audit`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features raster --test resvg_safe_fixture_smoke`
- `git diff --check`

## HPD-080 - Extended Theme Override Derivations

Outcome:

- Re-audited official Mermaid 11.15 `neo/redux*` theme behavior after the host-theme boundary work
  left exact extended-theme override derivation as an honest follow-up.
- Confirmed that no-override defaults should continue to come from the generated
  `theme_variables_11_15_0.json` snapshots, but user override handling cannot be a static snapshot
  merge. Mermaid theme modules run `calculate(overrides)`: copy user base keys, call
  `updateColors()`, then re-apply explicit user keys so direct derived-key overrides win.
- Added a bounded source-backed derivation seam in
  [crates/merman-core/src/theme.rs](/F:/SourceCodes/Rust/merman/crates/merman-core/src/theme.rs).
  It recomputes visible current-renderer derived keys for extended themes when users override
  source base keys such as `primaryColor`, `secondaryColor`, `background`, `lineColor`, and
  `mainBkg`.
- Kept the seam intentionally narrower than a full hand-port of every extended theme line. It
  covers keys consumed by current SVG renderers: Flowchart edge-label/icon surfaces, shared line and
  arrow colors, Architecture edge colors, Requirement relation colors, Sequence actor/label-box
  backgrounds, C4 person backgrounds, State backgrounds/transitions, and GitGraph tag-label
  background.
- Preserved explicit override precedence. If the user directly sets a derived key such as
  `nodeBkg` or `edgeLabelBackground`, local theme expansion leaves that value in place after
  derivation.
- Added a Flowchart SVG regression proving `theme: "redux"` plus
  `themeVariables.primaryColor = "#123456"` derives the Mermaid source secondary color into
  visible edge-label CSS, while correctly keeping Redux node fill on the `mainBkg` default
  `#ffffff`.

Source evidence:

- `repo-ref/mermaid/packages/mermaid/src/themes/index.js`
- `repo-ref/mermaid/packages/mermaid/src/themes/theme-neo.js`
- `repo-ref/mermaid/packages/mermaid/src/themes/theme-redux.js`
- `repo-ref/mermaid/packages/mermaid/src/themes/theme-redux-dark.js`
- `repo-ref/mermaid/packages/mermaid/src/themes/theme-redux-color.js`
- `repo-ref/mermaid/packages/mermaid/src/themes/theme-redux-dark-color.js`
- Installed Mermaid `11.15.0` dist probes through
  `tools/mermaid-cli/node_modules/mermaid/dist/mermaid.core.mjs`.

Focused verification:

- `cargo fmt --check -p merman-core -p merman-render`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-core theme`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-render --test flowchart_svg_test`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render --test theme_renderability_smoke`

Residual note:

- Future `neo/redux*` work should extend the source-backed derivation seam only when a fixture or
  consumer proves a currently emitted surface still misses Mermaid's override-derived value. Do not
  replace the generated default snapshots with fixture-keyed constants.

## HPD-080 - Extended Theme Dark Palette Derivations

Outcome:

- Continued the official `neo/redux*` override audit against Mermaid 11.15 dark extended themes.
- Confirmed from pinned source and installed Mermaid output that `neo-dark` / `redux-dark` /
  `redux-dark-color` with `themeVariables.primaryColor` derive visible
  `requirementBackground`, `pie1`, and `quadrant1Fill`.
- Confirmed `redux-dark` / `redux-dark-color` also derive GitGraph `git0..git7` plus
  `gitInv0..gitInv7` from the current palette, and that explicit `gitN` values should derive
  `gitInvN` unless the inverse key is explicit too.
- Extended [crates/merman-core/src/theme.rs](/F:/SourceCodes/Rust/merman/crates/merman-core/src/theme.rs)
  with HSL color parsing, dark extended-theme primary palette derivation, `redux-dark*` GitGraph
  palette/inverse derivation, and explicit `gitN` inverse derivation.
- Found and fixed a second production rendering gap while adding render-path coverage:
  [crates/merman-render/src/pie.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/pie.rs)
  previously built slice/legend colors from a hardcoded default palette. Pie layout now reads
  `effective_config.themeVariables.pie1..pie12`, preserving the default color-domain behavior when
  theme variables are absent.
- Added Pie and QuadrantChart regressions proving `redux-dark` `primaryColor` overrides reach
  visible slice/quadrant fills.

Source evidence:

- `repo-ref/mermaid/packages/mermaid/src/themes/theme-neo-dark.js`
- `repo-ref/mermaid/packages/mermaid/src/themes/theme-redux-dark.js`
- `repo-ref/mermaid/packages/mermaid/src/themes/theme-redux-dark-color.js`
- Installed Mermaid `11.15.0` dist probe through
  `tools/mermaid-cli/node_modules/mermaid/dist/mermaid.core.mjs`.

Focused verification:

- `cargo fmt --check -p merman-core -p merman-render -p merman`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-core theme`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-render --test pie_svg_test`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-render --test quadrantchart_svg_test`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render --test theme_renderability_smoke`

Residual note:

- This still avoids broad snapshot replacement or browser-color overfitting. The seam should grow
  only from source-backed rules that affect currently emitted SVG surfaces.

## HPD-080 - Journey Arrowhead Visible Signal Audit

Outcome:

- Audited Journey `themeVariables.arrowheadColor` after the renderability smoke had counted it as a
  visible theme signal.
- Confirmed pinned Mermaid 11.15 `user-journey/styles.js` emits `.arrowheadPath`, but
  `user-journey/svgDraw.js` creates the marker path without that class. Local Journey output mirrors
  that marker DOM.
- Removed `arrowheadColor` from the Journey case in
  [crates/merman/tests/theme_renderability_smoke.rs](/F:/SourceCodes/Rust/merman/crates/merman/tests/theme_renderability_smoke.rs)
  so the smoke no longer treats an inert CSS token as visible coverage.
- Updated [docs/workstreams/headless-parity-deepening/THEME_RENDERING_COVERAGE.md](/F:/SourceCodes/Rust/merman/docs/workstreams/headless-parity-deepening/THEME_RENDERING_COVERAGE.md)
  to track Journey arrowhead color as an upstream-inert provider rule, not a renderability signal.

Source evidence:

- `repo-ref/mermaid/packages/mermaid/src/diagrams/user-journey/styles.js`
- `repo-ref/mermaid/packages/mermaid/src/diagrams/user-journey/svgDraw.js`
- `crates/merman-render/src/svg/parity/journey.rs`

Focused verification:

- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render --test theme_renderability_smoke`

Residual note:

- No renderer DOM class was added. If merman later chooses to make `.arrowheadPath` effective as a
  headless renderability improvement, it should be tracked explicitly as a deliberate DOM divergence
  from pinned Mermaid output.

## HPD-080 - Resvg-Safe Audit Filtering

Outcome:

- Ran the ignored all-supported `resvg-safe` audit with render-only features and confirmed it passes
  across the current supported fixture corpus.
- Attempted the unfiltered all-supported raster audit; it exceeded the command timeout before
  producing fixture-level signal. Representative raster `resvg-safe` smoke still passed.
- Added optional filters to
  [crates/merman/tests/resvg_safe_fixture_smoke.rs](/F:/SourceCodes/Rust/merman/crates/merman/tests/resvg_safe_fixture_smoke.rs):
  `MERMAN_RESVG_SAFE_AUDIT_FAMILY` and `MERMAN_RESVG_SAFE_AUDIT_FILTER`.
- Default ignored-audit behavior is unchanged. Filtered audits now accept non-empty filtered corpora,
  which makes PNG-level triage usable without turning the full all-supported raster pass into a
  blocking gate.

Focused verification:

- `cargo fmt --check -p merman`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render --test resvg_safe_fixture_smoke`
- `$env:RUSTFLAGS='-C linker=rust-lld'; $env:MERMAN_RESVG_SAFE_AUDIT_FAMILY='journey'; cargo nextest run -p merman --features raster --test resvg_safe_fixture_smoke --run-ignored ignored-only all_supported_fixtures_render_headless_resvg_safe_audit`

Residual note:

- Use filtered raster audits for failure triage and keep representative raster smoke as the routine
  gate. The unfiltered raster corpus is still useful manually, but it should not be treated as a
  normal fast-pass command.

## HPD-080 - Raster Ink Renderability Gate

Outcome:

- Tightened the raster branch of
  [crates/merman/tests/resvg_safe_fixture_smoke.rs](/F:/SourceCodes/Rust/merman/crates/merman/tests/resvg_safe_fixture_smoke.rs)
  so PNG conversion no longer proves only "bytes were produced".
- The gate now decodes the generated PNG and requires contentful diagrams to contain visible pixels
  that differ from the first-pixel background. This catches gross host-visible failures where an SVG
  parses and rasterizes but the diagram is shifted out of the viewport or rendered as an
  all-background image.
- The gate is intentionally source-aware, not pixel-parity based. Header-only, accessibility-only,
  and title-only metadata fixtures still must emit parseable/resvg-safe SVG and rasterize to PNG,
  but they do not require non-background ink.
- This calibration is source-backed. Pinned Mermaid 11.15 Architecture parses
  `architecture-beta title ...` into `db.getDiagramTitle()`, but the stored upstream SVG for
  `upstream_architecture_title_first_line_spec` contains no visible title and pinned
  `architectureRenderer.ts` still says title support is TODO. Treating that title-only fixture as
  required visible ink would be a false renderability failure.
- Added a small source-content gate regression so inline content such as `graph TD;a-X-node;` is
  still classified as visible content while `accTitle` / `accDescr` lines and `accDescr { ... }`
  blocks are not.

Focused verification:

- `cargo fmt --check -p merman`
- `cargo nextest run -p merman --features raster --test resvg_safe_fixture_smoke`
- `$env:MERMAN_RESVG_SAFE_AUDIT_FAMILY='architecture,class,sequence'; cargo nextest run -p merman --features raster --test resvg_safe_fixture_smoke --run-ignored ignored-only all_supported_fixtures_render_headless_resvg_safe_audit`

Residual note:

- This is a gross renderability smoke, not a visual diff. It should fail on blank/all-background
  PNGs for diagrams with actual diagram content, while leaving fine color, antialiasing, and title
  rendering parity to source-backed focused tests.

## HPD-080 - Raster Ink Calibration And Single-Leaf Treemap

Outcome:

- Calibrated the raster ink source-content detector for parser/header/options fixtures that do not
  produce visible marks: Journey section-only sources, `packet-beta` header-only input, Radar
  option-only input, and Treemap root/classDef-only input no longer falsely require non-background
  ink.
- Found a real contentful raster failure in
  `fixtures/treemap/upstream_pkgtests_treemap_test_032.mmd`: a single top-level Treemap value
  inherited Mermaid 11.15's first color-scale fill of `transparent` and default `cScaleLabel0`
  white text, making the PNG all-background on the white root canvas.
- Kept Mermaid's transparent first leaf fill, but Treemap now uses `themeVariables.textColor` for
  leaf label/value inline fill only when the generated leaf fill is transparent, no explicit
  class/style fill overrides it, and the generated label color is white/near-white. This follows
  Mermaid's Treemap CSS-provider default for `.treemapLabel` / `.treemapValue` while avoiding
  unreadable headless output.

Focused verification:

- `cargo fmt -p merman-render -p merman`
- `cargo fmt -p merman-render -p merman --check`
- `cargo nextest run -p merman-render --test treemap_svg_test`
- `cargo nextest run -p merman --features raster --test resvg_safe_fixture_smoke source_content_gate_distinguishes_accessibility_only_from_visible_content`
- `$env:MERMAN_RESVG_SAFE_AUDIT_FAMILY='treemap'; cargo nextest run -p merman --features raster --test resvg_safe_fixture_smoke --run-ignored ignored-only all_supported_fixtures_render_headless_resvg_safe_audit`
- `$env:MERMAN_RESVG_SAFE_AUDIT_FAMILY='gitgraph,kanban,timeline,journey'; cargo nextest run -p merman --features raster --test resvg_safe_fixture_smoke --run-ignored ignored-only all_supported_fixtures_render_headless_resvg_safe_audit`
- `$env:MERMAN_RESVG_SAFE_AUDIT_FAMILY='treemap,pie,quadrantchart,xychart'; cargo nextest run -p merman --features raster --test resvg_safe_fixture_smoke --run-ignored ignored-only all_supported_fixtures_render_headless_resvg_safe_audit`
- `$env:MERMAN_RESVG_SAFE_AUDIT_FAMILY='radar,requirement,packet,sankey,c4'; cargo nextest run -p merman --features raster --test resvg_safe_fixture_smoke --run-ignored ignored-only all_supported_fixtures_render_headless_resvg_safe_audit`
- `cargo run -p xtask -- compare-treemap-svgs --check-dom --dom-mode parity --dom-decimals 3 --filter upstream_pkgtests_treemap_test_032`

Residual note:

- The unfiltered multi-family raster audit can exceed a five-minute tool timeout. Use
  `MERMAN_RESVG_SAFE_AUDIT_FAMILY` slices for broad PNG-level triage and keep treating the ink check
  as a gross renderability gate, not a pixel-diff parity metric.

## HPD-080 - Raster Ink Directive-Only Calibration

Outcome:

- Continued the PNG-level `resvg_safe` audit after the single-leaf Treemap fix and found three
  source-content gate false positives rather than renderer blank-output defects:
  - State `classDef`-only fixtures populate the style-class registry but no nodes or edges.
  - State `state foo` plus floating note alias declarations are parser-only smoke cases in pinned
    Mermaid 11.15 and produce no visible state/node output in the stored upstream SVGs.
  - Flowchart `click X callback "X";` without a node definition records interaction metadata but
    produces no visible node or edge output.
- Tightened [crates/merman/tests/resvg_safe_fixture_smoke.rs](/F:/SourceCodes/Rust/merman/crates/merman/tests/resvg_safe_fixture_smoke.rs)
  so `classDef`, `click`, and `linkStyle` metadata lines do not by themselves require non-background
  raster ink.
- Added State-specific source-content calibration for bare `state <id>` declarations and floating
  note aliases while preserving visible State forms such as `state "Long state description" as S1`,
  `state fork_state <<fork>>`, and `foo: bar` plus note content.
- Kept Flowchart `style ...` lines visible. A style statement can materialize styled node ids in
  Mermaid/merman output, so it is not treated like `classDef` or `click` metadata.
- No renderer output was changed in this slice. The fix only prevents the raster audit from
  requiring ink for source files that Mermaid itself treats as non-visual parser/metadata cases.

Source evidence:

- `repo-ref/mermaid/packages/mermaid/src/diagrams/state/parser/state-style.spec.js` asserts
  `classDef` parsing by inspecting `StateDB.getClasses()` rather than renderer output.
- `repo-ref/mermaid/packages/mermaid/src/diagrams/state/stateDiagram.spec.js` and
  `stateDiagram-v2.spec.js` contain the floating-note samples as `parser.parse(...)` smoke cases.
- `fixtures/state/upstream_pkgtests_state_style_spec_012.mmd`,
  `fixtures/state/upstream_pkgtests_statediagram_spec_028.mmd`, and
  `fixtures/state/upstream_pkgtests_statediagram_v2_spec_031.mmd` have empty stored models/upstream
  SVGs for these non-visual inputs.
- `fixtures/flowchart/upstream_pkgtests_flow_spec_007.mmd` contains only
  `click X callback "X";` under `graph LR`; the stored model carries tooltip metadata without
  nodes/edges.

Focused verification:

- `cargo fmt --check -p merman`
- `cargo nextest run -p merman --features raster --test resvg_safe_fixture_smoke source_content_gate_distinguishes_accessibility_only_from_visible_content`
- `$env:MERMAN_RESVG_SAFE_AUDIT_FAMILY='gantt,mindmap,block'; cargo nextest run -p merman --features raster --test resvg_safe_fixture_smoke --run-ignored ignored-only all_supported_fixtures_render_headless_resvg_safe_audit`
- `$env:MERMAN_RESVG_SAFE_AUDIT_FAMILY='er'; cargo nextest run -p merman --features raster --test resvg_safe_fixture_smoke --run-ignored ignored-only all_supported_fixtures_render_headless_resvg_safe_audit`
- `$env:MERMAN_RESVG_SAFE_AUDIT_FAMILY='state'; cargo nextest run -p merman --features raster --test resvg_safe_fixture_smoke --run-ignored ignored-only all_supported_fixtures_render_headless_resvg_safe_audit`
- `$env:MERMAN_RESVG_SAFE_AUDIT_FAMILY='flowchart'; $env:MERMAN_RESVG_SAFE_AUDIT_FILTER='upstream_pkgtests_'; cargo nextest run -p merman --features raster --test resvg_safe_fixture_smoke --run-ignored ignored-only all_supported_fixtures_render_headless_resvg_safe_audit`
- Flowchart raster audit split coverage passed for `stress_flowchart`, `probe_flowchart`,
  `upstream_docs`, `upstream_html`, `upstream_pkgtests_`, `upstream_cypress_flowchart`,
  `upstream_cypress_newshapes`, `upstream_cypress_oldshapes`, `upstream_cypress_conf`,
  `upstream_cypress_theme`, `upstream_cypress_appli`, `upstream_flowchart`, `upstream_flow_`,
  `upstream_flowdb`, the remaining single-file `upstream_*` prefixes, and the local
  `basic` / `class_style` / `subgraph_click` / `zed_pr_57644_flowchart` fixtures.

Residual note:

- Unfiltered Flowchart raster audit remains large enough to be inefficient under ordinary tool
  timeouts, so the recorded gate is split-prefix coverage. This is a gross renderability gate:
  source-content calibration prevents false blank-output claims, while real contentful blank PNGs
  should still fail.

## HPD-080 - Boundary Resvg-Safe Renderability

Outcome:

- Added a separate public renderability smoke for `fixtures/error`, `fixtures/info`, and
  `fixtures/zenuml`.
- Kept these directories out of `SUPPORTED_FIXTURE_DIRS`. `info` uses shared info-like rendering,
  `error` is also a suppress-errors host entrypoint, and local `zenuml` is a documented headless
  Sequence-compatibility subset rather than full Mermaid browser-plugin parity.
- The new boundary smoke still reuses the same resvg-safe hazards as the main public smoke:
  XML parseability, no `foreignObject`, unsupported CSS cleanup, invalid visual token rejection,
  non-empty style elements, and optional PNG raster ink checks.
- The `error` fixture corpus runs through lenient parsing so invalid State samples exercise the
  host-visible suppressed error diagram. For those inputs, the raster ink sentinel is `error\n`
  because the original source may be parser-only while the generated error diagram must be visible.
- No production renderer defect was found in this slice; the change is a regression gate for
  boundary entrypoints that the implemented-family audit intentionally excludes.

Focused verification:

- `cargo fmt -p merman`
- `cargo nextest run -p merman --features render --test resvg_safe_fixture_smoke boundary_fixtures_render_headless_resvg_safe`
- `cargo nextest run -p merman --features raster --test resvg_safe_fixture_smoke boundary_fixtures_render_headless_resvg_safe`
- `cargo nextest run -p merman --features raster --test resvg_safe_fixture_smoke`

## HPD-080 - Flowchart Visible Edge Stroke Width

Outcome:

- Fixed a real Flowchart visible-DOM seam for ordinary edge thickness.
- Earlier Flowchart `strokeWidth` coverage updated the diagram-owned `.edgePath .path` provider
  rule, but current ordinary Flowchart paths do not carry the `.path` class. Their final visible
  stroke width is controlled by the shared `.edge-thickness-normal` rule.
- Pinned Mermaid 11.15 shared `styles.ts` sets `.edge-thickness-normal` to
  `themeVariables.strokeWidth ?? 1`, and `flowDb.ts` / the installed Mermaid 11.15 CLI output show
  ordinary Flowchart paths carry `edge-thickness-normal edge-pattern-solid flowchart-link`.
- Local Flowchart CSS now drives `.edge-thickness-normal` from the same `SvgTheme::css_value(
  "strokeWidth", "1")` source used by node and edge-path provider rules.
- Explicit `linkStyle` / edge inline style precedence is preserved: the themed class remains the
  default, while visible path `style="...stroke-width:...;..."` can still override it.

Touched production surfaces:

- [crates/merman-render/src/svg/parity/flowchart/css.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/flowchart/css.rs)
- [crates/merman-render/tests/flowchart_svg_test.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/tests/flowchart_svg_test.rs)
- [crates/merman/tests/theme_renderability_smoke.rs](/F:/SourceCodes/Rust/merman/crates/merman/tests/theme_renderability_smoke.rs)

Focused verification:

- `cargo fmt --check` - passed.
- `cargo nextest run -p merman-render --test flowchart_svg_test` - passed, `28` tests run.
- `cargo nextest run -p merman --features render --test theme_renderability_smoke` - passed, `10`
  tests run.
- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed, structural Flowchart DOM parity stayed green.
- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\flowchart_report_parity_root_hpd080_edge_strokewidth.md` -
  expected-failed on existing Flowchart max-width/root residual rows; the failures were root/style
  width mismatches, not new structural edge-style mismatches.
- `git diff --check` - passed.

Residual note:

- This slice fixes visible ordinary Flowchart edge stroke-width theming. It does not claim
  Flowchart root-bounds closure or broad Flowchart CSS parity beyond current source-backed DOM
  consumers.

## HPD-080 - Block Visible Edge Stroke Width

Outcome:

- Fixed the same visible edge-class seam for Block diagrams.
- Pinned Mermaid 11.15 shared `styles.ts` sets `.edge-thickness-normal` from
  `themeVariables.strokeWidth`, and Block `renderHelpers.ts` assigns visible edge paths
  `edge-thickness-normal edge-pattern-solid flowchart-link LS-a1 LE-b1`.
- Local Block CSS previously emitted only the diagram-owned `.edgePath .path` `2.0px` rule, but
  current visible Block edge paths do not carry `.path`. A focused Mermaid CLI render with
  `themeVariables.strokeWidth = 4` confirmed upstream final SVG has
  `.edge-thickness-normal{stroke-width:4px;}` on the matching visible edge class.
- Local Block CSS now emits the shared edge thickness and pattern rules, with normal edge
  thickness driven by `SvgTheme::css_value("strokeWidth", "1")`.
- Public dark-theme smoke now uses a Block sample with both composite cluster DOM and a visible
  edge, so it counts `strokeWidth` only through matching current edge DOM.

Touched production surfaces:

- [crates/merman-render/src/svg/parity/block.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/block.rs)
- [crates/merman-render/tests/block_svg_test.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/tests/block_svg_test.rs)
- [crates/merman/tests/theme_renderability_smoke.rs](/F:/SourceCodes/Rust/merman/crates/merman/tests/theme_renderability_smoke.rs)

Focused verification:

- `cargo nextest run -p merman-render --test block_svg_test` - passed, `4` tests run.
- `cargo run -p xtask -- compare-block-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed, structural Block DOM parity stayed green.
- `cargo nextest run -p merman --features render --test theme_renderability_smoke` - passed, `10`
  tests run.
- `cargo fmt --check` - passed.
- `git diff --check` - passed.

Residual note:

- This slice fixes visible ordinary Block edge stroke-width theming and shared edge pattern CSS. It
  does not change Block layout, node sizing, or cluster fade behavior.

## HPD-060 - Semantic / Render Unification Pilot

Outcome:

- Selected Sequence as the bounded pilot because it already had a typed render model and an obvious
  duplicate compatibility JSON construction path.
- Added `SequenceDiagramRenderModel::to_compat_json(...)` in
  [crates/merman-core/src/diagrams/sequence/render_model.rs](/F:/SourceCodes/Rust/merman/crates/merman-core/src/diagrams/sequence/render_model.rs)
  as the compatibility adapter from typed model to legacy JSON shape.
- Replaced the manual `SequenceDb::into_model(...)` JSON builder in
  [crates/merman-core/src/diagrams/sequence/db.rs](/F:/SourceCodes/Rust/merman/crates/merman-core/src/diagrams/sequence/db.rs)
  with `self.into_render_model().to_compat_json(...)`.
- Kept compatibility JSON field semantics explicit:
  - top-level `type` and `constants.placement` are still added by the adapter,
  - ordinary message `placement` is omitted when absent,
  - `centralConnection` is omitted when zero,
  - `from` / `to` still serialize as nullable compatibility fields.
- Expanded the typed-vs-JSON parse test in
  [crates/merman-core/src/tests/misc.rs](/F:/SourceCodes/Rust/merman/crates/merman-core/src/tests/misc.rs)
  to cover actor order, messages, notes, boxes, create/destroy actor indexes, and omitted optional
  message fields.

Focused verification:

- `cargo fmt --all`
- `cargo test -p merman-core parse_sequence_render_model_uses_typed_variant_without_changing_json_parse --lib`
- `cargo test -p merman-core sequence --lib`
- `cargo test -p merman-core --lib`
- `cargo test -p merman-render sequence_long_leftof_notes_keep_mermaid_11_15_note_width --test sequence_svg_test`
- `cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\sequence_report_parity_after_hpd060_typed_projection.md`
- `cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\sequence_report_parity_root_after_hpd060_typed_projection.md`

Verification notes:

- Sequence structural parity remained green: the post-HPD-060 `parity` report says all fixtures
  matched.
- The Sequence `parity-root` report remains an expected failure with `28` dom mismatches. The top
  rows are existing measurement/root residuals, led by long left-of wrapped notes at `+19px` and
  math/line-break rows.
- Later default-CI cleanup moved these known Sequence measurement tails out of the default red path:
  `sequence_note_width_expands_for_literal_br_backslash_t_in_vendored_mode` now allows the narrow
  deterministic `151..=152px` band, and
  `sequence_long_leftof_notes_keep_mermaid_11_15_root_width` is ignored as the documented long-note
  root-width residual (`570px` deterministic local vs. `566px` upstream).
- This pilot does not claim repo-wide semantic/render unification. It proves the narrower pattern:
  use one typed semantic source, then project compatibility JSON as an adapter instead of keeping a
  second parser-owned JSON master.

## HPD-070 - Unsupported-Family Rubric

Outcome:

- Added
  [docs/alignment/UNSUPPORTED_FAMILY_ADMISSION_RUBRIC.md](/F:/SourceCodes/Rust/merman/docs/alignment/UNSUPPORTED_FAMILY_ADMISSION_RUBRIC.md)
  as the durable admission policy for unsupported Mermaid families.
- Updated the Mermaid 11.15 unsupported-family table in
  [docs/alignment/STATUS.md](/F:/SourceCodes/Rust/merman/docs/alignment/STATUS.md) so it uses the
  locked Mermaid source commit rather than the current `repo-ref/mermaid` working tree when those
  diverge.
- Classified pinned Mermaid 11.15 unsupported families in priority order:
  1. `treeView-beta` header / `treeView` id
  2. `ishikawa` / `ishikawa-beta`
  3. `eventmodeling`
  4. `venn-beta`
  5. `wardley-beta`
- Marked `railroad-*` and `cynefin-beta` as not part of the Mermaid 11.15 parity backlog because
  they are absent from the pinned `41646dfd...` source tree.

Source evidence:

- `git -C repo-ref/mermaid ls-tree --name-only 41646dfd43ac83f001b03c70605feb036afae46d:packages/mermaid/src/diagrams`
  listed `eventmodeling`, `ishikawa`, `treeView`, `venn`, and `wardley`, but not `railroad` or
  `cynefin`.
- Pinned `diagram-orchestration.ts` registers lazy detector ids for `eventmodeling`, `treeView`,
  `ishikawa`, `venn`, and `wardley`. The `treeView` detector accepts the `treeView-beta` header.
- Pinned source shape used for prioritization:
  - `treeView`: parser `16` lines, DB `69`, renderer `114`
  - `ishikawa`: parser `45`, DB `79`, renderer `468`
  - `eventmodeling`: parser `25`, DB `602`, renderer `138`
  - `venn`: parser `110`, DB `116`, renderer `336`, plus `@upsetjs/venn.js`
  - `wardley`: parser `218`, DB `138`, renderer `971`, plus `WardleyBuilder`

Verification:

- `git -C repo-ref/mermaid ls-tree ...` and pinned-source `git show ...` checks listed in the
  journal.
- `rg --files fixtures docs | rg "treeView|treeview|venn|ishikawa|eventmodeling|wardley|railroad|cynefin"`
  found no local fixtures/docs besides the new rubric/status updates, confirming these families are
  not already partially admitted locally.
- JSONL validation for `CONTEXT.jsonl`, `TASKS.jsonl`, and `CAMPAIGNS.jsonl`.

Review notes:

- `treeView` is the recommended first new-family workstream only when new-family implementation is
  actually approved. HPD-070 does not start that work.
- `venn` should not be implemented with a guessed local circle layout. It needs either a port or
  source-backed audit of `@upsetjs/venn.js`.
- `railroad-*` and `cynefin-beta` may exist in newer Mermaid development branches, but they are not
  part of the current pinned 11.15 scope.

## HPD-080 - Flowchart Renderability Audit

Outcome:

- Audited current HEAD for a fresh Flowchart visible/renderability defect.
- No new defect was found, so no production code or tests were changed.
- Existing Flowchart dark-theme smoke still proves DOM-consumed visible theme signals, including
  the visible `.edge-thickness-normal` stroke-width rule and matching current edge-path DOM.
- Recommendation: return this slice's remaining budget to HPD-050 Architecture until a new failing
  renderability gate, source-backed emitted-surface gap, or concrete consumer report appears.

Evidence written:

- [docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-04-hpd-080-flowchart-renderability-audit.md](/F:/SourceCodes/Rust/merman/docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-04-hpd-080-flowchart-renderability-audit.md)

Focused verification:

- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render --test theme_renderability_smoke` -
  passed, `12` tests run.
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render --test resvg_safe_fixture_smoke` -
  passed, `5` tests run and `1` skipped.
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-render --test flowchart_svg_test` -
  passed, `29` tests run.
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed.
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo run -p xtask -- report-overrides --check-no-growth` -
  passed; override growth and root viewport usage checks were both ok.
- `cargo fmt --check` - passed.

Gate notes:

- Full ignored all-supported Flowchart raster audit was not rerun. This slice was scoped to fresh
  visible-defect triage, and the public theme smoke, representative `resvg_safe` smoke, Flowchart
  renderer tests, structural compare, and override-growth check all passed.
- Known Flowchart root/max-width residuals remain outside this renderability slice. No layout,
  root viewport, baseline, or override-pin changes were made.

## 2026-06-05 - Main Closeout Nextest Recheck

Fresh main-worktree evidence from 2026-06-05 00:57 +08:00:

- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run --no-fail-fast`:
  passed on current `main`, run id `d9fcbd92-73cb-4ba7-8eb3-91118fa8c7fd`.
- Nextest summary: `1857` tests run, `1857` passed, `5` skipped.

Scope:

- This recheck covers the integrated root-report coverage work, the Class/Sequence measurement
  no-growth cleanup, the HPD-080 Flowchart renderability audit docs, and the HPD-050 Architecture
  FCoSE residual reclassification docs now present on `main`.

Outcome:

- No broad test regression was found after the integrated evidence/no-growth slices.
- Continue from HPD-050 Architecture/FCoSE source-backed audit work when no fresh visible
  renderability defect is active.

## HPD-050 - Architecture FCoSE Geometry Epsilon

Outcome:

- Closed the active `stress_architecture_batch6_junctions_multi_split_with_group_edges_087`
  Architecture root residual by fixing FCoSE compound-repulsion boundary geometry.
- The source-backed diagnosis found missing second-run displacement in compound repulsion around
  near-equal overlap centers and rectangle-boundary floating-point drift. This was later narrowed by
  the strict-intersects slice below: `rects_intersect(...)` keeps source-strict positive-gap
  semantics, while near-equal center/direction comparisons retain the epsilon guard.
- No random-seed offset change, root override, stored SVG baseline, or browser metric constant was
  kept.

Focused verification:

- `cargo nextest run -p manatee fcose` - passed, `10` tests run.
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_batch6_junctions_multi_split_with_group_edges_087 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_batch6_junctions_hpd050_fcose_geometry_epsilon.md` -
  passed with upstream/local `max-width: 653.184px`.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target/compare/architecture_report_parity_hpd050_fcose_geometry_epsilon.md` -
  passed.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_report_parity_root_hpd050_fcose_geometry_epsilon.md` -
  expected failure with `24` Architecture root mismatch rows; the fixed batch6 row is root-exact
  at `+0.000px`.

Gate notes:

- Remaining Architecture `parity-root` failures are led by the existing `+5px`
  long-title/HTML-title tails and remain HPD-050 follow-up material.
- Current code no longer treats tiny positive rectangle gaps as intersection; see
  `HPD-050 - Architecture FCoSE Strict Rectangle Intersects` below for the narrowed source rule.

## HPD-050 - Architecture Long Group Title Small Residuals

Outcome:

- Classified two smaller Architecture long-group-title residuals after the FCoSE geometry epsilon
  fix without changing production code.
- `stress_architecture_batch3_long_group_titles_wrapping_055` is not a group-title root-bounds
  target. Local service positions match upstream exactly; the row's `-1px` group/root width tail
  comes from Cytoscape child label/bbox phase cancellation: browser child labels are `3px` wider
  and `2px` taller than the local contribution label phase, while browser final expansion is `83px`
  versus local emitted expansion `85px`.
- `stress_architecture_batch6_long_group_titles_wrapping_extreme_095` has exact services and group
  rects. Its remaining `-0.468750px` root-width tail is in group-title SVG text root-bounds
  estimation, where upstream relies on Chromium `getBBox()` after `createText(...)`.

Focused verification:

- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_batch3_long_group_titles_wrapping_055 --out target\compare\architecture-delta-batch3-long-group-title-hpd050` -
  passed.
- `cargo run -p xtask -- debug-architecture-fcose-probe --fixture stress_architecture_batch3_long_group_titles_wrapping_055 --out-dir target\compare\architecture-fcose-probe-batch3-long-group-title-hpd050 --browser-exe "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"` -
  passed from the main workspace Mermaid CLI installation, writing artifacts into this worktree's
  `target\compare`.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_batch3_long_group_titles_wrapping_055 --probe-dir target\compare\architecture-fcose-probe-batch3-long-group-title-hpd050 --out target\compare\architecture-delta-batch3-long-group-title-probe-join-hpd050` -
  passed.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_batch6_long_group_titles_wrapping_extreme_095 --out target\compare\architecture-delta-long-group-title-small-residuals-hpd050` -
  passed.

Gate notes:

- A production fix for the `batch6_long...095` class needs a reusable group-title SVG text
  `getBBox()` rule across fixtures. Do not add a single-title constant or Architecture root pin.

## HPD-050 - Architecture Multiline Group Title Root Bounds

Outcome:

- Landed the narrow reusable rule identified by the previous small-residual classification:
  wrapped Architecture group titles now round each measured SVG title row width up to an integer
  pixel boundary only when the title emits multiple outer `tspan` rows.
- The rule is scoped to synthetic root content-bounds union for group titles. It does not change
  one-line group titles, service labels, child contribution bounds, group rectangles, FCoSE inputs,
  root overrides, or stored SVG baselines.
- `stress_architecture_batch6_long_group_titles_wrapping_extreme_095` is root-exact after the
  change. `stress_architecture_long_group_titles_018` remains a separate existing residual because
  its title is one outer row and its group/service geometry still drifts.

Focused verification:

- `cargo fmt --check` - passed.
- `git diff --check` - passed.
- `cargo nextest run -p merman-render architecture` - passed, `31` tests run.
- `cargo run -p xtask -- report-overrides --check-no-growth` - passed; Architecture root
  overrides remain at `0`, and override-growth/root-usage checks are ok.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed across the implemented matrix.
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_batch6_long_group_titles_wrapping_extreme_095 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --no-root-overrides --out target\compare\arch-focused-095-ceil` -
  passed with upstream/local `max-width: 533.000px`.
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_long_group_titles_018 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --no-root-overrides --out target\compare\arch-focused-018-ceil` -
  expected-failed with the existing `+0.656px` root-width tail.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --no-root-overrides --out target\compare\architecture-report-parity-root-multiline-title-ceil-final` -
  expected-failed with `23` Architecture root/style mismatches, `28` non-zero root delta rows, and
  absolute root-width residual sum `27.596px`. The prior Architecture queue was `24` mismatches,
  `29` non-zero root delta rows, and about `28.065px` absolute residual sum.

Gate notes:

- This is not a root pin or fixture constant. It closes the multi-line group-title SVG
  `getBBox()` lattice tail represented by `095` and leaves the remaining service child
  contribution / Cytoscape bbox phase residuals open.

## HPD-050 - Architecture FCoSE Strict Rectangle Intersects

Outcome:

- Revalidated `stress_architecture_group_port_edges_017` after the multiline group-title fix and
  confirmed it had reappeared as a current root residual: upstream/local max-width was
  `707.769226px` / `709.237549px`, with local root height `17.845154px` shorter.
- The current force debug matched the earlier source audit: the second-run `inner` compound was
  taking the overlap repulsion branch (`rep=(40,40)`) instead of the upstream positive-gap clipping
  branch (`rep=(0,250)`).
- Restored `rects_intersect(...)` to source-strict layout-base semantics: touching edges intersect,
  but a positive gap remains non-intersecting. The `GEOMETRY_EPSILON` guard remains only for
  near-equal center/direction comparisons, preserving the `087` closure.
- Added/updated tests for source-strict `RectangleD.intersects(...)` and the `group_port_edges_017`
  positive-gap clipping branch. No root pin, fixture special case, baseline refresh, group padding
  tweak, or broad solver rewrite was added.

Focused verification:

- `cargo nextest run -p manatee -E 'test(rects_intersect_keeps_positive_touch_gap_separate) or test(overlap_separation_treats_nearly_equal_centers_as_equal) or test(constraint_handler_preserves_group_port_second_run_tiny_gap)'` -
  passed, `3` tests run.
- `cargo nextest run -p manatee fcose` - passed, `10` tests run.
- `cargo nextest run -p manatee` - passed, `14` tests run.
- `cargo nextest run -p merman-render architecture` - passed, `31` tests run.
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_group_port_edges_017 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --no-root-overrides --out target\compare\arch-focused-017-strict-intersect-final` -
  passed.
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_batch6_junctions_multi_split_with_group_edges_087 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --no-root-overrides --out target\compare\arch-focused-087-strict-intersect-final` -
  passed.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture-report-parity-strict-intersect-final` -
  passed.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --no-root-overrides --out target\compare\architecture-report-parity-root-strict-intersect-final` -
  expected-failed with `20` Architecture root/style mismatch rows. `group_port_edges_017` is
  root-exact, and `087` remains outside the mismatch list.
- `cargo run -p xtask -- report-overrides --check-no-growth` - passed.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed across the implemented matrix.
- `cargo fmt --check` - passed.
- `git diff --check` - passed.

Gate notes:

- The post-095 Architecture root queue improves from `23` mismatch rows to `20` mismatch rows.
- Do not reintroduce a global rectangle-intersection epsilon. The current source boundary is strict
  positive-gap handling plus narrowly tested near-equal-center handling for overlap direction.

## HPD-050 - Architecture Group Padding 1.5 Rejection

Outcome:

- Tested a temporary global reduction of final Architecture SVG group bbox expansion from
  `padding + 2.5px` to `padding + 1.5px` after the strict-intersects fix.
- The focused direct-width tails improved only by the expected expansion component:
  `batch5_long_titles_and_punct_076` `+5 -> +3`, `html_titles_and_escapes_041` `+5 -> +3`, and
  `unicode_and_xml_escapes_019` `+3 -> +1`.
- The same focused rows gained a `-2px` viewBox height delta, matching the earlier service-phase
  join evidence that local child content is already `-2px` short in height and the existing
  `+2px` final expansion cancels it.
- Full Architecture `parity-root` regressed from the post-strict `20` mismatch rows to `105`
  mismatch rows, so the experiment was rejected and reverted.

Evidence:

- `target/compare/architecture-delta-direct-width-tails-current-hpd050` - current focused direct
  width baseline.
- `target/compare/architecture-delta-direct-width-tails-pad15-experiment-hpd050` - focused
  `padding + 1.5px` experiment; width deltas `+3`, `+3`, `+1`; height deltas `-2` for all three
  rows.
- `target/compare/architecture-report-parity-root-pad15-experiment-hpd050` - full rejected
  experiment report with `105` Architecture root/style mismatches.
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-05-hpd-050-architecture-pad15-experiment-rejected.md`
  - journal and residual boundary.

Gate notes:

- Do not globally change Architecture group padding, final group bbox extra, root padding, or final
  group rect emission to chase the direct-width tails.
- Continue from service label/content contribution geometry and browser final service
  `node.boundingBox()` differences; any future production change must survive full Architecture
  `parity-root` instead of just improving these three rows.

## HPD-050 - Architecture Post-Strict Root Queue Classification

Outcome:

- Regenerated local `debug-architecture-delta` reports for all `20` post-strict Architecture
  `parity-root` mismatch rows.
- Joined the current top five residuals (`076`, `041`, `019`, `093`, and `002`) with the existing
  browser/Cytoscape label-contribution probe batch.
- Classified the queue into separate residual families:
  - direct group-width tails: `076` / `041` / `019`, still split as child content `+3/+3/+1` plus
    final expansion `+2`;
  - source-shaped service/body/final phase rows: `093`, where child content is negative-width
    relative to browser and service/group X displacement is large;
  - nested/group aggregate rows: `002`, controlled by child-group and service aggregate placement;
  - small group-rect/root lattice tails around `1px`, `0.5px`, and `0.25px`;
  - top-level service icon/text root-bounds tails with no group rects.

Evidence:

- `target/compare/architecture-delta-post-strict-20-hpd050`
- `target/compare/architecture-delta-post-strict-probe-join-top5-hpd050`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-05-hpd-050-architecture-post-strict-root-queue-classification.md`

Focused verification:

- `cargo run -p xtask -- debug-architecture-delta ... --out target\compare\architecture-delta-post-strict-20-hpd050` -
  passed for all `20` post-strict mismatch rows.
- `cargo run -p xtask -- debug-architecture-delta ... --probe-dir F:\SourceCodes\Rust\merman\target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050 --out target\compare\architecture-delta-post-strict-probe-join-top5-hpd050` -
  passed for the current top five residual rows.

Gate notes:

- No production code changed.
- The next experiment must include both positive-content rows (`076` / `041` / `019`) and
  negative-content or nested rows (`093` / `002`). A candidate that improves only the three
  direct-width rows is not a valid HPD-050 production fix.

## HPD-050 - Architecture Child Group Parent-Input Diagnostics

Outcome:

- Added a `debug-architecture-delta` probe-join table for nested child-group parent input.
- The table compares browser child group final `node.boundingBox()` values with local emitted
  child group rects and the local renderer's `1px` inset parent-input rect.
- Re-ran the top five post-strict Architecture rows. The direct-width rows (`076` / `041` / `019`)
  and `093` have no child-group rows, while `002` now exposes its nested parent-input phase:
  raw `platform -> data/runtime` child widths are `-0.5px` / `0px`, but the parent-input rects are
  `-2.5px` / `-2px` wide and `-2px` tall after the current inset.

Evidence:

- `target/compare/architecture-delta-child-group-parent-input-hpd050`
- `target/compare/architecture-report-parity-child-group-parent-input-hpd050`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-05-hpd-050-architecture-child-group-parent-input-diagnostics.md`

Focused verification:

- `cargo fmt --check` - passed.
- `cargo nextest run -p xtask -E 'test(architecture_probe_join_reports_nested_group_aggregate_content) or test(architecture_probe_join_decomposes_group_and_service_bounds)'` - passed, `2` tests run.
- `cargo nextest run -p xtask` - passed, `112` tests run.
- `git diff --check` - passed.
- `cargo run -p xtask -- report-overrides --check-no-growth` - passed; Architecture root
  overrides remain at `0`.
- `cargo run -p xtask -- debug-architecture-delta ... --out target\compare\architecture-delta-child-group-parent-input-hpd050` - passed for the top five post-strict rows.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture-report-parity-child-group-parent-input-hpd050` - passed.

Gate notes:

- No production renderer behavior changed.
- `002` should remain classified as nested child-group / parent-input phase evidence. Do not fold
  it into the direct service label-width family or use it to justify a global child-group inset
  retune without full Architecture root verification.

## HPD-050 - Architecture Final Node Edge Owner Diagnostics

Outcome:

- Added a `debug-architecture-delta` probe-join table for final node edge ownership.
- The table compares browser final node `bb` edge owners with local final-frame service bboxes plus
  emitted group rects, reporting X/Y min/max owners and span deltas.
- Re-ran the top five post-strict Architecture rows. The direct width rows are final group-edge
  owned (`076` `+5px`, `041` `+5px`, `019` `+3px`), and `093` is also final group-edge owned with
  an X span delta of `-2.5px`. `002` remains a nested frame-mismatch row: the final-node table
  shows a `+42.5px` X span delta while the SVG root delta is only `+2.5px`, so it still needs
  render-path/source-frame evidence before any production change.

Evidence:

- `target/compare/architecture-delta-final-node-edge-owner-hpd050`
- `target/compare/architecture-report-parity-final-node-edge-owner-hpd050`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-05-hpd-050-architecture-final-node-edge-owner-diagnostics.md`

Focused verification:

- `cargo fmt --check` - passed.
- `cargo nextest run -p xtask -E 'test(architecture_probe_join_reports_nested_group_aggregate_content) or test(architecture_probe_join_decomposes_group_and_service_bounds)'` - passed, `2` tests run.
- `cargo nextest run -p xtask` - passed, `112` tests run.
- `git diff --check` - passed.
- `cargo run -p xtask -- report-overrides --check-no-growth` - passed; Architecture root
  overrides remain at `0`.
- `cargo run -p xtask -- debug-architecture-delta ... --out target\compare\architecture-delta-final-node-edge-owner-hpd050` - passed for the top five post-strict rows.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture-report-parity-final-node-edge-owner-hpd050` - passed.

Gate notes:

- No production renderer behavior changed.
- The final-node owner table is evidence for boundary ownership and frame alignment, not a
  root-width formula. Use it to route direct final group-edge rows separately from nested frame
  rows.

## HPD-050 - Architecture Relocate Center Diagnostics

Outcome:

- Added a render-path join table for bundled/browser versus local
  `relocateComponent.before-shift` inputs.
- The table reports per-run `rectBbox`, `originalCenter`, `rectCenter`, `delta`, and local-minus-
  bundled deltas, making the post-FCoSE component translation visible before same-stage group/node
  comparisons.
- Re-ran `002` and `093` with `MANATEE_FCOSE_DEBUG_TRACE=1` so local relocate trace values were
  present. For `002`, run 1 has matching `rectBbox`/`rectCenter` but local `originalCenter.x` and
  `delta.x` are both `+1.250000px`. For `093`, run 1 has matching `rectBbox`/`rectCenter` but
  local `originalCenter.x` and `delta.x` are both `+22.963987px`.

Evidence:

- `target/compare/architecture-delta-relocate-table-check-trace-hpd050`
- `target/compare/architecture-report-parity-relocate-table-hpd050`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-05-hpd-050-architecture-relocate-center-diagnostics.md`

Focused verification:

- `cargo fmt --check` - passed.
- `cargo nextest run -p xtask architecture_render_path_join_reports_local_deltas` - passed, `1`
  test run.
- `cargo nextest run -p xtask` - passed, `112` tests run.
- `cargo run -p xtask -- report-overrides --check-no-growth` - passed; Architecture root
  overrides remain at `0`.
- `cargo run -p xtask -- debug-architecture-delta ... --out target\compare\architecture-delta-relocate-table-check-trace-hpd050`
  with `MANATEE_FCOSE_DEBUG_TRACE=1` - passed for `002` and `093`.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture-report-parity-relocate-table-hpd050` -
  passed.

Gate notes:

- No production renderer behavior changed.
- The local relocate columns require `MANATEE_FCOSE_DEBUG_TRACE=1`; without that trace, the table is
  still structurally present but local values are `<none>`.
- The table isolates post-FCoSE translation input drift. It does not by itself explain all residual
  width, so continue through group-bounds consumption and SVG/root emission before any production
  fix.

## HPD-050 - Architecture Edge Curve Style Relocate Fix

Outcome:

- Added `IndexedEdge::curve_style_segments` so local FCoSE relocation bbox logic distinguishes
  real Cytoscape `edge.segments` edges from ordinary diagonal `straight` edges.
- Architecture now derives that flag from Mermaid's direction-pair rule: one horizontal direction
  and one vertical direction means `edge.segments`; same-axis edges remain `straight`.
- Local run > 0 `eles.boundingBox()` now places labels for straight diagonal edges at the straight
  midpoint instead of the orthogonal bend.
- Architecture FCoSE edge-label measurement now uses Cytoscape's default edge-label text style,
  matching Mermaid's stylesheet where `font-size` is set on `node[label]`, not `edge[label]`.
- Focused `093` evidence confirms the run 1 `api-db` label center moved from the local bend
  approximation to the browser midpoint: `x=82.037349 y=-66.880297` became
  `x=40.000000 y=12.407124`.
- The bundled/local run 1 relocate `originalCenter.x` drift for `093` dropped from
  `+22.963987px` to `+1.230469px`; `002` remains at `+1.250000px`.
- The remaining `002` / `093` root-width residuals are still `2.5px`, so this is a source-backed
  input-model fix, not full root closure.

Evidence:

- `target/compare/architecture-delta-segment-style-fix-hpd050`
- `target/compare/architecture-report-parity-segment-style-fix-hpd050`
- `target/compare/architecture-report-parity-root-segment-style-fix-hpd050`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-05-hpd-050-architecture-edge-curve-style-relocate-fix.md`

Focused verification:

- `cargo fmt --check` - passed.
- `cargo nextest run -p manatee` - passed, `15` tests run.
- `cargo nextest run -p merman-render architecture` - passed, `32` tests run.
- `cargo nextest run -p xtask` - passed, `112` tests run.
- `cargo run -p xtask -- report-overrides --check-no-growth` - passed; Architecture root
  overrides remain at `0`.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture-report-parity-segment-style-fix-hpd050` -
  passed.
- `cargo run -p xtask -- debug-architecture-delta ... --out target\compare\architecture-delta-segment-style-fix-hpd050`
  with `MANATEE_FCOSE_DEBUG_TRACE=1` and `MANATEE_FCOSE_DEBUG_ELES_BBOX=1` - passed for `002`
  and `093`.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --no-root-overrides --out target\compare\architecture-report-parity-root-segment-style-fix-hpd050` -
  expected-failed with the active `20` Architecture root mismatch rows.

Gate notes:

- This fix removes the large `093` relocate-origin family error without adding root overrides,
  root pins, group padding changes, or baseline refreshes.
- Continue the remaining `2.5px` tails through final group/root bbox modeling. For `093`, the next
  evidence is direct group-width deltas (`left=-3px`, `right=-1px`); for `002`, the row remains a
  nested frame/root-width sensor.

## HPD-050 - Architecture Root Tail Edge Attribution

Outcome:

- Added a `Root viewport edge attribution` table to `debug-architecture-delta` render-path joins.
- The table compares render-path/local SVG root viewBox min/max edges with the group/service owner
  edge that drives each side before root padding.
- Service contributors are expanded from SVG service positions using local service body dimensions,
  so top-level service-owned root edges are visible beside group-owned root edges.
- Regenerated the current `002` / `093` joint report after the edge curve-style fix.
- `093` now decomposes to `group-left` root-left delta `+2.730461px` and `group-right` root-right
  delta `+0.230461px`, producing the remaining `-2.5px` width tail.
- `002` decomposes to `service-ingress` root-left delta `+1.250000px` and `group-platform`
  root-right delta `+3.750000px`, producing the remaining `+2.5px` width tail.
- Root padding stayed stable on both sides (`~30px` for `093`, `~40px` for `002`), so this
  evidence rejects another root-padding constant change.

Evidence:

- `target/compare/architecture-delta-root-tail-attribution-002-093-hpd050`
- `target/compare/architecture-report-parity-root-tail-attribution-hpd050`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-05-hpd-050-architecture-root-tail-edge-attribution.md`
- `crates/xtask/src/cmd/debug/architecture.rs`

Focused verification:

- `cargo fmt --check` - passed.
- `cargo nextest run -p xtask -E 'test(architecture_render_path_join_reports_local_deltas) or test(architecture_probe_join_decomposes_group_and_service_bounds)'` -
  passed, `2` tests run.
- `cargo nextest run -p xtask` - passed, `112` tests run.
- `cargo run -p xtask -- report-overrides --check-no-growth` - passed; Architecture root
  overrides remain at `0`.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture-report-parity-root-tail-attribution-hpd050` -
  passed.
- `MANATEE_FCOSE_DEBUG_TRACE=1 MANATEE_FCOSE_DEBUG_ELES_BBOX=1 cargo run -p xtask -- debug-architecture-delta ... --out target\compare\architecture-delta-root-tail-attribution-002-093-hpd050` -
  passed for `002` and `093`.

Gate notes:

- This is an evidence-surface improvement only. It does not change Architecture rendering, layout,
  root overrides, or baselines.
- The next production-capable seam must explain why the owning final SVG edges differ; this table
  only removes ambiguity about which edges own the remaining root tails.

## HPD-050 - Architecture Small Root Tail Precision

Outcome:

- Re-read the current `002` / `093` render-path probe and root-tail attribution reports after the
  edge curve-style fix landed on `main`.
- Confirmed both browser render-path probes match the stored upstream SVG facts, so the focused
  evidence is not relying on a stale manual FCoSE reconstruction.
- Classified `093` as a small final group-edge owner tail:
  - `group-left` root-left owner delta is `+2.730461px`;
  - `group-right` root-right owner delta is `+0.230461px`;
  - the owner-span/root-width tail is therefore `-2.500000px`;
  - local group SVG widths are `-3px` for `left` and `-1px` for `right`.
- Classified `002` as a mixed top-level service plus parent-group root edge tail:
  - `service-ingress` root-left owner delta is `+1.250000px`;
  - `group-platform` root-right owner delta is `+3.750000px`;
  - the owner-span/root-width tail is therefore `+2.500000px`;
  - nested child-group parent-input evidence remains active for `platform`.
- Root padding is unchanged in both rows (`~30px` for `093`, `~40px` for `002`), so the focused
  precision pass continues to reject root-padding fixes.
- A temporary Cytoscape node-label font-family experiment also left both focused deltas unchanged
  (`093=-2.500`, `002=+2.500`) and was reverted.

Evidence:

- `target/compare/architecture-render-path-source-frame-002-093-main-hpd050`
- `target/compare/architecture-delta-root-tail-attribution-002-093-main-hpd050`
- `target/compare/architecture-delta-cy-node-default-family-experiment-hpd050`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-05-hpd-050-architecture-small-root-tail-precision.md`

Focused verification:

- Current worktree inspection confirmed no production change from the reverted font-family
  experiment.
- The no-output-change experiment's focused batch report kept `002` and `093` at the same
  `2.5px` residuals.
- `git diff --check` - passed.
- `cargo fmt --check` - passed.
- `cargo run -p xtask -- report-overrides --check-no-growth` - passed; Architecture root
  overrides remain at `0`.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed across the implemented matrix.

Gate notes:

- No renderer, layout, fixture, baseline, or root override behavior changed in this pass.
- The `002` / `093` precision is now good enough to stop chasing them with global constants. Keep
  them as small diagnostic tails unless a broader service-label/final-bbox model survives full
  Architecture verification.
- The next HPD-050 production-capable target should return to the larger direct service
  label/content rows `076`, `041`, and `019`; any candidate must keep the now-small `002` / `093`
  tails stable.

## HPD-050 - Architecture Direct Service Tail Render-Path Revalidation

Outcome:

- Regenerated actual `mermaid.render(...)` render-path probes for the direct service label/content
  residual rows `076`, `041`, and `019` on current `main`.
- All three render-path probes reported `facts match: true`, so their browser facts match the
  stored upstream SVGs.
- Joined those render-path facts with the existing FCoSE label-contribution probe batch and current
  local SVG delta reports.
- Current focused deltas remain `076=+5px`, `041=+5px`, and `019=+3px`.
- The same source split still holds:
  - `076`: service content `+3px` plus final expansion `+2px`;
  - `041`: service content `+3px` plus final expansion `+2px`;
  - `019`: service content `+1px` plus final expansion `+2px`.
- Boundary service attribution remains service-label/content owned:
  - `076/pipeline`: `storage` left `-2.5px`, `registry` right `+0.5px`, edge width `+3px`;
  - `041/ui`: `web` left `-0.5px`, `origin` right `+2.5px`, edge width `+3px`;
  - `019/i`: `metrics` left `-3.5px`, `store` right `-2.5px`, edge width `+1px`.
- This revalidation does not reopen exact `labelWidth` lookup as a standalone production seam: the
  existing rejected experiment reduced focused widths only to `+2px`, raised full Architecture
  root mismatches to `25`, and shifted `093` to `-8px`.

Evidence:

- `target/compare/architecture-render-path-direct-service-tails-main-hpd050`
- `target/compare/architecture-delta-direct-service-tail-render-path-main-hpd050`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-05-hpd-050-architecture-direct-service-tail-render-path.md`

Focused verification:

- `cargo run -p xtask -- debug-architecture-render-path-probe --fixture stress_architecture_batch5_long_titles_and_punct_076 --fixture stress_architecture_html_titles_and_escapes_041 --fixture stress_architecture_unicode_and_xml_escapes_019 --browser-exe "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe" --out target\compare\architecture-render-path-direct-service-tails-main-hpd050` -
  passed; all three fixtures reported `facts match: true`.
- `MANATEE_FCOSE_DEBUG_TRACE=1 MANATEE_FCOSE_DEBUG_ELES_BBOX=1 cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_batch5_long_titles_and_punct_076 --fixture stress_architecture_html_titles_and_escapes_041 --fixture stress_architecture_unicode_and_xml_escapes_019 --probe-dir target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050 --render-probe-dir target\compare\architecture-render-path-direct-service-tails-main-hpd050 --out target\compare\architecture-delta-direct-service-tail-render-path-main-hpd050` -
  passed and wrote the joined current-HEAD reports.
- `git diff --check` - passed.
- `cargo run -p xtask -- report-overrides --check-no-growth` - passed; Architecture root
  overrides remain at `0`.

Gate notes:

- No renderer, layout, fixture, baseline, or root override behavior changed in this pass.
- The next production candidate must handle service child-label contribution, final group
  expansion, and root SVG consumption together across both axes. Improving only `076` / `041` /
  `019` width remains insufficient.

## HPD-050 - Architecture Top-Service Icon Root-Bounds Audit

Outcome:

- Revalidated the current Architecture `parity-root` queue on current `main`; the gate remains an
  expected diagnostic failure with `20` root/style mismatch rows.
- Regenerated actual `mermaid.render(...)` render-path probes for the remaining top-level
  service/icon rows:
  - `stress_architecture_external_icons_005`;
  - `upstream_architecture_cypress_fallback_icon`;
  - `upstream_cypress_architecture_spec_should_render_an_architecture_diagram_with_the_fallback_icon_004`;
  - `upstream_html_demos_architecture_default_icon_from_unknown_icon_name_003`;
  - `upstream_html_demos_architecture_external_icons_demo_012`.
- All five render-path probes reported `facts match: true`, so the probe facts match the stored
  upstream SVGs.
- Joined those render-path facts with local SVG deltas using the root-edge attribution table.
- The three fallback/default single-service icon rows are not layout-width defects:
  - the root X owners are `service-unknown@0` and `service-unknown@80` on both sides;
  - owner X deltas are `0`;
  - the `-0.273438px` width residual comes from asymmetric root padding / text-bbox lattice
    differences (`49.851562/50.101562` upstream vs. `49.839844/49.839844` local).
- `upstream_html_demos_architecture_external_icons_demo_012` is a no-group service-position
  lattice row:
  - all four service SVG positions are shifted by `dx=-0.5`, `dy=-1.0`;
  - root X owners remain `service-fa` and `service-s3`;
  - the remaining `+0.523438px` width residual is root-padding lattice on top of that uniform
    service shift, while viewBox height is exact.
- `stress_architecture_external_icons_005` is group-owned rather than top-level-service owned:
  - both root X edges are owned by `group-cloud`;
  - the root padding is stable at `40px`;
  - the remaining `+0.5px` width residual is exactly the emitted `group-cloud` SVG rect width
    delta, not a root-padding or service-body-width issue.

Evidence:

- `target/compare/architecture-report-parity-root-top-service-icon-audit-hpd050.md`
- `target/compare/architecture-render-path-top-service-icon-hpd050`
- `target/compare/architecture-delta-top-service-icon-render-path-hpd050`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-06-hpd-050-architecture-top-service-icon-root-bounds.md`

Focused verification:

- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --out target\compare\architecture-report-parity-root-top-service-icon-audit-hpd050.md` -
  expected-failed with the active `20` Architecture root/style mismatch rows.
- `cargo run -p xtask -- debug-architecture-render-path-probe --fixture stress_architecture_external_icons_005 --fixture upstream_architecture_cypress_fallback_icon --fixture upstream_cypress_architecture_spec_should_render_an_architecture_diagram_with_the_fallback_icon_004 --fixture upstream_html_demos_architecture_default_icon_from_unknown_icon_name_003 --fixture upstream_html_demos_architecture_external_icons_demo_012 --browser-exe "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe" --out target\compare\architecture-render-path-top-service-icon-hpd050` -
  passed; all five fixtures reported `facts match: true`.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_external_icons_005 --fixture upstream_architecture_cypress_fallback_icon --fixture upstream_cypress_architecture_spec_should_render_an_architecture_diagram_with_the_fallback_icon_004 --fixture upstream_html_demos_architecture_default_icon_from_unknown_icon_name_003 --fixture upstream_html_demos_architecture_external_icons_demo_012 --render-probe-dir target\compare\architecture-render-path-top-service-icon-hpd050 --out target\compare\architecture-delta-top-service-icon-render-path-hpd050` -
  passed and wrote the joined current-HEAD reports.
- `cargo run -p xtask -- report-overrides --check-no-growth` - passed; Architecture root
  overrides remain at `0`.
- `git diff --check` - passed.

Gate notes:

- No renderer, layout, fixture, baseline, or root override behavior changed in this pass.
- These five rows are now classified as bounded service/group root-bounds lattice diagnostics, not
  release-blocking production formula candidates.
- Do not add a root-padding, service-body, icon-size, or group-rect constant for these rows. The
  next HPD-050 production-capable work should return to the source-shaped service child-label /
  final-bbox model for the larger direct rows, or to another fresh Architecture/Dagre/Graphlib
  seam with stronger source evidence.

## HPD-050 - Ishikawa Deep-Tree Panic Surface

Outcome:

- Hardened a user-input-reachable panic surface outside Architecture root residual chasing:
  deeply nested Ishikawa cause/subcause trees no longer depend on recursive Rust call-stack
  traversal for semantic JSON projection, typed render-model construction, descendant counting, or
  render-layout flattening.
- `parse_ishikawa(...)` now projects the root node JSON through an explicit postorder stack instead
  of serializing the nested tree through `json!`.
- `arena_node_to_render_model(...)` and `flatten_nodes(...)` now use heap-backed stacks, preserving
  the existing depth-first output order.
- `count_descendants(...)` and render-side `flatten_tree(...)` now use explicit stacks. The
  odd-depth parent-bone lookup also degrades to the current branch bone if the traversal invariant
  is violated instead of panicking with `expect("parent bone exists")`.
- Added public-path regressions:
  - core parses and semantically projects a `1,500`-level Ishikawa hierarchy;
  - render parses the typed model and layouts a `1,200`-level hierarchy through
    `layout_parsed_render_layout_only(...)`.

Evidence:

- `crates/merman-core/src/diagrams/ishikawa.rs`
- `crates/merman-render/src/ishikawa.rs`
- `crates/merman-render/tests/ishikawa_svg_test.rs`
- `docs/quality/PANIC_SURFACE.md`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-06-hpd-050-ishikawa-deep-tree-panic-surface.md`

Focused verification:

- `cargo fmt --check -p merman-core -p merman-render` - passed.
- `cargo nextest run -p merman-core ishikawa` - passed, `5` tests run.
- `cargo nextest run -p merman-render --test ishikawa_svg_test` - passed, `2` tests run.
- `git diff --check` - passed.

Gate notes:

- No SVG baseline, root override, Architecture root-bounds formula, or Mermaid parity fixture was
  changed.
- This is release-boundary hardening for panic-surface policy, not a claim that every recursive
  tree-shaped renderer has been audited.

## HPD-050 - TreeView Depth-Boundary Panic Surface

Outcome:

- Hardened another user-input-reachable tree traversal surface outside Architecture root residual
  chasing: accepted `treeView-beta` trees no longer depend on recursive Rust call-stack traversal
  for typed render-model construction, semantic JSON projection, flattened node projection, or
  render layout.
- The existing `MAX_DIAGRAM_NESTING_DEPTH` policy remains unchanged. This slice does not accept
  deeper TreeView syntax; it removes recursive walkers from the maximum accepted public parse/layout
  boundary and keeps invalid deeper models on the existing `InvalidModel` / parse-error path.
- `arena_node_to_render_model(...)` now builds the nested render model through an explicit
  postorder stack.
- `flatten_nodes(...)` and the root JSON projection now use explicit stacks, preserving the
  existing preorder `nodes` output and nested `root` JSON shape.
- Render layout now uses an explicit enter/exit stack, preserving preorder node layout rows and
  postorder vertical-line emission.
- Added public-path regressions:
  - core parses and semantically projects the maximum accepted `256`-node TreeView chain;
  - render parses and layouts the same maximum accepted chain through
    `layout_parsed_render_layout_only(...)`.

Evidence:

- `crates/merman-core/src/diagrams/tree_view.rs`
- `crates/merman-render/src/tree_view.rs`
- `crates/merman-render/tests/tree_view_svg_test.rs`
- `docs/quality/PANIC_SURFACE.md`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-07-hpd-050-tree-view-depth-boundary.md`

Focused verification:

- `cargo fmt --check -p merman-core -p merman-render` - passed.
- `cargo nextest run -p merman-core tree_view` - passed, `5` tests run.
- `cargo nextest run -p merman-render --test tree_view_svg_test` - passed, `5` tests run.
- `cargo run -p xtask -- compare-tree-view-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed.
- `git diff --check` - passed.

Gate notes:

- No SVG baseline, root override, Architecture root-bounds formula, or Mermaid parity fixture was
  changed.
- This is release-boundary hardening for TreeView's accepted depth boundary; similar tree-shaped
  families remain candidates for follow-up audit.

## HPD-050 - Treemap Deep-Tree Panic Surface

Outcome:

- Hardened Treemap's user-authored hierarchy path after the TreeView depth-boundary cleanup. Unlike
  TreeView, Treemap has no custom `MAX_DIAGRAM_NESTING_DEPTH` rejection path, so this slice keeps
  deeply nested input parseable while removing Rust call-stack recursion from the public parse/layout
  paths.
- `parse_treemap(...)` now builds the semantic `root` object with hand-built `serde_json::Map`
  output and explicit heap-backed traversal, avoiding both recursive tree walkers and deep `json!`
  serialization of user-authored `Value` trees.
- `node_to_value(...)`, `node_to_render_model(...)`, and `flatten_preorder(...)` now use explicit
  stacks while preserving the existing nested root shape and preorder flat `nodes` projection.
- Treemap layout now uses explicit stacks for typed-model flattening, subtree sum computation, and
  source-compatible child sorting.
- The semantic-JSON layout entrypoint now projects Treemap nodes iteratively instead of relying on
  recursive serde deserialization.
- `layout_parsed(...)` now retains semantic JSON through a non-recursive `serde_json::Value` clone.
  This is a shared render-entry hardening because the Treemap deep-chain test reproduced stack
  overflow there even after Treemap's own walkers were converted.
- Added public-path regressions:
  - core parses and semantically projects a `1,200`-level Treemap hierarchy;
  - core builds the typed Treemap render model for the same hierarchy;
  - render parses through the ordinary JSON semantic path and layouts the same hierarchy through
    `layout_parsed(...)`.

Evidence:

- `crates/merman-core/src/diagrams/treemap.rs`
- `crates/merman-render/src/json.rs`
- `crates/merman-render/src/lib.rs`
- `crates/merman-render/src/treemap.rs`
- `crates/merman-render/tests/treemap_svg_test.rs`
- `docs/quality/PANIC_SURFACE.md`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-07-hpd-050-treemap-deep-tree-panic-surface.md`

Focused verification:

- `cargo fmt --check -p merman-core -p merman-render` - passed.
- `cargo nextest run -p merman-core treemap` - passed, `13` tests run.
- `cargo nextest run -p merman-render --test treemap_svg_test` - passed, `6` tests run.
- `cargo run -p xtask -- compare-treemap-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed.
- `git diff --check` - passed.

Gate notes:

- No SVG baseline, root override, Architecture root-bounds formula, or Mermaid parity fixture was
  changed.
- This is release-boundary hardening for Treemap's unbounded hierarchy path plus the shared
  `layout_parsed(...)` deep semantic clone surface it exposed.

## HPD-050 - Mindmap Deep-Tree Panic Surface

Outcome:

- Hardened Mindmap's unbounded user-authored hierarchy path after the Treemap cleanup. Mindmap has
  no custom depth rejection boundary, so this slice keeps deeply nested input parseable while
  removing recursive Rust call-stack traversal from the public parse/layout paths.
- `MindmapDb::assign_sections(...)` now uses an explicit stack while preserving the existing root
  child section assignment semantics.
- Mindmap semantic flat `nodes`, semantic flat `edges`, typed render `nodes`, and typed render
  `edges` now use explicit heap-backed traversal while preserving preorder node order and DFS edge
  order.
- `MindmapDb::to_root_node_value(...)` now builds the nested `rootNode` compatibility field with
  explicit postorder traversal and moves child `Value`s upward instead of recursively walking the
  tree.
- `parse_mindmap(...)` now assembles the final non-empty semantic object with a hand-built
  `serde_json::Map`, avoiding deep `json!` wrapping of the nested `rootNode` value.
- The Mindmap semantic-JSON layout entrypoint now deserializes only the flat `nodes` / `edges`
  fields consumed by layout, avoiding recursive serde traversal of the deep `rootNode`
  compatibility field.
- Added public-path regressions:
  - core parses and semantically projects a `1,200`-level Mindmap chain;
  - core builds the typed Mindmap render model for the same hierarchy;
  - render parses through the ordinary JSON semantic path and layouts the same hierarchy through
    `layout_parsed(...)`.

Evidence:

- `crates/merman-core/src/diagrams/mindmap/db.rs`
- `crates/merman-core/src/diagrams/mindmap/parse.rs`
- `crates/merman-core/src/diagrams/mindmap/tests.rs`
- `crates/merman-render/src/mindmap.rs`
- `crates/merman-render/tests/mindmap_svg_test.rs`
- `docs/quality/PANIC_SURFACE.md`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-07-hpd-050-mindmap-deep-tree-panic-surface.md`

Focused verification:

- `cargo fmt --check -p merman-core -p merman-render` - passed.
- `cargo nextest run -p merman-core mindmap` - passed, `34` tests run.
- `cargo nextest run -p merman-render --test mindmap_svg_test` - passed, `4` tests run.
- `cargo run -p xtask -- compare-mindmap-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed.
- `git diff --check` - passed.

Gate notes:

- No SVG baseline, root override, Architecture root-bounds formula, or Mermaid parity fixture was
  changed.
- This is release-boundary hardening for Mindmap's unbounded hierarchy path. It does not introduce
  a new Mindmap depth limit.

## HPD-050 - Block Deep-Composite Panic Surface

Outcome:

- Hardened Block's user-authored composite hierarchy path after the Mindmap cleanup. A
  `1,200`-level nested Block input reproduced stack overflow in both core semantic projection and
  public SVG rendering before this slice.
- Block DB parent-child population now uses explicit heap-backed tree cloning instead of recursive
  derived `Clone` when copying completed child subtrees into parent composites.
- `BlockDb::blocks_flat(...)` now returns block references instead of recursively cloning every
  stored subtree before semantic or typed render projection.
- Block semantic `blocks`, `edges`, and `blocksFlat` projection still uses explicit postorder
  traversal, and the completed-child maps now degrade by dropping missing invariant children instead
  of panicking.
- `parse_block(...)` now assembles the final semantic object with a hand-built `serde_json::Map`,
  avoiding deep `json!` wrapping of nested block trees.
- The Block semantic-JSON layout and SVG entrypoints now project `blocksFlat` through an explicit
  heap-backed `serde_json::Value` traversal instead of recursive serde traversal over nested
  children.
- Block SVG metadata collection now uses an explicit stack instead of recursive `collect_nodes(...)`.
- Added public-path regressions:
  - core parses and semantically projects a `1,200`-level nested Block composite chain;
  - core builds the typed Block render model for the same hierarchy;
  - render parses, layouts, and renders SVG for the same hierarchy through the public
    `layout_parsed(...)` / `render_block_diagram_svg(...)` path.

Evidence:

- `crates/merman-core/src/diagrams/block.rs`
- `crates/merman-render/src/block.rs`
- `crates/merman-render/src/svg/parity/block.rs`
- `crates/merman-render/tests/block_svg_test.rs`
- `docs/quality/PANIC_SURFACE.md`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-07-hpd-050-block-deep-composite-panic-surface.md`

Focused verification:

- `cargo fmt --check -p merman-core -p merman-render` - passed.
- `cargo nextest run -p merman-core block` - passed, `35` tests run.
- `cargo nextest run -p merman-render --test block_svg_test` - passed, `7` tests run.
- `cargo run -p xtask -- compare-block-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed.
- `git diff --check` - passed.

Gate notes:

- No SVG baseline, root override, Architecture root-bounds formula, or Mermaid parity fixture was
  changed.
- This is release-boundary hardening for Block's accepted deep composite path. It does not
  introduce a new Block depth limit.

## HPD-050 - C4 Deep-Boundary Panic Surface

Outcome:

- Hardened C4's user-authored boundary/deployment-node nesting path after the Block cleanup. A
  `1,500`-level nested C4 boundary input reproduced stack overflow in the public render-model
  layout path before this slice.
- C4 core semantic output already stays flat for `boundaries` / `shapes`; this slice keeps the
  production change scoped to render layout rather than introducing a new nested semantic shape.
- `layout_inside_boundary(...)` now uses an explicit heap-backed frame stack instead of recursive
  calls while preserving the existing parent-bounds accumulation model:
  - sibling boundary row placement still uses the shared per-level `current_bounds`;
  - shapes still lay out before child boundaries;
  - child boundary layout still expands the pending parent's bounds before the parent boundary is
    finalized;
  - root width/height still come from the accumulated global C4 bounds.
- Added a public-path regression:
  - render parses a `1,500`-level C4 boundary chain through `parse_diagram_for_render_model_sync`
    and layouts it through `layout_parsed_render_layout_only(...)`.

Evidence:

- `crates/merman-render/src/c4.rs`
- `crates/merman-render/tests/c4_svg_test.rs`
- `docs/quality/PANIC_SURFACE.md`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-07-hpd-050-c4-deep-boundary-panic-surface.md`

Focused verification:

- `cargo nextest run -p merman-render --test c4_svg_test c4_public_layout_handles_deep_boundary_chain` -
  first run failed before the fix with stack overflow; passed after the explicit-stack layout
  traversal.
- `cargo nextest run -p merman-render c4` - passed, `6` tests run.
- `cargo fmt --check -p merman-render` - passed.
- `cargo run -p xtask -- compare-c4-svgs --check-dom --dom-mode parity --dom-decimals 3` - passed.
- `git diff --check` - passed.

Gate notes:

- No SVG baseline, root override, Architecture root-bounds formula, or Mermaid parity fixture was
  changed.
- This is release-boundary hardening for C4's accepted deep boundary path. It does not introduce a
  new C4 depth limit.

## HPD-050 - State Deep-Composite Panic Surface

Outcome:

- Hardened State's user-authored composite-state hierarchy path after the C4 cleanup. A
  `1,500`-level nested `stateDiagram-v2` composite input reproduced stack overflow in the public
  render-model parse path before the core fix, and the first render-only cluster-extraction fix
  was insufficient because parse-only still overflowed.
- `StateDb::extract(...)` now traverses the parsed root document by reference instead of cloning
  the deep AST, and `StateRecord` no longer stores recursively cloned composite `doc` subtrees.
- State semantic JSON now projects top-level composite `doc` compatibility values through explicit
  heap-backed traversal and hand-built `serde_json::Map` output, avoiding recursive `json!`
  wrapping of already deep `Value` trees.
- `StateDb` now drops the parsed AST through an explicit stack so successful parse/render-model
  paths do not overflow while cleaning up a deep public input.
- State render cluster extraction, cluster preparation, nested prepared-graph layout, and
  prepared-graph cleanup now use explicit heap-backed stacks. The old `prepare_graph(...)`
  10-level recursion bailout was removed, so deep no-edge composite chains no longer fall through
  into a huge unextracted compound Dagre graph.
- Added public-path regressions:
  - core parses and semantically projects a `1,200`-level State composite chain;
  - core builds the typed State render model for the same chain;
  - render parses a `1,500`-level State composite chain through
    `parse_diagram_for_render_model_sync(...)`;
  - render layouts a `512`-level State composite chain through `layout_parsed(...)`.

Evidence:

- `crates/merman-core/src/diagrams/state/db.rs`
- `crates/merman-core/src/tests/state.rs`
- `crates/merman-render/src/state/layout.rs`
- `crates/merman-render/tests/state_layout_test.rs`
- `docs/quality/PANIC_SURFACE.md`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-07-hpd-050-state-deep-composite-panic-surface.md`

Focused verification:

- `cargo nextest run -p merman-render --test state_layout_test state_parse_for_render_model_handles_deep_composite_chain` -
  first run failed before the core fix with stack overflow; passed after the non-recursive State DB
  extraction/projection/drop changes.
- `cargo nextest run -p merman-core state_deep_composite_chain_semantic_and_render_model_use_heap_traversal` -
  first run failed before replacing the final semantic `json!` wrapper; passed after hand-built
  semantic root assembly.
- `cargo nextest run -p merman-render --test state_layout_test state_layout_handles_deep_composite_chain` -
  passed after the explicit-stack cluster preparation and layout traversal.
- `cargo nextest run -p merman-core state` - passed, `39` tests run.
- `cargo nextest run -p merman-render state` - passed, `17` tests run.
- `cargo fmt --check -p merman-core -p merman-render` - passed.
- `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed.

Gate notes:

- No SVG baseline, root override, Architecture root-bounds formula, or Mermaid parity fixture was
  changed.
- This is release-boundary hardening for State's accepted deep composite path. It does not
  introduce a new State depth limit.

## HPD-050 - Flowchart Deep-Subgraph Panic Surface

Outcome:

- Hardened Flowchart's public nested `subgraph` path after the State cleanup. A `1,200`-level
  `flowchart TB` subgraph chain parsed successfully but reproduced stack overflow in the public
  layout path before this slice.
- Flowchart's extracted cluster placement now uses an explicit heap-backed frame stack instead of
  recursive `place_graph(...)` calls while preserving parent/child graph offset calculation, edge
  label overrides, and extracted cluster rect/base-width capture.
- Fallback compound subtree rectangle collection and final cluster rectangle postorder computation
  now use explicit stacks instead of recursively walking subgraph membership.
- Flowchart nested SVG root rendering now uses explicit render frames instead of recursively
  calling `render_flowchart_root(...)`, preserving Mermaid's nested `.root` group ordering and
  timing counters.
- Added public-path regressions:
  - render-model parsing accepts a `1,200`-level Flowchart subgraph chain;
  - public `layout_parsed(...)` layouts the same chain and emits the leaf node plus outer cluster;
  - public SVG rendering emits the same chain without stack overflow and with current Flowchart DOM
    id shape.

Evidence:

- `crates/merman-render/src/flowchart/layout.rs`
- `crates/merman-render/src/svg/parity/flowchart/render/root.rs`
- `crates/merman-render/tests/flowchart_svg_test.rs`
- `docs/quality/PANIC_SURFACE.md`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-07-hpd-050-flowchart-deep-subgraph-panic-surface.md`

Focused verification:

- `cargo nextest run -p merman-render flowchart_parse_for_render_model_handles_deep_subgraph_chain` -
  passed, `1` test run.
- `cargo nextest run -p merman-render flowchart_layout_handles_deep_subgraph_chain` - first run
  failed before the fix with stack overflow; passed after the non-recursive layout placement and
  cluster-rect traversal changes.
- `cargo nextest run -p merman-render flowchart_svg_handles_deep_subgraph_chain` - passed after
  the explicit-stack SVG root traversal and current DOM id assertion.
- `cargo nextest run -p merman-render flowchart` - passed, `106` tests run.
- `cargo fmt --check -p merman-render` - passed.
- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed.
- `git diff --check` - passed.

Gate notes:

- No SVG baseline, root override, Architecture root-bounds formula, or Mermaid parity fixture was
  changed.
- This is release-boundary hardening for Flowchart's accepted deep subgraph path. It does not
  introduce a new Flowchart depth limit or claim closure of known Flowchart max-width/root
  residuals.

## HPD-050 - Class Namespace And Dugong Deep Traversal Panic Surface

Outcome:

- Hardened Class's public nested `namespace` path after the Flowchart cleanup. A deep
  `classDiagram` namespace chain was publicly parseable; parse-only stayed green, but layout first
  exposed recursive traversal in dugong/graphlib and SVG output then exposed recursive namespace
  root rendering.
- `dugong::rank::util::longest_path(...)` now computes ranks with an explicit frame stack instead
  of recursive DFS, preserving `minlen` rank propagation across deep edge chains.
- `dugong_graphlib::alg::{preorder, postorder}` now traverse successors iteratively while
  preserving the upstream-compatible root/successor order and missing-root panic behavior.
- `dugong::order::sort_subgraph_ix(...)`, timed sort-subgraph traversal, and the public
  `dugong::order::sort_subgraph(...)` API now use explicit enter/exit frames for deep compound
  subgraph chains.
- Class namespace SVG root output now uses explicit render frames instead of recursively calling
  `render_class_namespace_root(...)`, preserving the existing DOM ordering:
  namespace root open, clusters, edge labels, node group, child roots, edge paths, and close.
- Added public-path and cheap lower-level regressions:
  - Class parse, layout, and SVG output cover a `128`-level namespace chain on a small thread
    stack;
  - Graphlib preorder/postorder cover a `2,048`-edge successor chain on a `64KB` stack;
  - dugong longest-path covers a `2,048`-edge rank chain on a `64KB` stack;
  - public `sort_subgraph(...)` covers a `2,048`-level compound chain on a `64KB` stack.

Evidence:

- `crates/dugong-graphlib/src/graph/alg.rs`
- `crates/dugong-graphlib/tests/alg_test.rs`
- `crates/dugong/src/order/barycenter.rs`
- `crates/dugong/src/rank/util.rs`
- `crates/dugong/tests/order_sort_subgraph_test.rs`
- `crates/dugong/tests/rank_util_test.rs`
- `crates/merman-render/src/svg/parity/class/nodes.rs`
- `crates/merman-render/tests/class_svg_test.rs`
- `docs/quality/PANIC_SURFACE.md`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-07-hpd-050-class-namespace-dugong-deep-traversal.md`

Focused verification:

- `cargo fmt --check -p dugong -p dugong-graphlib -p merman-render` - passed.
- `cargo nextest run -p dugong-graphlib --test alg_test` - passed.
- `cargo nextest run -p dugong --test rank_util_test` - passed.
- `cargo nextest run -p dugong --test order_sort_subgraph_test` - passed.
- `cargo nextest run -p merman-render --test class_svg_test` - passed.
- `cargo run -p xtask -- compare-class-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed.
- `git diff --check` - passed.

Gate notes:

- No SVG baseline, root override, Architecture root-bounds formula, or Mermaid parity fixture was
  changed.
- The public Class regression is intentionally `128` levels because deeper public layout chains are
  slow enough to behave like a performance stress test on Windows. Deeper stack-safety coverage is
  carried by cheap dugong/graphlib unit regressions.
- This is release-boundary hardening for Class namespace layout/SVG and dugong-adjacent traversal,
  not a claim that Class root residuals or Architecture solver diagnostics are closed.

## HPD-050 - Architecture Deep-Group Panic Surface

Outcome:

- Hardened Architecture's public nested group path after the Class namespace / dugong cleanup. A
  deep `architecture-beta` `group ... in ...` chain was publicly parseable; parse-only stayed
  green, while layout reproduced stack overflow on a small thread stack before the manatee/FCoSE
  traversal cleanup.
- `manatee::algo::fcose::SimGraph::from_indexed(...)` no longer computes compound inclusion depth
  through recursive parent calls. It now expands the parent chain explicitly and backfills memoized
  depths.
- `SimGraph::all_nodes_layout_order(...)` no longer recursively visits owner graphs. It now uses
  an explicit preorder stack while preserving layout-base graph/node iteration order.
- Architecture SVG group rectangle computation no longer recursively calls
  `GroupRectComputer::compute(...)` for child groups. It now uses explicit enter/exit frames and
  keeps the existing service, junction, child-group inset, padding, debug, and empty-group
  behavior.
- Added public-path and cheap lower-level regressions:
  - Architecture parse, layout, and SVG output cover a `64`-level group chain on a small thread
    stack;
  - manatee/FCoSE compound depth and layout-order reconstruction cover a `2,048`-level compound
    chain on a `64KB` stack;
  - Architecture SVG group-rect computation covers a `2,048`-level child-group chain on a `64KB`
    stack.

Evidence:

- `crates/manatee/src/algo/fcose/mod.rs`
- `crates/merman-render/src/svg/parity/architecture/geometry.rs`
- `crates/merman-render/tests/architecture_layout_test.rs`
- `crates/merman-render/tests/architecture_svg_test.rs`
- `docs/quality/PANIC_SURFACE.md`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-07-hpd-050-architecture-deep-group-panic-surface.md`

Focused verification:

- `cargo fmt --check -p manatee -p merman-render` - passed.
- `cargo nextest run -p manatee` - passed, `16` tests run.
- `cargo nextest run -p merman-render --test architecture_layout_test --test architecture_svg_test` -
  passed, `17` tests run and `1` skipped.
- `cargo nextest run -p merman-render group_rect_computer_handles_deep_child_group_chain_with_small_stack` -
  passed.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed.
- `git diff --check` - passed.

Gate notes:

- No SVG baseline, root override, Architecture root-bounds formula, or Mermaid parity fixture was
  changed.
- The public Architecture regression is intentionally `64` levels because deeper public layout
  chains quickly become FCoSE performance stress tests on Windows. Deeper stack-safety coverage is
  carried by cheap manatee and SVG group-rect unit regressions.
- This is release-boundary hardening for Architecture group layout/SVG traversal, not a claim that
  Architecture `parity-root` diagnostics or group-bounds residuals are closed.

## HPD-050 - Architecture iconText XHTML Panic Surface

Outcome:

- Hardened Architecture service `iconText` XHTML fragment normalization after the deep-group
  cleanup. Mermaid `architecture-beta` exposes `iconText` through public service syntax, and the
  renderer accepts XHTML/SVG-like markup inside the service icon foreignObject.
- The fragment parser was already stack-based, but the post-parse namespace rewrite and fragment
  serialization still walked the user-authored fragment tree recursively.
- `rewrite_foreign_object_fragment_nodes(...)` now uses explicit heap-backed frames while
  preserving SVG/HTML namespace classification, SVG integration-point behavior, and the existing
  split of HTML children out of SVG-only parents.
- `serialize_foreign_object_fragment(...)` now consumes the fragment tree through an explicit stack,
  taking child vectors before each node is dropped so deep fragments do not overflow during
  traversal or cleanup.
- Added regressions:
  - public Architecture SVG output covers a `1,200`-level nested XHTML `iconText` fragment;
  - the lower-level Architecture foreignObject normalizer covers a `2,048`-level nested XHTML
    fragment on a `64KB` stack.
- Rechecked the previously landed Architecture deep-group layout regression after a user-reported
  abort; it passes on the current worktree.

Evidence:

- `crates/merman-render/src/svg/parity/architecture/foreign_object.rs`
- `crates/merman-render/tests/architecture_svg_test.rs`
- `docs/quality/PANIC_SURFACE.md`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-07-hpd-050-architecture-icon-text-xhtml-panic-surface.md`

Focused verification:

- `cargo nextest run -p merman-render architecture_svg_handles_deep_icon_text_xhtml_fragment` -
  passed.
- `cargo nextest run -p merman-render normalize_xhtml_fragment_handles_deep_nested_html_with_small_stack` -
  passed.
- `cargo nextest run -p merman-render architecture_layout_handles_deep_group_chain` - passed.
- `cargo nextest run -p merman-render --test architecture_layout_test --test architecture_svg_test` -
  passed, `18` tests run and `1` skipped.
- `cargo fmt --check` - passed.
- `git diff --check` - passed.

Gate notes:

- No SVG baseline, root override, Architecture root-bounds formula, Mermaid parity fixture, or
  sanitizer behavior changed.
- The public SVG regression initializes `Engine` outside the artificial small-stack thread so the
  gate measures user-controlled XHTML traversal rather than fixed registry/theme initialization
  overhead.
- This is release-boundary hardening for Architecture foreignObject XHTML handling, not a claim
  that Architecture `parity-root` diagnostics or group/service bbox residuals are closed.

## HPD-050 - Dugong And Graphlib Cycle Traversal Panic Surface

Outcome:

- Hardened two remaining cycle-traversal paths discovered while auditing production panic and
  recursion surfaces after the Architecture XHTML cleanup.
- `dugong_graphlib::alg::find_cycles(...)` previously ran Tarjan SCC traversal through recursive
  `strongconnect(...)`. A public Graphlib `2,048`-edge successor chain reproduced stack overflow
  on a `64KB` stack before this slice even though the graph had no cycles.
- `dugong::acyclic::run(...)` defaults to Dagre's DFS feedback-arc path when `acyclicer` is absent,
  `"dfs"`, or unknown. That path previously recursed through `dfs_fas(...)`, and a `2,048`-edge
  acyclic successor chain reproduced stack overflow on a `64KB` stack before this slice.
- Both traversals now use explicit heap-backed frames:
  - Graphlib Tarjan preserves successor order, lowlink propagation, SCC output, and self-loop cycle
    filtering;
  - Dugong acyclic DFS preserves Dagre's node insertion order, out-edge order, self-loop skip, and
    feedback-edge collection behavior.
- Added regressions:
  - `find_cycles_handles_deep_successor_chains_with_small_stack`;
  - `acyclic_run_handles_deep_dfs_chains_with_small_stack`.

Evidence:

- `crates/dugong-graphlib/src/graph/alg.rs`
- `crates/dugong-graphlib/tests/alg_test.rs`
- `crates/dugong/src/acyclic.rs`
- `crates/dugong/tests/acyclic_test.rs`
- `docs/quality/PANIC_SURFACE.md`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-07-hpd-050-dugong-cycle-traversal-panic-surface.md`

Focused verification:

- `cargo nextest run -p dugong-graphlib find_cycles_handles_deep_successor_chains_with_small_stack` -
  first run failed before the fix with stack overflow; passed after iterative Tarjan traversal.
- `cargo nextest run -p dugong acyclic_run_handles_deep_dfs_chains_with_small_stack` - first run
  failed before the fix with stack overflow; passed after iterative DFS feedback-arc traversal.
- `cargo nextest run -p dugong-graphlib --test alg_test` - passed, `23` tests run.
- `cargo nextest run -p dugong --test acyclic_test --test greedy_fas_test` - passed, `15` tests
  run.
- `cargo nextest run -p dugong-graphlib` - passed, `99` tests run.
- `cargo nextest run -p dugong` - passed, `278` tests run.
- `cargo nextest run -p merman-render --test class_svg_test` - passed, `26` tests run.
- `cargo nextest run -p merman-render --test flowchart_svg_test` - passed, `34` tests run.
- `cargo nextest run -p merman-render state` - passed, `17` tests run.
- `cargo fmt --check -p dugong -p dugong-graphlib` - passed.
- `git diff --check` - passed.

Gate notes:

- No SVG baseline, root override, Mermaid parity fixture, Architecture root-bounds formula, or
  rendered output formula changed.
- This is release-boundary hardening for public Graphlib cycle detection and Dugong's default
  Dagre cycle-removal traversal. It does not change upstream Dagre semantics or claim closure of
  any `parity-root` residual.

## HPD-050 - Shared Config And Directive Panic Surface

Outcome:

- Hardened the shared Mermaid config boundary after the Dugong/Graphlib cycle traversal cleanup.
  The public inputs here are host `site_config`, YAML frontmatter config, and `%%{init: ...}%%`
  directive config.
- `MermaidConfig` clone-on-write, `set_value(...)`, `deep_merge(...)`, and legacy root
  `fontFamily` mirroring now use explicit heap-backed clone/drop/merge paths instead of recursive
  `serde_json::Value` clone/drop.
- Init directive sanitization now uses an explicit path stack over objects and arrays while
  preserving the existing sanitizer behavior:
  - remove `secure`;
  - remove keys beginning with `__`;
  - clear string values containing `<`, `>`, or `url(data:`.
- Frontmatter keeps the legacy `serde_yaml::Value` to `serde_json::Value` conversion behavior,
  including ignoring non-string YAML keys, while subsequent config merge/drop paths avoid recursive
  `serde_json::Value` clone/drop.
- Frontmatter stripping in preprocess and `DetectorRegistry::detect_type(...)` now uses line
  scanning instead of broad regex replacement over user input.
- Deep YAML / JSON5 config bodies are rejected before entering the recursive third-party parsers
  when their structural nesting exceeds `MAX_DIAGRAM_NESTING_DEPTH`; the guard covers flow
  collections, YAML indentation depth, and inline YAML sequence indicators. Accepted nesting still
  merges through the same config semantics.
- Added regressions for:
  - deep host `site_config` merge through public metadata parsing;
  - accepted init directive config sanitization;
  - accepted frontmatter config merge;
  - excessive init/frontmatter config rejection without stack overflow;
  - excessive inline YAML sequence nesting rejection without stack overflow;
  - non-string YAML key conversion compatibility;
  - config nesting helper coverage for inline YAML sequence indicators;
  - deep directive sanitizer traversal on a `64KB` stack;
  - deep config clone-on-write on a `64KB` stack;
  - detector frontmatter stripping on a `64KB` stack.

Evidence:

- `crates/merman-core/src/config/mod.rs`
- `crates/merman-core/src/preprocess/mod.rs`
- `crates/merman-core/src/detect/mod.rs`
- `crates/merman-core/src/tests/misc.rs`
- `crates/merman-core/src/tests/detect.rs`
- `docs/quality/PANIC_SURFACE.md`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-07-hpd-050-config-directive-panic-surface.md`

Focused verification:

- `cargo +1.95 nextest run -p merman-core clone_on_write_handles_deep_config_with_small_stack sanitize_directive_handles_deep_values_with_small_stack detector_registry_strips_deep_frontmatter_with_small_stack site_config_deep_merge_handles_deep_public_config_with_small_stack init_directive_config_sanitizes_deep_values_with_small_stack frontmatter_config_deep_merge_handles_deep_values_with_small_stack init_directive_rejects_excessive_config_nesting_with_small_stack frontmatter_rejects_excessive_config_nesting_with_small_stack frontmatter_rejects_excessive_inline_yaml_sequence_nesting_with_small_stack frontmatter_non_string_yaml_keys_are_ignored_like_legacy_conversion config_nesting_counts_inline_yaml_sequence_indicators` -
  passed, `11` tests run.
- `cargo +1.95 nextest run -p merman-core` - passed, `609` tests run.
- `cargo +1.95 fmt` - passed.
- `git diff --check` - passed.

Gate notes:

- No SVG baseline, root override, Mermaid parity fixture, rendered output formula, or
  Architecture root-bounds behavior changed.
- The default `cargo` shim for the repo's `1.95.0` override reported that its `cargo.exe` component
  was not applicable, so verification used the installed `1.95-x86_64-pc-windows-msvc` toolchain
  explicitly.

## HPD-050 - COSE-Bilkent Radial Tree Panic Surface

Outcome:

- Hardened the Mindmap-facing COSE-Bilkent radial tree placement path in `manatee`.
- `SimGraph::branch_radial_layout(...)` no longer recursively descends through forest branches.
  It now uses explicit heap-backed `BranchFrame` traversal while preserving the previous node
  angle, child order, parent-edge skip, and radial-distance semantics.
- Added a public `layout_indexed(...)` regression that lays out a `2,048`-node tree on a `64KB`
  stack and asserts finite output positions.
- This is release-boundary stack-safety hardening for a shared layout algorithm used by Mindmap
  rendering. It does not change Mermaid SVG baselines, root viewport formulas, Architecture
  residuals, or COSE-Bilkent force constants.

Evidence:

- `crates/manatee/src/algo/cose_bilkent/mod.rs`
- `docs/quality/PANIC_SURFACE.md`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-07-hpd-050-cose-bilkent-radial-panic-surface.md`

Focused verification:

- `cargo +1.95 nextest run -p manatee layout_indexed_handles_deep_tree_radial_layout_with_small_stack` -
  passed, `1` test run.
- `cargo +1.95 nextest run -p manatee` - passed, `17` tests run.
- `cargo +1.95 nextest run -p merman-render --test mindmap_svg_test` - passed, `4` tests run.
- `cargo +1.95 fmt` - passed.
- `git diff --check` - passed.

Gate notes:

- The broader `merman-render` package and full SVG compare matrix were not rerun for this narrow
  layout traversal change. The focused Mindmap SVG test covers the direct renderer integration,
  while `manatee` package tests cover the changed algorithm crate.

## HPD-050 - ASCII Flowchart Group Bounds Panic Surface

Outcome:

- Hardened the public ASCII Flowchart render path after the SVG Flowchart deep-subgraph cleanup.
- ASCII group raw-bounds calculation no longer recursively re-enters child groups. It now builds
  node/group lookup tables and resolves descendant group bounds with explicit enter/exit frames,
  preserving the previous child-before-parent bounds aggregation and title padding behavior.
- Added a public `merman` ASCII API regression that renders a `512`-level `flowchart TB` subgraph
  chain on a `64KB` stack and asserts the leaf node remains visible.
- This is release-boundary stack-safety hardening for terminal rendering. It does not change SVG
  baselines, root viewport formulas, Mermaid parity fixtures, or graph layout spacing constants.

Evidence:

- `crates/merman-ascii/src/graph/layout.rs`
- `crates/merman/tests/ascii_api.rs`
- `docs/quality/PANIC_SURFACE.md`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-07-hpd-050-ascii-flowchart-group-bounds-panic-surface.md`

Focused verification:

- `cargo +1.95 nextest run -p merman --features ascii --test ascii_api render_ascii_model_handles_deep_flowchart_subgraph_chain_with_small_stack` -
  passed, `1` test run.
- `cargo +1.95 nextest run -p merman --features ascii --test ascii_api` - passed, `7` tests run.

Gate notes:

- The default `cargo` shim for the repo's `1.95.0` override reported that its `cargo.exe` component
  was not applicable, so verification used the installed `1.95-x86_64-pc-windows-msvc` toolchain
  explicitly.
- Running the same `ascii_api` target without `--features ascii` compiles `0` tests because the
  integration test is feature-gated with `#![cfg(feature = "ascii")]`; those no-test runs were not
  counted as evidence.

## HPD-050 - Sequence Compat JSON Panic Surface

Outcome:

- Hardened the Sequence typed render-model compatibility bridge after the ASCII Flowchart cleanup.
- `SequenceDiagramRenderModel::to_compat_json(...)` no longer serializes `self` through
  `serde_json::to_value(...)` and then depends on `expect`, `unreachable!`, and field-removal
  panics to rebuild the public JSON object.
- The compat object is now assembled directly from typed fields while preserving the old JSON
  shape:
  - `accTitle`, `accDescr`, `actorOrder`, `createdActors`, `destroyedActors`, `actorKeys`, and
    `centralConnection` names stay camel-cased as before;
  - `placement` remains omitted when absent;
  - `centralConnection` remains omitted for `0`;
  - Sequence autonumber values keep integer JSON numbers for whole finite values and float JSON
    numbers for decimal values.
- This is a production panic-surface cleanup for an existing parser/render bridge. It does not
  change Sequence parsing semantics, SVG output, baselines, root viewport formulas, or known
  Sequence measurement residuals.

Evidence:

- `crates/merman-core/src/diagrams/sequence/render_model.rs`
- `docs/quality/PANIC_SURFACE.md`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-07-hpd-050-sequence-compat-json-panic-surface.md`

Focused verification:

- `cargo +1.95 fmt --check -p merman-core` - passed.
- `cargo +1.95 nextest run -p merman-core parse_sequence_render_model_uses_typed_variant_without_changing_json_parse` -
  passed, `1` test run.
- `cargo +1.95 nextest run -p merman-core sequence` - passed, `34` tests run.
- `git diff --check` - passed.

Gate notes:

- The existing typed-vs-JSON regression compares the hand-built compat JSON against the legacy
  `parse_diagram_sync(...)` JSON path, including optional message fields and created/destroyed
  actor maps.
- No new broad `merman-core` package run was performed for this narrow internal JSON projection
  cleanup; the focused Sequence package filter covers the changed diagram family.

## HPD-050 - XYChart Compat JSON Panic Surface

Outcome:

- Hardened the XYChart typed render-model compatibility bridge after the Sequence compat JSON
  cleanup.
- `parse_xychart(...)` no longer serializes `XyChartDiagramRenderModel` through
  `serde_json::to_value(...).expect(...)` before adding public `type` and `config` fields.
- `XyChartDiagramRenderModel::to_compat_json(...)` now assembles the compatibility map directly
  from typed fields while preserving the old JSON shape:
  - `accTitle`, `accDescr`, `xAxis`, and `yAxis` names stay camel-cased as before;
  - axis variants still emit tagged `type` values of `band` and `linear`;
  - plot variants still emit tagged `type` values of `bar` and `line`;
  - absent optional title/accessibility fields and optional numeric axis/data values still emit
    JSON `null`;
  - retained `config` is copied with the shared non-recursive JSON clone helper instead of a
    recursive `serde_json::Value` clone.
- This is a production panic-surface cleanup for an existing parser/render bridge. It does not
  change XYChart parsing semantics, SVG output, baselines, root viewport formulas, or known XYChart
  parity residuals.

Evidence:

- `crates/merman-core/src/diagrams/xychart.rs`
- `crates/merman-core/src/tests/misc.rs`
- `docs/quality/PANIC_SURFACE.md`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-07-hpd-050-xychart-compat-json-panic-surface.md`

Focused verification:

- `cargo +1.95 fmt --check -p merman-core` - passed.
- `cargo +1.95 nextest run -p merman-core parse_xychart_render_model_uses_typed_variant_without_changing_json_parse` -
  passed, `1` test run.
- `cargo +1.95 nextest run -p merman-core xychart` - passed, `17` tests run.
- `git diff --check` - passed.

Gate notes:

- The typed-vs-JSON regression now compares the full hand-built compat JSON object against the
  legacy `parse_diagram_sync(...)` JSON path, including the retained `config` field.
- No new broad `merman-core` package run was performed for this narrow internal JSON projection
  cleanup; the focused XYChart package filter covers the changed diagram family.

## HPD-050 - Retained Semantic Config Panic Surface

Outcome:

- Hardened retained effective-config projection in public semantic JSON roots after the Sequence
  and XYChart compat JSON cleanup.
- Block, State, Treemap, Sankey, C4, and Architecture public JSON models now copy retained
  `meta.effective_config` through `clone_value_nonrecursive(...)` instead of recursive
  `serde_json::Value::clone()`.
- C4, Sankey, and Architecture now assemble the final semantic JSON root with hand-built
  `serde_json::Map` objects so the retained config is moved into the result and not recursively
  wrapped through `json!`.
- Architecture still applies the source-backed default `layout: "dagre"` fallback to the cloned
  effective config when the caller did not supply an explicit layout.
- Added a small-stack regression that parses Block, State, Treemap, Sankey, C4, and Architecture
  through the known-type semantic JSON entrypoint with a `1,024`-level host `site_config`, verifies
  the retained config leaf, and drops the returned model through the non-recursive drop helper.

Evidence:

- `crates/merman-core/src/diagrams/architecture.rs`
- `crates/merman-core/src/diagrams/block.rs`
- `crates/merman-core/src/diagrams/c4.rs`
- `crates/merman-core/src/diagrams/sankey.rs`
- `crates/merman-core/src/diagrams/state/db.rs`
- `crates/merman-core/src/diagrams/treemap.rs`
- `crates/merman-core/src/tests/misc.rs`
- `docs/quality/PANIC_SURFACE.md`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-07-hpd-050-retained-semantic-config-panic-surface.md`

Focused verification:

- `cargo +1.95 fmt -p merman-core` - passed.
- `cargo +1.95 nextest run -p merman-core retained_semantic_config_handles_deep_public_config_with_small_stack` -
  passed, `1` test run.
- `cargo +1.95 nextest run -p merman-core block_render_model_uses_typed_variant_without_changing_json_parse treemap_render_model_uses_typed_variant_without_changing_json_parse parse_sankey_render_model_uses_typed_variant_without_changing_json_parse c4_render_model_uses_typed_variant_without_changing_json_parse` -
  passed, `4` tests run.
- `cargo +1.95 nextest run -p merman-core state architecture c4 block treemap sankey` - passed,
  `133` tests run.
- `cargo +1.95 fmt --check -p merman-core` - passed.
- `git diff --check` - passed.

Gate notes:

- The small-stack regression intentionally uses `parse_diagram_with_type_sync(...)` to isolate the
  retained semantic config projection path from automatic detector-registry initialization and
  detector-chain overhead. A diagnostic auto-detect attempt overflowed before semantic parsing,
  inside detection, which is a separate boundary and is not claimed by this slice.
- This is a semantic JSON panic-surface hardening slice only. It does not change parser behavior,
  SVG output, SVG baselines, root viewport formulas, theme semantics, or Architecture residual
  classification.

## HPD-050 - C4 Detector Regex Panic Surface

Outcome:

- Hardened the automatic detection boundary exposed while validating retained semantic config.
  The first failing auto-detect small-stack attempt overflowed before semantic parsing, in the
  detector path.
- `detector_c4(...)` no longer lazily compiles a static regex on first use. It now uses direct
  string checks for the same fixed Mermaid source pattern.
- The hand-written checks preserve the upstream ungrouped regex shape:
  - `C4Context` matches only after leading whitespace, like `^\s*C4Context`;
  - `C4Container`, `C4Component`, `C4Dynamic`, and `C4Deployment` match anywhere in cleaned text,
    matching the ungrouped alternation behavior.
- Added a small-stack metadata parsing regression for common headers with a `1,024`-level host
  config, covering `block`, `sankey`, `treemap`, and `C4Context`.

Evidence:

- `crates/merman-core/src/detect/mod.rs`
- `crates/merman-core/src/tests/detect.rs`
- `docs/quality/PANIC_SURFACE.md`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-07-hpd-050-c4-detector-regex-panic-surface.md`

Focused verification:

- `cargo +1.95 fmt -p merman-core` - passed.
- `cargo +1.95 nextest run -p merman-core c4_detector_preserves_upstream_ungrouped_regex_shape auto_detect_common_headers_with_deep_config_small_stack` -
  passed, `2` tests run.
- `cargo +1.95 nextest run -p merman-core detect` - passed, `17` tests run.
- `cargo +1.95 fmt --check -p merman-core` - passed.
- `git diff --check` - passed.

Gate notes:

- No detector ordering or family profile behavior changed. This is an equivalent implementation of
  the existing C4 detector shape, not a fast-detect expansion.
- This is a parser detection panic-surface cleanup only. It does not change semantic models,
  rendered output, SVG baselines, root viewport formulas, or Architecture residual classification.

## HPD-050 - Remaining Retained Semantic Config Panic Surface

Outcome:

- Completed the retained effective-config projection sweep for remaining public semantic JSON
  roots after the Block/State/Treemap/Sankey/C4/Architecture slice.
- GitGraph, Kanban, Packet, QuadrantChart, Radar, Requirement, and Mindmap now copy retained
  `meta.effective_config` with `clone_value_nonrecursive(...)` instead of recursive
  `serde_json::Value::clone()`.
- These roots now use hand-built `serde_json::Map` objects where the retained config enters the
  public root object, so deep host config is moved into the model instead of recursively wrapped
  through `json!`.
- Mindmap's normal-root and empty-root early-return semantic paths are both covered. The existing
  source-backed default `layout: "cose-bilkent"` insertion is preserved when the caller did not
  provide an explicit layout.
- Added a second small-stack retained-config regression covering GitGraph, Kanban, Packet,
  QuadrantChart, Radar, Requirement, Mindmap normal root, and Mindmap empty root with a
  `1,024`-level host `site_config`.

Evidence:

- `crates/merman-core/src/diagrams/git_graph.rs`
- `crates/merman-core/src/diagrams/kanban.rs`
- `crates/merman-core/src/diagrams/mindmap/parse.rs`
- `crates/merman-core/src/diagrams/packet.rs`
- `crates/merman-core/src/diagrams/quadrant_chart.rs`
- `crates/merman-core/src/diagrams/radar.rs`
- `crates/merman-core/src/diagrams/requirement.rs`
- `crates/merman-core/src/tests/misc.rs`
- `docs/quality/PANIC_SURFACE.md`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-07-hpd-050-retained-semantic-config-remaining-families.md`

Focused verification:

- `cargo +1.95 fmt -p merman-core` - passed.
- `cargo +1.95 nextest run -p merman-core remaining_retained_semantic_config_handles_deep_public_config_with_small_stack` -
  passed, `1` test run.
- `cargo +1.95 nextest run -p merman-core parse_kanban_render_model_uses_typed_variant_without_changing_json_parse parse_packet_render_model_uses_typed_variant_without_changing_json_parse parse_requirement_render_model_uses_typed_variant_without_changing_json_parse parse_radar_render_model_uses_typed_variant_without_changing_json_parse parse_gitgraph_render_model_uses_typed_variant_without_changing_json_parse parse_quadrant_chart_render_model_uses_typed_variant_without_changing_json_parse mindmap_render_model_projects_same_look_and_theme_shape_as_json_model` -
  passed, `7` tests run.
- `cargo +1.95 nextest run -p merman-core gitGraph kanban packet quadrant radar requirement mindmap` -
  passed, `117` tests run.
- `cargo +1.95 fmt --check -p merman-core` - passed.
- `git diff --check` - passed.

Gate notes:

- The small-stack regression intentionally uses `parse_diagram_with_type_sync(...)` to keep this
  evidence scoped to semantic JSON projection. Detector-registry small-stack behavior remains a
  separate boundary and is not claimed by this slice.
- `rg -n -F "effective_config.as_value().clone()" crates/merman-core/src` now finds no remaining
  production or test use in `merman-core`.
- This is a semantic JSON panic-surface hardening slice only. It does not change parser behavior,
  SVG output, SVG baselines, root viewport formulas, theme semantics, or Architecture residual
  classification.

## HPD-050 - Detector Comment Cleanup Panic Surface

Outcome:

- Removed the remaining detector-registry comment stripping regex from the public auto-detect
  boundary.
- `DetectorRegistry` no longer owns or compiles `any_comment_re:
  Regex::new(r"(?m)\s*%%.*\n").unwrap()` when the registry is constructed.
- `detect_type(...)` and `preprocess_diagram(...)` now share
  `crate::utils::cleanup_mermaid_comments(...)`.
- The shared helper follows Mermaid 11.15 `cleanupComments` source semantics:
  - remove lines whose first non-whitespace bytes are `%%`, not `%%{`, and have a non-newline
    comment body after the marker;
  - preserve `%%{...}%%` init/directive lines until directive processing;
  - trim leading blank/comment lines;
  - remove a final comment line even when it has no trailing newline.
- Added detector and preprocess regressions for indented comments, EOF comments, and directive
  preservation/removal through the existing public paths.

Evidence:

- `repo-ref/mermaid/packages/mermaid/src/diagram-api/comments.ts`
- `repo-ref/mermaid/packages/mermaid/src/diagram-api/comments.spec.ts`
- `crates/merman-core/src/utils.rs`
- `crates/merman-core/src/detect/mod.rs`
- `crates/merman-core/src/preprocess/mod.rs`
- `crates/merman-core/src/tests/detect.rs`
- `docs/quality/PANIC_SURFACE.md`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-07-hpd-050-detector-comment-cleanup-panic-surface.md`

Focused verification:

- `cargo +1.95 fmt -p merman-core` - passed.
- `cargo +1.95 nextest run -p merman-core cleanup_mermaid_comments_matches_mermaid_line_comment_shape detector_registry_strips_mermaid_comment_lines_without_regex preprocess_strips_mermaid_comment_at_eof_without_regex detector_registry_strips_deep_frontmatter_with_small_stack auto_detect_common_headers_with_deep_config_small_stack` -
  passed, `5` tests run.
- `cargo +1.95 nextest run -p merman-core detect` - passed, `19` tests run.
- `cargo +1.95 fmt --check -p merman-core` - passed.
- `git diff --check` - passed.
- `rg -n 'any_comment_re|cleanup_comments\(|Regex::new\(r"\(\?m\)\\s\*%%' crates/merman-core/src -S` -
  no detector comment-regex or duplicate local cleanup helper matches.
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check - passed,
  `794` lines.

Gate notes:

- No detector ordering, family profile, known-type parse side effect, or parser registry behavior
  changed.
- This is a public detection/preprocess panic-surface cleanup only. It does not change semantic
  models, rendered output, SVG baselines, root viewport formulas, or Architecture residual
  classification.

## HPD-050 - Preprocess CRLF Cleanup Panic Surface

Outcome:

- Removed the preprocess CRLF normalization regex from the public preprocessing boundary.
- `cleanup_text(...)` no longer calls `Regex::new(r"\r\n?").expect(...)` through the cached
  preprocess regex helper when source text contains `\r`.
- CRLF and CR-only input now flows through a small scanner that preserves Mermaid's line-ending
  normalization to `\n`.
- Public preprocessing continues to normalize line endings before frontmatter, directive, detector,
  and comment cleanup handling.

Evidence:

- `crates/merman-core/src/preprocess/mod.rs`
- `crates/merman-core/src/tests/detect.rs`
- `docs/quality/PANIC_SURFACE.md`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-07-hpd-050-preprocess-crlf-panic-surface.md`

Focused verification:

- `cargo +1.95 fmt -p merman-core` - passed.
- `cargo +1.95 nextest run -p merman-core normalize_crlf_matches_mermaid_line_ending_cleanup preprocess_normalizes_crlf_without_regex preprocess_strips_mermaid_comment_at_eof_without_regex` -
  passed, `3` tests run.
- `cargo +1.95 nextest run -p merman-core detect` - passed, `20` tests run.
- `cargo +1.95 fmt --check -p merman-core` - passed.
- `git diff --check` - passed.
- `rg -n 're_crlf|cached_regex!\(re_crlf|Regex::new\(r"\\r\\n\?' crates/merman-core/src/preprocess/mod.rs` -
  no CRLF regex helper matches.
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check - passed.

Gate notes:

- This is a public preprocessing panic-surface cleanup only. It does not change entity encoding,
  HTML attribute rewrite behavior, frontmatter/directive parsing, detector order, semantic models,
  rendered output, SVG baselines, or Architecture residual classification.

## HPD-050 - Preprocess Entity Placeholder Cleanup Panic Surface

Outcome:

- Removed the preprocess entity placeholder regexes from the public preprocessing boundary.
- `encode_mermaid_entities_like_upstream(...)` no longer calls the cached `#\w+;` entity regex or
  the integer-classification regex.
- Entity placeholder encoding now scans ASCII word bytes directly, matching Mermaid's JavaScript
  `/#\w+;/g` source shape.
- Numeric entity placeholders still receive the `ﬂ°°...¶ß` marker, while nonnumeric word
  placeholders receive `ﬂ°...¶ß`.
- Non-ASCII and non-word hash sequences such as `#é;`, `#+123;`, and `#has-dash;` remain untouched,
  matching the upstream regex boundary.

Evidence:

- `repo-ref/mermaid/packages/mermaid/src/utils.ts`
- `repo-ref/mermaid/packages/mermaid/src/mermaidAPI.spec.ts`
- `crates/merman-core/src/preprocess/mod.rs`
- `crates/merman-core/src/tests/detect.rs`
- `docs/quality/PANIC_SURFACE.md`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-07-hpd-050-preprocess-entity-panic-surface.md`

Focused verification:

- `cargo +1.95 fmt -p merman-core` - passed.
- `cargo +1.95 nextest run -p merman-core encode_entity_placeholders_matches_mermaid_ascii_word_shape preprocess_encodes_entities_without_entity_regex preprocess_normalizes_crlf_without_regex` -
  passed, `3` tests run.
- `cargo +1.95 nextest run -p merman-core detect flowchart` - passed, `117` tests run.
- `cargo +1.95 fmt --check -p merman-core` - passed.
- `git diff --check` - passed.
- `rg -n 're_entity|re_int|cached_regex!\(re_entity|cached_regex!\(re_int|Regex::new\(r"#\\w\+;"|Regex::new\(r"\^\\\+\?\\d\+\$"' crates/merman-core/src/preprocess/mod.rs` -
  no entity or integer preprocess regex helper matches.
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check - passed.

Gate notes:

- This is a public preprocessing panic-surface cleanup only. It does not change style/classDef hex
  protection, HTML attribute rewrite behavior, frontmatter/directive parsing, detector order,
  semantic models, rendered output, SVG baselines, or Architecture residual classification.

## HPD-050 - Preprocess Style Hex Protection Cleanup Panic Surface

Outcome:

- Removed the preprocess `style` / `classDef` hex-protection regexes from the public preprocessing
  boundary.
- `encode_mermaid_entities_like_upstream(...)` no longer calls cached regex helpers for
  `style.*:\S*#.*;` or `classDef.*:\S*#.*;`.
- The replacement scanner works line-by-line because Mermaid's JavaScript regex `.` does not cross
  line terminators.
- The scanner preserves the upstream greedy final-semicolon behavior: a same-line
  `style a fill:#fff; style b fill:#000;` span removes only the final semicolon, so the earlier
  `#fff;` can still flow into entity placeholder encoding.
- The scanner preserves the upstream non-match boundary when whitespace appears between `:` and
  `#`.

Evidence:

- `repo-ref/mermaid/packages/mermaid/src/utils.ts`
- `repo-ref/mermaid/packages/mermaid/src/mermaidAPI.spec.ts`
- `crates/merman-core/src/preprocess/mod.rs`
- `crates/merman-core/src/tests/detect.rs`
- `docs/quality/PANIC_SURFACE.md`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-07-hpd-050-preprocess-style-hex-panic-surface.md`

Focused verification:

- `cargo +1.95 fmt -p merman-core` - passed.
- `cargo +1.95 nextest run -p merman-core encode_entity_placeholders_matches_mermaid_ascii_word_shape preprocess_encodes_entities_without_entity_regex` -
  passed, `2` tests run.
- `cargo +1.95 nextest run -p merman-core detect flowchart` - passed, `117` tests run.
- `cargo +1.95 fmt --check -p merman-core` - passed.
- `git diff --check` - passed.
- `rg -n 're_style_hex|re_classdef_hex|cached_regex!\(re_style_hex|cached_regex!\(re_classdef_hex|Regex::new\(r"style\.\*|Regex::new\(r"classDef\.\*' crates/merman-core/src/preprocess/mod.rs` -
  no style/classDef hex-protection regex helper matches.
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check - passed.

Gate notes:

- This is a public preprocessing panic-surface cleanup only. It does not change entity placeholder
  marker semantics, HTML attribute rewrite behavior, frontmatter/directive parsing, detector order,
  semantic models, rendered output, SVG baselines, or Architecture residual classification.

## HPD-050 - Preprocess HTML Attribute Cleanup Panic Surface

Outcome:

- Removed the final cached regex helpers from the public preprocessing boundary.
- `cleanup_text(...)` no longer calls preprocess tag or attribute regex helpers for Mermaid's HTML
  cleanup pass.
- The replacement scanner preserves the upstream `/<(\w+)([^>]*)>/g` tag shape with JavaScript
  ASCII `\w` tag names.
- The attribute rewrite preserves the upstream `/="([^"]*)"/g` replacement only inside matched
  tags, including empty attribute values, first-`>` tag termination, and non-match behavior for
  non-ASCII tag names such as `<é ...>`.

Evidence:

- `repo-ref/mermaid/packages/mermaid/src/preprocess.ts`
- `crates/merman-core/src/preprocess/mod.rs`
- `crates/merman-core/src/tests/detect.rs`
- `docs/quality/PANIC_SURFACE.md`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-07-hpd-050-preprocess-html-attr-panic-surface.md`

Focused verification:

- `cargo +1.95 fmt -p merman-core` - passed.
- `cargo +1.95 nextest run -p merman-core normalize_html_tag_attributes_matches_mermaid_cleanup_shape preprocess_rewrites_html_attributes_without_regex encode_entity_placeholders_matches_mermaid_ascii_word_shape preprocess_encodes_entities_without_entity_regex preprocess_normalizes_crlf_without_regex` -
  passed, `5` tests run.
- `cargo +1.95 nextest run -p merman-core detect flowchart` - passed, `118` tests run.
- `cargo +1.95 fmt --check -p merman-core` - passed.
- `git diff --check` - passed.
- `rg -n 'cached_regex|OnceLock|Regex|regex::|re_tag|re_attr_eq_double_quoted|Regex::new' crates/merman-core/src/preprocess/mod.rs` -
  no preprocess regex helper matches.
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check - passed.

Gate notes:

- This is a public preprocessing panic-surface cleanup only. It does not change CRLF
  normalization, entity placeholder marker semantics, style/classDef hex protection,
  frontmatter/directive parsing, detector order, semantic models, rendered output, SVG baselines,
  root viewport formulas, or Architecture residual classification.

## HPD-050 - Sanitizer Line Break Cleanup Panic Surface

Outcome:

- Removed the sanitizer line-break regex compilation point from the public `sanitize_text(...)`
  boundary.
- `break_to_placeholder(...)` no longer calls a cached `Regex::new(r"(?i)<br\s*/?>")` helper.
- The replacement scanner preserves Mermaid common `lineBreakRegex = /<br\s*\/?>/gi` semantics for
  ASCII-case-insensitive `br`, JavaScript regex whitespace, optional slash, and immediate `>`.
- Added direct helper coverage plus public `sanitize_text(...)` coverage for the non-loose HTML
  label escaping path that protects `<br>` with placeholders before escaping other tags.

Evidence:

- `repo-ref/mermaid/packages/mermaid/src/diagrams/common/common.ts`
- `crates/merman-core/src/sanitize.rs`
- `docs/quality/PANIC_SURFACE.md`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-07-hpd-050-sanitize-line-break-panic-surface.md`

Focused verification:

- `cargo +1.95 fmt -p merman-core` - passed.
- `cargo +1.95 nextest run -p merman-core break_to_placeholder_matches_mermaid_line_break_regex_shape sanitize_text_preserves_mermaid_line_break_tags_without_regex sanitize` -
  passed, `28` tests run.
- `cargo +1.95 fmt --check -p merman-core` - passed.
- `git diff --check` - passed.
- `rg -n 'fn line_break_regex|line_break_regex\(\)|Regex::new\(r"\(\?i\)<br\\s\*/\?>"' crates/merman-core/src/sanitize.rs` -
  no sanitizer line-break regex helper matches.
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check - passed.

Gate notes:

- This is a public sanitizer panic-surface cleanup only. It does not change DOMPurify-like allowed
  tag/attribute policy, URI validation, script/data URL checks, Mermaid entity decoding, semantic
  parsing, rendered output, SVG baselines, root viewport formulas, or Architecture residual
  classification.

## HPD-050 - Sanitizer Attribute Entity Cleanup Panic Surface

Outcome:

- Removed five fixed regex compilation points from the sanitizer URL-attribute decoding path.
- `decode_attr_html_entities_minimally(...)` no longer compiles cached regexes for `&colon;`,
  `&newline;`, `&tab;`, `&#0*58;?`, or `&#x0*3a;?`.
- The replacement scanners preserve the existing local DOMPurify bridge behavior: named entity
  replacements run before numeric colon replacements, ASCII case-insensitive matching is preserved,
  and numeric colon references keep the previous optional-semicolon and prefix-match behavior.
- Public sanitizer coverage now proves numeric decimal and hex colon references are decoded before
  JavaScript URL validation removes unsafe links.

Evidence:

- `repo-ref/mermaid/packages/mermaid/src/diagrams/common/common.ts`
- `crates/merman-core/src/sanitize.rs`
- `docs/quality/PANIC_SURFACE.md`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-07-hpd-050-sanitize-attr-entity-panic-surface.md`

Focused verification:

- `cargo +1.95 fmt -p merman-core` - passed.
- `cargo +1.95 nextest run -p merman-core decode_attr_entities_matches_minimal_dompurify_url_subset_without_regex remove_script_decodes_colon_entities_before_url_validation_without_regex sanitize` -
  passed, `30` tests run.
- `cargo +1.95 fmt --check -p merman-core` - passed.
- `git diff --check` - passed.
- `rg -n 'colon_entity_regex|newline_entity_regex|tab_entity_regex|numeric_colon_dec_regex|numeric_colon_hex_regex|Regex::new\(r"\(\?i\)&colon;|Regex::new\(r"\(\?i\)&newline;|Regex::new\(r"\(\?i\)&tab;|Regex::new\(r"\(\?i\)&\#0\*58|Regex::new\(r"\(\?i\)&\#x0\*3a' crates/merman-core/src/sanitize.rs` -
  no sanitizer minimal-entity regex helper matches.
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check - passed.

Gate notes:

- This is a public sanitizer panic-surface cleanup only. It does not change DOMPurify-like allowed
  tag/attribute policy, URI allowlist semantics, script/data URL checks, broad HTML entity
  decoding, semantic parsing, rendered output, SVG baselines, root viewport formulas, or
  Architecture residual classification.

## HPD-050 - Sanitizer Data/ARIA Attribute Cleanup Panic Surface

Outcome:

- Removed two fixed regex compilation points from the sanitizer's DOMPurify-like `data-*` and
  `aria-*` attribute-name validation path.
- `dompurify_is_valid_attribute(...)` no longer calls cached regex helpers for DOMPurify's
  `DATA_ATTR` and `ARIA_ATTR` checks.
- The replacement scanners preserve the pinned DOMPurify 3.4.0 source shapes:
  `DATA_ATTR = /^data-[\-\w.\u00B7-\uFFFF]+$/` and `ARIA_ATTR = /^aria-[\-\w]+$/`.
- The validation order and configuration behavior are unchanged: data attributes still require
  `ALLOW_DATA_ATTR` and are blocked by `FORBID_ATTR`; ARIA attributes still require
  `ALLOW_ARIA_ATTR` and are accepted before the generated default-attribute fallback.
- Added helper-level source-boundary coverage and public `sanitize_text(...)` coverage proving
  valid `data-*` / `aria-*` names survive while invalid source-shape neighbors are removed.

Evidence:

- `repo-ref/dompurify/dist/purify.cjs.js`
- `crates/merman-core/src/sanitize.rs`
- `docs/quality/PANIC_SURFACE.md`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-07-hpd-050-sanitize-data-aria-attr-panic-surface.md`

Focused verification:

- `cargo +1.95 fmt -p merman-core` - passed.
- `cargo +1.95 nextest run -p merman-core sanitize` - passed, `31` tests run.
- `cargo +1.95 fmt --check -p merman-core` - passed.
- `git diff --check` - passed.
- `rg -n 'dompurify_(data|aria)_attr_regex|fn dompurify_.*attr_regex|Regex::new\(r"\^data-|Regex::new\(r"\^aria-' crates/merman-core/src/sanitize.rs` -
  no sanitizer data/ARIA attribute-name regex helper matches.
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check - passed.

Gate notes:

- This is a public sanitizer panic-surface cleanup only. It does not change DOMPurify generated
  allowlists, URI allowlist semantics, whitespace cleanup, script/data URL checks, minimal HTML
  entity decoding, tag policy, semantic parsing, rendered output, SVG baselines, root viewport
  formulas, or Architecture residual classification.

## HPD-050 - Sanitizer Attribute Whitespace Cleanup Panic Surface

Outcome:

- Removed one fixed regex compilation point from the sanitizer's pre-URI attribute whitespace
  cleanup path.
- `dompurify_is_valid_attribute(...)` no longer calls a cached regex helper before URI allowlist
  validation or the `ALLOW_UNKNOWN_PROTOCOLS` script/data guard.
- The replacement scanner preserves pinned DOMPurify 3.4.0 `ATTR_WHITESPACE` semantics:
  `U+0000..U+0020`, `U+00A0`, `U+1680`, `U+180E`, `U+2000..U+2029`, `U+205F`, and `U+3000`
  are removed from the parsed attribute value before URI checks.
- Added helper-level source-boundary coverage and public `sanitize_text(...)` coverage proving a
  whitespace-obfuscated `java\u00A0script:` `href` is still rejected.

Evidence:

- `repo-ref/dompurify/dist/purify.cjs.js`
- `crates/merman-core/src/sanitize.rs`
- `docs/quality/PANIC_SURFACE.md`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-07-hpd-050-sanitize-attr-whitespace-panic-surface.md`

Focused verification:

- `cargo +1.95 fmt -p merman-core` - passed.
- `cargo +1.95 nextest run -p merman-core sanitize` - passed, `33` tests run.
- `cargo +1.95 fmt --check -p merman-core` - passed.
- `git diff --check` - passed.
- `rg -n 'dompurify_attr_whitespace_regex|fn dompurify_attr_whitespace_regex|Regex::new\(r"\[\\u\{0000\}-\\u\{0020\}|Regex::new\(r"\[\\u0000-\\u0020' crates/merman-core/src/sanitize.rs` -
  no sanitizer attribute-whitespace regex helper matches.
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check - passed.

Gate notes:

- This is a public sanitizer panic-surface cleanup only. It does not change DOMPurify generated
  allowlists, data/ARIA attribute-name policy, URI allowlist regex semantics, script/data URL regex
  semantics, minimal HTML entity decoding, tag policy, semantic parsing, rendered output, SVG
  baselines, root viewport formulas, or Architecture residual classification.

## HPD-050 - Sanitizer Script/Data Guard Cleanup Panic Surface

Outcome:

- Removed one fixed regex compilation point from the sanitizer's `ALLOW_UNKNOWN_PROTOCOLS`
  script/data URI guard.
- `dompurify_is_valid_attribute(...)` no longer calls a cached regex helper for DOMPurify's
  `IS_SCRIPT_OR_DATA` check after attribute-whitespace removal.
- The replacement scanner preserves pinned DOMPurify 3.4.0 `IS_SCRIPT_OR_DATA` semantics:
  `data:` matches directly, and `\w+script:` requires at least one ASCII word character before the
  case-insensitive `script:` suffix.
- Added helper-level source-boundary coverage and public `sanitize_text(...)` coverage proving
  `ALLOW_UNKNOWN_PROTOCOLS` keeps an unknown `foo:` href while still removing `javascript:` and
  `data:` href values.

Evidence:

- `repo-ref/dompurify/dist/purify.cjs.js`
- `crates/merman-core/src/sanitize.rs`
- `docs/quality/PANIC_SURFACE.md`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-07-hpd-050-sanitize-script-data-guard-panic-surface.md`

Focused verification:

- `cargo +1.95 fmt -p merman-core` - passed.
- `cargo +1.95 nextest run -p merman-core sanitize` - passed, `35` tests run.
- `cargo +1.95 fmt --check -p merman-core` - passed.
- `git diff --check` - passed.
- `rg -n 'dompurify_is_script_or_data_regex|fn dompurify_is_script_or_data_regex|Regex::new\(r"\(\?i\)\^\(\?:\\w\+script\|data\):' crates/merman-core/src/sanitize.rs` -
  no sanitizer script/data guard regex helper matches.
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check - passed.

Gate notes:

- This is a public sanitizer panic-surface cleanup only. It does not change DOMPurify generated
  allowlists, data/ARIA attribute-name policy, URI allowlist regex semantics, attribute-whitespace
  cleanup, minimal HTML entity decoding, tag policy, semantic parsing, rendered output, SVG
  baselines, root viewport formulas, or Architecture residual classification.

## HPD-050 - Sanitizer URI Allowlist Cleanup Panic Surface

Outcome:

- Removed the final sanitizer regex compilation point from the DOMPurify-like URI allowlist path.
- `dompurify_is_valid_attribute(...)` now calls `is_dompurify_allowed_uri(...)` instead of a cached
  `Regex::new(...)` helper, and `crates/merman-core/src/sanitize.rs` no longer imports
  `regex::Regex`.
- The replacement scanner preserves pinned DOMPurify 3.4.0 `IS_ALLOWED_URI` semantics, including
  safe schemes, relative-like non-letter starts, and source-shaped ASCII scheme-prefix fallback.
- This slice intentionally aligns the default sanitizer with pinned DOMPurify 3.4.0 by allowing
  `matrix:` URIs, which the previous Rust regex omitted.
- Added helper-level source-boundary coverage and public `sanitize_text(...)` coverage proving
  `matrix:` survives while default unknown `foo:` href remains stripped.

Evidence:

- `repo-ref/dompurify/dist/purify.cjs.js`
- `crates/merman-core/src/sanitize.rs`
- `docs/quality/PANIC_SURFACE.md`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-07-hpd-050-sanitize-uri-allowlist-panic-surface.md`

Focused verification:

- `cargo +1.95 fmt -p merman-core` - passed.
- `cargo +1.95 nextest run -p merman-core sanitize` - passed, `37` tests run.
- `cargo +1.95 fmt --check -p merman-core` - passed.
- `git diff --check` - passed.
- `rg -n 'Regex|regex::|dompurify_is_allowed_uri_regex|fn dompurify_is_allowed_uri_regex' crates/merman-core/src/sanitize.rs` -
  no sanitizer regex dependency or URI allowlist regex helper matches.
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check - passed.

Gate notes:

- This is a source-backed URI allowlist convergence and public sanitizer panic-surface cleanup. It
  intentionally changes default URI acceptance for `matrix:` to match pinned DOMPurify 3.4.0. It
  does not change DOMPurify generated allowlists, data/ARIA attribute-name policy,
  attribute-whitespace cleanup, script/data guard semantics, minimal HTML entity decoding, tag
  policy, semantic parsing, rendered output, SVG baselines, root viewport formulas, or Architecture
  residual classification.

## HPD-050 - sanitize_url Cleanup Regex Panic Surface

Outcome:

- Removed the remaining two fixed regex compilation points from the public `sanitize_url(...)`
  cleanup loop.
- `crates/merman-core/src/utils.rs` no longer imports `regex::Regex` or stores cached
  `html_ctrl_entity_regex(...)` / `whitespace_escape_chars_regex(...)` helpers.
- The named-control-entity scanner preserves installed `@braintree/sanitize-url` 7.1.2
  `htmlCtrlEntityRegex = /&(newline|tab);/gi` semantics for repeated cleanup-loop stripping.
- The whitespace-escape scanner preserves installed `@braintree/sanitize-url` 7.1.2
  `whitespaceEscapeCharsRegex = /(\\|%5[cC])((%(6[eE]|72|74))|[nrt])/g` semantics, including
  `%5c` / `%5C` backslash encodings, `%6e` / `%6E` newline encodings, `%72` / `%74`, and the
  source's lowercase literal `[nrt]` branch.
- Existing public sanitize-url attack-vector coverage stayed green, and helper-level tests now
  cover the scanner boundaries directly.

Evidence:

- `tools/mermaid-cli/node_modules/@braintree/sanitize-url/src/constants.ts`
- `tools/mermaid-cli/node_modules/@braintree/sanitize-url/src/index.ts`
- `tools/mermaid-cli/node_modules/@braintree/sanitize-url/src/__tests__/index.test.ts`
- `tools/mermaid-cli/package-lock.json`
- `crates/merman-core/src/utils.rs`
- `docs/quality/PANIC_SURFACE.md`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-07-hpd-050-sanitize-url-cleanup-regex-panic-surface.md`

Focused verification:

- `cargo +1.95 fmt -p merman-core` - passed.
- `cargo +1.95 nextest run -p merman-core sanitize_url` - passed, `3` tests run.
- `cargo +1.95 nextest run -p merman-core sanitize` - passed, `39` tests run.
- `cargo +1.95 fmt --check -p merman-core` - passed.
- `git diff --check` - passed with the existing `CONTEXT.jsonl` LF/CRLF conversion warning.
- `rg -n 'html_ctrl_entity_regex|whitespace_escape_chars_regex|Regex|regex::' crates/merman-core/src/utils.rs` -
  no sanitize-url regex dependency or cleanup regex helper matches.
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check - passed, `829`
  lines parsed.

Gate notes:

- This is a source-backed public URL sanitizer panic-surface cleanup. It does not change the
  DOMPurify-like `sanitize_text(...)` sanitizer boundary, DOMPurify generated allowlists,
  data/ARIA attribute-name policy, URI allowlist semantics, attribute-whitespace cleanup,
  script/data guard semantics, Mermaid preprocessing, semantic parsing, rendered output, SVG
  baselines, root viewport formulas, or Architecture residual classification.

## HPD-050 - RaTeX Math Label Line-Break Regex Panic Surface

Outcome:

- Removed the feature-gated static `<br>` regex compilation from
  `RatexMathRenderer::math_only_lines(...)`.
- The pure-math RaTeX label path now reuses `crate::text::split_html_br_lines(...)`, the existing
  direct scanner for Mermaid common `lineBreakRegex = /<br\s*\/?>/gi`.
- This aligns the pure-math path with the mixed KaTeX-like path in the same module, which already
  used `split_html_br_lines(...)`.
- Added feature-gated coverage for uppercase `<BR />`, whitespace before the optional slash, and a
  non-matching `<brx>` lookalike.

Evidence:

- `repo-ref/mermaid/packages/mermaid/src/diagrams/common/common.ts`
- `crates/merman-render/src/text/wrap.rs`
- `crates/merman-render/src/math.rs`
- `docs/quality/PANIC_SURFACE.md`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-07-hpd-050-ratex-math-line-break-regex-panic-surface.md`

Focused verification:

- `cargo +1.95 fmt -p merman-render` - passed.
- `cargo +1.95 nextest run -p merman-render --features ratex-math ratex_math_renderer` -
  passed, `4` tests run.
- `cargo +1.95 fmt --check -p merman-render` - passed.
- `git diff --check` - passed with the existing `CONTEXT.jsonl` LF/CRLF conversion warning.
- `rg -n 'LINE_BREAK_RE|Regex::new\(r"\(\?i\)<br|regex::Regex|<br\\s' crates/merman-render/src/math.rs` -
  no RaTeX math line-break regex helper matches.
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check - passed, `832`
  lines parsed.

Gate notes:

- This is a render-path panic-surface cleanup for optional RaTeX math labels. It does not change
  the shared text wrapping scanner, Node/KaTeX renderer probing, non-math labels, Mermaid
  preprocessing, core sanitization, semantic parsing, SVG baselines, root viewport formulas, or
  Architecture residual classification.

## HPD-050 - ClassDB Member/accDescr Regex Panic Surface

Outcome:

- Removed the remaining ClassDB-local regex compilation points from
  `crates/merman-core/src/diagrams/class/db.rs`.
- Replaced the method member fallback regex with a source-shaped scanner for Mermaid 11.15
  `ClassMember.parseMember(...)`:
  `([#+~-])?(.+)\((.*)\)([\s$*])?(.*)([$*])?`.
- The scanner preserves the upstream greedy boundary by using the last `(` before the last `)` for
  method parameters, so names containing earlier parentheses remain part of the method id.
- Replaced class multiline `accDescr` `\n\s+` replacement with a direct scanner that collapses
  indentation after newlines.
- Added public parser coverage for the greedy method boundary and multiline accessibility
  description normalization.

Evidence:

- `repo-ref/mermaid/packages/mermaid/src/diagrams/class/classTypes.ts`
- `repo-ref/mermaid/packages/mermaid/src/diagrams/class/classDiagram.spec.ts`
- `crates/merman-core/src/diagrams/class/db.rs`
- `crates/merman-core/src/tests/class.rs`
- `docs/quality/PANIC_SURFACE.md`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-07-hpd-050-classdb-regex-panic-surface.md`

Focused verification:

- `cargo +1.95 fmt -p merman-core` - passed.
- `cargo +1.95 nextest run -p merman-core parse_diagram_class_method_parser_matches_upstream_greedy_regex_boundary parse_diagram_class_acc_descr_multiline_collapses_newline_whitespace_without_regex` -
  passed, `2` tests run.
- `cargo +1.95 nextest run -p merman-core class` - passed, `49` tests run.
- `cargo +1.95 fmt --check -p merman-core` - passed.
- `git diff --check` - passed with the existing `CONTEXT.jsonl` LF/CRLF conversion warning.
- `rg -n 'Regex|regex::|OnceLock|METHOD_RE|ACC_DESCR_RE|class method regex|class acc descr regex' crates/merman-core/src/diagrams/class/db.rs` -
  no ClassDB regex dependency or helper matches.
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check - passed, `835`
  lines parsed.

Gate notes:

- This is a source-backed Class parser panic-surface cleanup and a method parser boundary
  convergence. It does not change Class layout, renderer SVG output, namespace semantics,
  link/callback behavior, common sanitizer policy, Gantt date parsing, retained config projection,
  SVG baselines, root viewport formulas, or Architecture residual classification.

## HPD-050 - Gantt Date/Duration Regex Panic Surface

Outcome:

- Removed the remaining Gantt-local regex compilation points from
  `crates/merman-core/src/diagrams/gantt/mod.rs` and
  `crates/merman-core/src/diagrams/gantt/date.rs`.
- Replaced `DIGITS_RE` with an ASCII digit scanner matching Mermaid's JavaScript `/^\d+$/`
  timestamp/date-fallback checks.
- Replaced `AFTER_RE` and `UNTIL_RE` with a source-shaped scanner for pinned Mermaid 11.15
  `ganttDb.js` `^after\s+(?<ids>[\d\w- ]+)` and
  `^until\s+(?<ids>[\d\w- ]+)` semantics.
- Preserved the upstream case-sensitive keyword boundary, JavaScript `\s+` whitespace after the
  keyword, ASCII word / hyphen / space ID capture, and non-anchored trailing behavior.
- Replaced `DURATION_RE` with a direct scanner for
  `^(\d+(?:\.\d+)?)([Mdhmswy]|ms)$`, preserving invalid duration fallback to `[NaN, 'ms']`.
- Replaced `STRICT_YYYY_MM_DD_RE` with a byte-shape check followed by the existing `NaiveDate`
  calendar validation.
- Added focused Gantt coverage for duration boundary failures, `_` / `-` relative IDs,
  source-regex whitespace backtracking, and source-case-sensitive `after` / `until` keyword
  behavior.

Evidence:

- `repo-ref/mermaid/packages/mermaid/src/diagrams/gantt/ganttDb.js`
- `repo-ref/mermaid/packages/mermaid/src/diagrams/gantt/ganttDb.spec.ts`
- `crates/merman-core/src/diagrams/gantt/mod.rs`
- `crates/merman-core/src/diagrams/gantt/date.rs`
- `crates/merman-core/src/diagrams/gantt/tests.rs`
- `docs/quality/PANIC_SURFACE.md`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-07-hpd-050-gantt-regex-panic-surface.md`

Focused verification:

- `cargo +1.95 fmt -p merman-core` - passed.
- `cargo +1.95 nextest run -p merman-core gantt` - passed, `45` tests run.
- `cargo +1.95 fmt --check -p merman-core` - passed.
- `git diff --check` - passed.
- `rg -n 'regex::Regex|Regex::new|OnceLock<Regex>|OnceLock\s*<\s*Regex' crates/merman-core/src -g '*.rs'` -
  no production core regex compile/cache matches.
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check - passed.

Gate notes:

- This is a source-backed Gantt parser/date panic-surface cleanup and completes the currently known
  production `merman-core/src` regex compilation cleanup. It does not change Gantt layout,
  renderer SVG output, section/task ordering, weekend/exclude behavior, common sanitizer policy,
  retained config projection, SVG baselines, root viewport formulas, or Architecture residual
  classification.

## HPD-050 - FontAwesome Icon Regex Panic Surface

Outcome:

- Removed the static regex compilation point from `replace_fontawesome_icons(...)` in
  `crates/merman-render/src/text/icons.rs`.
- Replaced Mermaid `/(fa[bklrs]?):fa-([\w-]+)/g` icon-token replacement with a direct scanner for
  `fa` plus optional `b/k/l/r/s`, literal `:fa-`, and ASCII word / hyphen icon names.
- Preserved non-anchored global replacement behavior and the existing local double-quoted
  `<i class="...">` fallback output shape used by current SVG baselines.
- Added focused text coverage for the upstream `fa`, `fab`, `fak`, and `fas` examples plus
  unsupported-prefix, empty-icon, non-ASCII, and inside-string matching boundaries.

Evidence:

- `repo-ref/mermaid/packages/mermaid/src/rendering-util/createText.ts`
- `repo-ref/mermaid/packages/mermaid/src/rendering-util/createText.spec.ts`
- `crates/merman-render/src/text/icons.rs`
- `crates/merman-render/src/text/tests.rs`
- `docs/quality/PANIC_SURFACE.md`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-07-hpd-050-fontawesome-icon-regex-panic-surface.md`

Focused verification:

- `cargo +1.95 fmt -p merman-render` - passed.
- `cargo +1.95 nextest run -p merman-render fontawesome` - passed, `7` tests run.
- `rg -n 'Regex|regex::|OnceLock|fontawesome_icon_at|replace_fontawesome_icons' crates/merman-render/src/text/icons.rs crates/merman-render/src/text/tests.rs` -
  no regex dependency matches in `text/icons.rs`; scanner and tests were the only relevant hits.
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check - passed, `843`
  lines parsed.

Gate notes:

- This is a source-backed render text-label panic-surface cleanup. It does not change registered
  icon pack resolution, Flowchart icon-shape nodes, HTML label measurement heuristics, SVG
  baselines, root viewport formulas, core parsing, sanitizer policy, or Architecture residual
  classification.

## HPD-050 - CSS Important Regex Panic Surface

Outcome:

- Removed the static regex compilation point from `CssOverridePostprocessor` in
  `crates/merman-render/src/svg/pipeline/builtin/css_override.rs`.
- Replaced local `(?i)\s*!important\b` replacement with a direct scanner anchored on `!`, removing
  contiguous whitespace immediately before case-insensitive `!important`.
- Preserved the previous word-boundary behavior after `important`: `!importantfoo` and
  `!importanté` remain untouched, while `!important-border` strips the marker and leaves
  `-border`.
- Kept `CssOverridePolicy::Preserve` behavior unchanged and verified the scoped CSS caller that
  reuses `strip_css_important(...)`.

Evidence:

- `crates/merman-render/src/svg/pipeline/builtin/css_override.rs`
- `crates/merman-render/src/svg/pipeline/builtin/scoped_css.rs`
- `docs/quality/PANIC_SURFACE.md`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-07-hpd-050-css-important-regex-panic-surface.md`

Focused verification:

- `cargo +1.95 fmt -p merman-render` - passed.
- `cargo +1.95 nextest run -p merman-render important` - passed, `3` tests run.
- `rg -n 'Regex|regex::|OnceLock|css_important|strip_css_important' crates/merman-render/src/svg/pipeline/builtin/css_override.rs crates/merman-render/src/svg/pipeline/builtin/scoped_css.rs` -
  no regex dependency matches in `css_override.rs`; scanner and call sites were the only relevant
  hits.
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check - passed, `846`
  lines parsed.

Gate notes:

- This is a local SVG pipeline panic-surface cleanup. It does not change CSS override policy
  selection, scoped CSS injection syntax, SVG baseline content outside existing-important
  stripping, core parsing, sanitizer policy, root viewport formulas, or Architecture residual
  classification.

## HPD-050 - CSS Sanitize Regex Panic Surface

Outcome:

- Removed the remaining static regex compilation points from SVG pipeline CSS sanitization in
  `crates/merman-render/src/svg/pipeline/builtin/css_sanitize.rs`.
- Replaced local animation declaration stripping for
  `(?i)(^|[;{])\s*animation(?:-[a-z-]+)?\s*:[^;}]*;?` with a delimiter-aware scanner that
  preserves the previous start / `;` / `{` match boundary and keeps the delimiter in output.
- Replaced local CSS degree-unit stripping for `(?i)(-?\d+(?:\.\d+)?)deg\b` with a direct scanner
  that preserves optional negative signs, decimal fractions, case-insensitive `deg`, and the
  previous trailing word-boundary behavior.
- Added focused coverage for animation suffix boundaries, delimiter preservation, `.5deg`
  substring matching, hyphen-followed `deg` stripping, and non-ASCII word-boundary non-matches.

Evidence:

- `crates/merman-render/src/svg/pipeline/builtin/css_sanitize.rs`
- `crates/merman-render/src/svg/pipeline/builtin/attr_sanitize.rs`
- `docs/quality/PANIC_SURFACE.md`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-07-hpd-050-css-sanitize-regex-panic-surface.md`

Focused verification:

- `cargo +1.95 fmt --check -p merman-render` - passed.
- `cargo +1.95 nextest run -p merman-render css_sanitize resvg_safe` - passed, `4` tests run.
- `rg -n 'Regex|regex::|OnceLock' crates/merman-render/src/svg/pipeline/builtin/css_sanitize.rs` -
  no regex dependency matches in `css_sanitize.rs`.
- `git diff --check` - passed.
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check - passed.

Gate notes:

- This is a local SVG pipeline panic-surface cleanup for raster-safe CSS sanitization. It does not
  change unsupported-rule filtering, style-element scanning, attribute sanitization, CSS override
  policy, scoped CSS injection, core parsing, sanitizer policy, SVG baselines, root viewport
  formulas, or Architecture residual classification. The remaining production render regex cluster
  is now in `crates/merman-render/src/svg/pipeline/builtin/attr_sanitize.rs`.

## HPD-050 - SVG Attribute Sanitize Regex Panic Surface

Outcome:

- Removed the remaining double-quoted SVG attribute regex compilation points from
  `crates/merman-render/src/svg/pipeline/builtin/attr_sanitize.rs`.
- Replaced local `\s+([A-Za-z_:][-A-Za-z0-9_:.]*)\s*=\s*"([^"]*)"` matching with a shared
  direct scanner used by both full tag attribute rewriting and `attr_value(...)` bad-`rect`
  dimension lookup.
- Preserved the previous scanner shape: at least one Unicode whitespace before the attribute,
  ASCII SVG-like attribute names, optional whitespace around `=`, double-quoted values, and
  no handling for single-quoted or unquoted attributes.
- Added focused coverage for unchanged attribute formatting, px normalization, empty guarded
  attribute dropping, style sanitization, and bad-`rect` detection through spaced uppercase
  attributes.

Evidence:

- `crates/merman-render/src/svg/pipeline/builtin/attr_sanitize.rs`
- `crates/merman-render/src/svg/pipeline/builtin/css_sanitize.rs`
- `crates/merman-render/src/svg/parity/er.rs`
- `docs/quality/PANIC_SURFACE.md`
- `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-07-hpd-050-attr-sanitize-regex-panic-surface.md`

Focused verification:

- `cargo +1.95 fmt -p merman-render` - passed.
- `cargo +1.95 nextest run -p merman-render attr_sanitize resvg_safe` - passed, `6` tests run.
- `cargo +1.95 fmt --check -p merman-render` - passed.
- `rg -n 'Regex|regex::|OnceLock' crates/merman-render/src/svg/pipeline/builtin/attr_sanitize.rs crates/merman-render/src/svg/pipeline/builtin/css_sanitize.rs crates/merman-render/src/svg/pipeline/builtin/css_override.rs` -
  no regex dependency matches in those builtin SVG sanitizer files.
- `rg -n "regex::Regex|Regex::new|OnceLock<regex::Regex>|OnceLock\s*<\s*Regex|regex::Captures|Captures<'" crates/merman-render/src -g '*.rs'` -
  reports only `crates/merman-render/src/svg/parity/er.rs`.
- `git diff --check` - passed.
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check - passed.

Gate notes:

- This is a local SVG pipeline panic-surface cleanup for raster-safe attribute sanitization. It
  does not change guarded attribute policy, invalid value policy, style declaration filtering,
  CSS override policy, scoped CSS injection, core parsing, sanitizer policy, SVG baselines, root
  viewport formulas, or Architecture residual classification. After this slice, the remaining
  precise `regex::Regex` / `Regex::new` render hit is the ER parity decimal-normalization helper
  in `crates/merman-render/src/svg/parity/er.rs`.
