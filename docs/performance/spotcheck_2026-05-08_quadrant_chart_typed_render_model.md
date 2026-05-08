# Quadrant Chart Typed Render Model Spotcheck

This report captures a same-machine Criterion spotcheck for the quadrant chart typed render-model
migration. Quadrant chart previously constructed semantic JSON in `merman-core`, then
`merman-render` deserialized that JSON into private layout structs. The render-only path now
returns `QuadrantChartRenderModel` directly and SVG render-model dispatch uses the layout-only
quadrant chart SVG path.

## Parameters

- Date: 2026-05-08
- Parent JSON baseline commit: `84ca63b2`
- Typed worktree base: `84ca63b2` plus the quadrant chart typed render-model change set
- Rust: `rustc 1.87.0 (17067e9ac 2025-05-09)`, host `x86_64-pc-windows-msvc`
- Fixture: `quadrant_medium`
- Criterion options: `--noplot --sample-size 20 --warm-up-time 1 --measurement-time 1`

## Commands

Parent JSON baseline:

```text
cargo bench -p merman --features render --bench pipeline quadrant_medium -- --noplot --sample-size 20 --warm-up-time 1 --measurement-time 1
```

Typed worktree:

```text
cargo bench -p merman --features render --bench pipeline quadrant_medium -- --noplot --sample-size 20 --warm-up-time 1 --measurement-time 1
```

## Mid Estimates

| bench | parent JSON render model | typed render model | change |
| --- | ---: | ---: | ---: |
| `parse/quadrant_medium` | 155.98 us | 8.6722 us | -94.4% |
| `parse_known_type/quadrant_medium` | 166.15 us | 113.09 us | -31.9% |
| `layout/quadrant_medium` | 12.307 us | 7.1043 us | -42.3% |
| `render/quadrant_medium` | 31.914 us | 27.042 us | -15.3% |
| `end_to_end/quadrant_medium` | 217.64 us | 41.942 us | -80.7% |

## Interpretation

- `parse/quadrant_medium` improves because render-only parse now returns
  `QuadrantChartRenderModel` without materializing the full semantic JSON tree.
- `parse_known_type/quadrant_medium` still exercises the semantic JSON API, but the JSON
  materializer now reuses the typed parser output instead of duplicating model construction logic.
- `layout/quadrant_medium` improves because render-layout dispatch consumes the typed model
  directly and skips private JSON deserialization.
- `render/quadrant_medium` improves because SVG render-model dispatch uses the existing
  layout-only quadrant chart renderer with a null semantic payload.
- `end_to_end/quadrant_medium` improves because parse and layout transport savings dominate.

## Verification

- `cargo fmt`
- `cargo check -p merman-core -p merman-render --all-features`
- `cargo nextest run -p merman-core quadrant`
- `cargo nextest run -p merman-render --no-tests pass quadrant`
- `cargo run -p xtask -- compare-quadrantchart-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo clippy -p merman-core -p merman-render --all-targets --all-features -- -D warnings`
- `cargo run -p xtask -- verify --strict`
