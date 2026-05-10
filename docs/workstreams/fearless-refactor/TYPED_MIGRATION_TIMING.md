# Typed Migration Timing Index

This index collects the timing evidence for typed render-model migrations and the follow-up
canaries that proved the newer paths stayed measurable.

Use this as the navigation layer for the individual reports in `docs/performance/`.

## Typed Migration Reports

| diagram | evidence | sample type | note |
| --- | --- | --- | --- |
| `sequence` | `docs/performance/spotcheck_2026-05-07_sequence_typed_render_model.md` | post-migration baseline | First typed render-model anchor. |
| `kanban` | `docs/performance/spotcheck_2026-05-08_kanban_typed_render_model.md` | parent-vs-typed Criterion pair | Render path stayed stable while parse dropped sharply. |
| `gantt` | `docs/performance/spotcheck_2026-05-08_gantt_json_baseline.md` / `docs/performance/spotcheck_2026-05-08_gantt_typed_render_model.md` | pre/post migration pair | Captures the JSON-fallback baseline and the typed follow-up. |
| `pie` | `docs/performance/spotcheck_2026-05-08_pie_typed_render_model.md` | parent-vs-typed Criterion pair | Small-diagram typed migration baseline. |
| `packet` | `docs/performance/spotcheck_2026-05-08_packet_typed_render_model.md` | parent-vs-typed Criterion pair | Typed layout/render path anchor. |
| `timeline` | `docs/performance/spotcheck_2026-05-08_timeline_typed_render_model.md` | parent-vs-typed Criterion pair | Moderate small-diagram typed migration anchor. |
| `journey` | `docs/performance/spotcheck_2026-05-08_journey_typed_render_model.md` | parent-vs-typed Criterion pair | Actor/task typed render-model anchor. |
| `requirement` | `docs/performance/spotcheck_2026-05-08_requirement_typed_render_model.md` | parent-vs-typed Criterion pair | Typed layout/render path anchor. |
| `sankey` | `docs/performance/spotcheck_2026-05-08_sankey_typed_render_model.md` | parent-vs-typed Criterion pair | Layout-only SVG path anchor. |
| `radar` | `docs/performance/spotcheck_2026-05-08_radar_typed_render_model.md` | parent-vs-typed Criterion pair | Typed layout/render path anchor. |
| `info` | `docs/performance/spotcheck_2026-05-08_info_typed_render_model.md` | JSON-fallback-vs-typed pair | Fixture-added migration anchor. |
| `zenuml` | `docs/performance/spotcheck_2026-05-08_zenuml_typed_render_model.md` | parent-vs-typed Criterion pair | Render-only translation into the sequence model. |
| `quadrantChart` | `docs/performance/spotcheck_2026-05-08_quadrant_chart_typed_render_model.md` | parent-vs-typed Criterion pair | Typed layout/render path anchor. |
| `gitGraph` | `docs/performance/spotcheck_2026-05-08_gitgraph_typed_render_model.md` | parent-vs-typed Criterion pair | Typed layout/render path anchor. |
| `treemap` | `docs/performance/spotcheck_2026-05-08_treemap_typed_render_model.md` | parent-vs-typed Criterion pair | Typed layout anchor after the benchmark fixture repair. |
| `block` | `docs/performance/spotcheck_2026-05-08_block_typed_render_model.md` | parent-vs-typed Criterion pair | Typed layout/render path anchor. |
| `er` | `docs/performance/spotcheck_2026-05-08_er_typed_render_model.md` | parent-vs-typed Criterion pair | Typed layout/render path anchor. |
| `c4` | `docs/performance/spotcheck_2026-05-08_c4_typed_render_model.md` / `docs/performance/spotcheck_2026-05-09_c4_direct_render_model_parse.md` | post-migration typed path + direct-parse cleanup | Tracks the typed render path and the direct `C4Db` parse cleanup. |
| `xychart` | `docs/performance/spotcheck_2026-05-08_xychart_typed_render_model.md` | post-migration typed render path | SVG emission stays layout-only; follow-up cleanup is tracked separately. |

## Cross-Diagram Canaries

| area | evidence | sample type | note |
| --- | --- | --- | --- |
| `c4` / `xychart` | `docs/performance/spotcheck_2026-05-09_c4_xychart_mmdr_comparison.md` | same-machine cross-repo comparison | Tracks local merman vs `mermaid-rs-renderer` movement. |
| `c4` / `xychart` | `docs/performance/spotcheck_2026-05-09_c4_xychart_stage_mmdr.md` | stage attribution | Breaks the comparison down into parse/layout/render. |
| `xychart` | `docs/performance/spotcheck_2026-05-09_xychart_render_allocation_cleanup.md` | follow-up cleanup spotcheck | Confirms the render allocation reduction stayed green. |
| `xychart` | `docs/performance/spotcheck_2026-05-09_xychart_layout_tick_cache.md` | follow-up cleanup spotcheck | Confirms the layout tick-cache cleanup stayed green. |
| `mindmap` / `architecture` / `c4` | `docs/performance/spotcheck_2026-05-09_mindmap_architecture_c4_stage_mmdr.md` | stage attribution | Highlights the current Architecture layout gap. |
| `mindmap` / `architecture` | `docs/performance/spotcheck_2026-05-10_mindmap_architecture_canary_pipeline.md` | short local canary | Quick local triage note. |
| `mindmap` / `architecture` | `docs/performance/spotcheck_2026-05-10_mindmap_architecture_canary_pipeline_long.md` | longer local canary | Default local checkpoint for the pair. |
| `mindmap` / `architecture` | `docs/performance/spotcheck_2026-05-10_standard_canaries_stage_mmdr_toolchain.md` | stage attribution with explicit toolchain | Confirms the current local layout-stage signal under the pinned mmdr toolchain. |

## Extension Rule

When the next typed migration lands:

1. Capture a same-machine baseline before the migration.
2. Capture the post-migration typed path on the same machine.
3. Append the new report here and link it from `TODO.md` and `MILESTONES.md`.

