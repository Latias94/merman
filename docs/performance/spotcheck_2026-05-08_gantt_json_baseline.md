# Gantt JSON-Fallback Baseline

This report captures the current JSON-fallback baseline before a future gantt typed render-model
migration. Gantt has date and timezone-sensitive semantics, so this baseline should be preserved
before changing the render-model boundary.

## Parameters

- Date: 2026-05-08
- Git commit: `de526e54`
- Rust: `rustc 1.87.0 (17067e9ac 2025-05-09)`, host `x86_64-pc-windows-msvc`
- Fixture: `gantt_medium`
- Raw output: `target/bench/gantt_json_baseline_2026-05-08.txt`
- Criterion options: `--noplot --sample-size 20 --warm-up-time 1 --measurement-time 1`

## Command

```text
cargo bench -p merman --features render --bench pipeline gantt_medium -- --noplot --sample-size 20 --warm-up-time 1 --measurement-time 1
```

## Mid Estimates

| bench | current JSON-fallback baseline |
| --- | ---: |
| `parse/gantt_medium` | 60.492 us |
| `parse_known_type/gantt_medium` | 64.446 us |
| `layout/gantt_medium` | 32.664 us |
| `render/gantt_medium` | 30.379 us |
| `end_to_end/gantt_medium` | 151.95 us |

## Notes

- This is the pre-migration baseline for `parse_diagram_for_render_model_sync` while gantt still
  uses `RenderSemanticModel::Json`.
- Criterion reported no significant change for parse, parse-known-type, layout, and end-to-end
  against local historical samples.
- Criterion reported a render-only regression against local historical samples. Treat that as a
  signal to re-run render after any gantt renderer changes; it is not caused by a gantt typed model
  migration because that migration has not started yet.
- `treemap_medium` and `xychart_medium` were still skipped by the filtered pipeline bench
  pre-checks; this matches the existing pipeline bench behavior.
