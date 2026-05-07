# Timeline Typed Render Model Spotcheck

This report captures a same-machine Criterion spotcheck for the timeline typed render-model
migration. Timeline is a moderate small-diagram migration: the render-only path previously parsed
timeline data into semantic JSON and then deserialized it back into private renderer structs for
layout.

## Parameters

- Date: 2026-05-08
- Parent JSON baseline commit: `d01bc944`
- Typed worktree base: `d01bc944` plus the timeline typed render-model change set
- Rust: `rustc 1.87.0 (17067e9ac 2025-05-09)`, host `x86_64-pc-windows-msvc`
- Fixture: `timeline_medium`
- Criterion options: `--noplot --sample-size 20 --warm-up-time 1 --measurement-time 1`

## Commands

Parent JSON baseline:

```text
cargo bench -p merman --features render --bench pipeline timeline_medium -- --noplot --sample-size 20 --warm-up-time 1 --measurement-time 1
```

Typed worktree:

```text
cargo bench -p merman --features render --bench pipeline timeline_medium -- --noplot --sample-size 20 --warm-up-time 1 --measurement-time 1
```

## Mid Estimates

| bench | parent JSON render model | typed render model | change |
| --- | ---: | ---: | ---: |
| `parse/timeline_medium` | 13.844 us | 3.1427 us | -77.3% |
| `parse_known_type/timeline_medium` | 17.154 us | 8.3332 us | -51.4% |
| `layout/timeline_medium` | 57.117 us | 60.238 us | +5.5% |
| `render/timeline_medium` | 31.177 us | 33.476 us | +7.4% |
| `end_to_end/timeline_medium` | 116.86 us | 95.275 us | -18.5% |

## Interpretation

- `parse/timeline_medium` improves because `parse_diagram_for_render_model_sync` now returns
  `TimelineDiagramRenderModel` instead of constructing semantic JSON for render-only callers.
- `parse_known_type/timeline_medium` still exercises the semantic JSON API, but it benefits from
  sharing typed task construction before serializing the stable JSON payload.
- `layout/timeline_medium` and `render/timeline_medium` were slightly slower in this sample. The
  absolute deltas are small relative to text measurement and SVG construction, so keep watching
  these benches before making another timeline-specific optimization.
- `end_to_end/timeline_medium` still improves because parse savings dominate the small layout/render
  midpoint drift.

## Verification

- `cargo fmt`
- `cargo check -p merman-core -p merman-render --all-features`
- `cargo nextest run -p merman-core timeline`
- `cargo nextest run -p merman-render timeline`
- `cargo clippy -p merman-core -p merman-render --all-targets --all-features -- -D warnings`
- `cargo run -p xtask -- compare-timeline-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo run -p xtask -- verify --strict`
