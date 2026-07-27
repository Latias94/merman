# Radar Typed Render Model Spotcheck

This report captures a same-machine Criterion spotcheck for the radar typed render-model
migration. Radar previously had three semantic transport shapes: parser JSON construction in
`merman-core`, private layout structs in `merman-render`, and a small private SVG semantic model for
title/accessibility/legend labels.

## Parameters

- Date: 2026-05-08
- Parent JSON baseline commit: `d9192a38`
- Typed worktree base: `d9192a38` plus the radar typed render-model change set
- Rust: `rustc 1.87.0 (17067e9ac 2025-05-09)`, host `x86_64-pc-windows-msvc`
- Fixture: `radar_medium`
- Criterion options: `--noplot --sample-size 20 --warm-up-time 1 --measurement-time 1`

## Commands

Parent JSON baseline:

```text
cargo bench -p merman --features render --bench pipeline radar_medium -- --noplot --sample-size 20 --warm-up-time 1 --measurement-time 1
```

Typed worktree:

```text
cargo bench -p merman --features render --bench pipeline radar_medium -- --noplot --sample-size 20 --warm-up-time 1 --measurement-time 1
```

The typed worktree was run more than once while restoring the semantic JSON materializer to a
hand-built object path. The final table uses the confirmation run after compilation had already
finished.

## Mid Estimates

| bench | parent JSON render model | typed render model | change |
| --- | ---: | ---: | ---: |
| `parse/radar_medium` | 103.90 us | 4.7683 us | -95.4% |
| `parse_known_type/radar_medium` | 118.62 us | 99.679 us | -16.0% |
| `layout/radar_medium` | 10.863 us | 11.673 us | +7.5% |
| `render/radar_medium` | 12.947 us | 13.506 us | +4.3% |
| `end_to_end/radar_medium` | 134.16 us | 29.070 us | -78.3% |

## Interpretation

- `parse/radar_medium` improves because `parse_diagram_for_render_model_sync` now returns
  `RadarDiagramRenderModel` without constructing the full semantic JSON object tree.
- `parse_known_type/radar_medium` still exercises the semantic JSON API, but it benefits from the
  shared parser DB while keeping the JSON payload shape stable.
- `layout/radar_medium` and `render/radar_medium` show small midpoint drift in this sample. Radar
  layout now reads numeric entries from JSON-valued typed fields; keep this visible before calling
  radar layout optimized.
- `end_to_end/radar_medium` improves because render-only parse savings dominate the small
  layout/render drift.

## Verification

- `cargo fmt`
- `cargo check -p merman-core -p merman-render --all-features`
- `cargo nextest run -p merman-core radar`
- `cargo nextest run -p merman-render radar`
- `cargo run -p xtask -- compare-radar-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo clippy -p merman-core -p merman-render --all-targets --all-features -- -D warnings`
- `cargo run -p xtask -- verify --strict`
