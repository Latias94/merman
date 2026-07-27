# Gantt Typed Render Model Spotcheck

This report captures a same-machine Criterion spotcheck for the gantt typed render-model
migration. The goal is to confirm that the render-only path avoids semantic JSON transport without
moving the stable `parse_diagram_sync` JSON API.

## Parameters

- Date: 2026-05-08
- JSON baseline commit: `de526e54`
- Typed worktree base: `dd0d7fff` plus the gantt typed render-model change set
- Rust: `rustc 1.87.0 (17067e9ac 2025-05-09)`, host `x86_64-pc-windows-msvc`
- Fixture: `gantt_medium`
- Criterion options: `--noplot --sample-size 20 --warm-up-time 1 --measurement-time 1`
- Baseline report: `docs/performance/spotcheck_2026-05-08_gantt_json_baseline.md`

## Command

```text
cargo bench -p merman --features render --bench pipeline gantt_medium -- --noplot --sample-size 20 --warm-up-time 1 --measurement-time 1
```

## Mid Estimates

| bench | JSON render-model baseline | typed render model | change |
| --- | ---: | ---: | ---: |
| `parse/gantt_medium` | 60.492 us | 29.175 us | -51.8% |
| `parse_known_type/gantt_medium` | 64.446 us | 62.321 us | -3.3% |
| `layout/gantt_medium` | 32.664 us | 35.916 us | +10.0% |
| `render/gantt_medium` | 30.379 us | 21.824 us | -28.2% |
| `end_to_end/gantt_medium` | 151.95 us | 85.661 us | -43.6% |

## Interpretation

- `parse/gantt_medium` measures `parse_diagram_for_render_model_sync`, so it captures the intended
  render-only migration from `RenderSemanticModel::Json` to `GanttDiagramRenderModel`.
- `parse_known_type/gantt_medium` still measures the stable semantic JSON API, so it is not
  expected to materially improve.
- `layout/gantt_medium` now consumes the typed model directly. This run reports no significant
  Criterion layout change even though the midpoint is slightly higher than the saved JSON baseline.
- `render/gantt_medium` improves because SVG render-model dispatch no longer deserializes a
  private gantt semantic model from JSON.
- `end_to_end/gantt_medium` improves through the render-model parse path plus the SVG typed path.

## Verification

- `cargo fmt --check`
- `cargo check --workspace --all-features`
- `cargo nextest run -p merman-core gantt`
- `cargo nextest run -p merman-render gantt`
- `cargo run -p xtask -- compare-gantt-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo run -p xtask -- verify --strict`
