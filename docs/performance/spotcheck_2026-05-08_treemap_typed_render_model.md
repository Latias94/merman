# Treemap Typed Render Model Spotcheck

This report captures a same-machine Criterion spotcheck for the treemap typed render-model
migration. Treemap previously constructed semantic JSON in `merman-core`, then `merman-render`
deserialized that JSON into a private layout model. The render-only path now returns
`TreemapDiagramRenderModel` directly, layout consumes the typed model, and SVG render-model
dispatch uses the layout-only treemap SVG path.

The checked-in `treemap_medium` fixture originally parsed as an error, so both runs below use the
same valid replacement sample:

```text
treemap-beta
"Root"
  "Group A"
    "A1": 30
    "A2": 30
  "Group B"
    "B1": 20
    "B2": 20
```

## Parameters

- Date: 2026-05-08
- Parent JSON baseline commit: `f64e6748`
- Typed worktree base: `f64e6748` plus the treemap typed render-model change set
- Rust: `rustc 1.87.0 (17067e9ac 2025-05-09)`, host `x86_64-pc-windows-msvc`
- Fixture: `treemap_medium`
- Criterion options: `--noplot --sample-size 20 --warm-up-time 1 --measurement-time 1`

## Commands

Parent JSON baseline:

```text
cargo bench -p merman --features render --bench pipeline treemap_medium -- --noplot --sample-size 20 --warm-up-time 1 --measurement-time 1
```

Typed worktree:

```text
cargo bench -p merman --features render --bench pipeline treemap_medium -- --noplot --sample-size 20 --warm-up-time 1 --measurement-time 1
```

## Mid Estimates

| bench | parent JSON render model | typed render model | change |
| --- | ---: | ---: | ---: |
| `parse/treemap_medium` | 136.35 us | 5.6264 us | -95.9% |
| `parse_known_type/treemap_medium` | 157.82 us | 132.21 us | -16.2% |
| `layout/treemap_medium` | 4.8934 us | 3.8335 us | -21.7% |
| `render/treemap_medium` | 59.469 us | 65.256 us | +9.7% |
| `end_to_end/treemap_medium` | 203.44 us | 89.354 us | -56.1% |

## Interpretation

- `parse/treemap_medium` improves because render-only parse now returns
  `TreemapDiagramRenderModel` without materializing the full semantic JSON tree.
- `parse_known_type/treemap_medium` still exercises the semantic JSON API, but the JSON
  materializer now reuses the typed parser output instead of duplicating model construction logic.
- `layout/treemap_medium` improves because render-layout dispatch consumes the typed model
  directly instead of deserializing a private transport struct.
- `render/treemap_medium` regresses slightly in this sample. Treemap SVG rendering is already
  layout-dominated, so this is worth keeping visible but not over-reading.
- `end_to_end/treemap_medium` still improves materially because semantic JSON construction and
  render-side JSON deserialization are removed from the render-only path.

## Verification

- `cargo fmt`
- `cargo check -p merman-core -p merman-render --all-features`
- `cargo nextest run -p merman-core treemap`
- `cargo nextest run -p merman-render treemap --no-tests pass`
- `cargo run -p xtask -- compare-treemap-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo clippy -p merman-core -p merman-render --all-targets --all-features -- -D warnings`
- `cargo run -p xtask -- verify --strict`
