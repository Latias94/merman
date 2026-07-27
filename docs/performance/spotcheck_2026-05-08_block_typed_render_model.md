# Block Typed Render Model Spotcheck

This report captures a same-machine Criterion spotcheck for the block typed render-model
migration. Block diagrams previously constructed semantic JSON in `merman-core`, then
`merman-render` deserialized that JSON into private layout/SVG transport structs. The render-only
path now returns `BlockDiagramRenderModel` directly, layout consumes the typed model, and SVG
render-model dispatch reads the same core model.

## Parameters

- Date: 2026-05-08
- Parent JSON baseline commit: `f88f4467`
- Typed worktree base: `f88f4467` plus the block typed render-model change set
- Rust: `rustc 1.87.0 (17067e9ac 2025-05-09)`, host `x86_64-pc-windows-msvc`
- Fixture: `block_medium`
- Fixture contents:

```text
block-beta
  A --> B
  B --> C
```

- Criterion options: `--noplot --sample-size 20 --warm-up-time 1 --measurement-time 1`

## Commands

Parent JSON baseline:

```text
cargo bench -p merman --features render --bench pipeline block_medium -- --noplot --sample-size 20 --warm-up-time 1 --measurement-time 1
```

Typed worktree:

```text
cargo bench -p merman --features render --bench pipeline block_medium -- --noplot --sample-size 20 --warm-up-time 1 --measurement-time 1
```

## Mid Estimates

| bench | parent JSON render model | typed render model | change |
| --- | ---: | ---: | ---: |
| `parse/block_medium` | 160.51 us | 12.930 us | -91.9% |
| `parse_known_type/block_medium` | 141.53 us | 145.81 us | +3.0% |
| `layout/block_medium` | 18.818 us | 14.952 us | -20.5% |
| `render/block_medium` | 19.926 us | 14.949 us | -25.0% |
| `end_to_end/block_medium` | 198.50 us | 49.017 us | -75.3% |

## Interpretation

- `parse/block_medium` improves because render-only parse now returns
  `BlockDiagramRenderModel` without materializing the semantic JSON payload.
- `parse_known_type/block_medium` still exercises the semantic JSON compatibility API; the small
  regression is visible but outside the render-only typed path this migration targets.
- `layout/block_medium` improves because render-layout dispatch consumes the typed block tree and
  edges directly instead of deserializing private transport structs.
- `render/block_medium` improves because SVG render-model dispatch now reuses the same typed block
  nodes and edges instead of repeating JSON deserialization.
- `end_to_end/block_medium` improves materially because the render pipeline removes both semantic
  JSON construction and render-side JSON transport deserialization from the hot path.

## Verification

- `cargo fmt`
- `cargo check -p merman-core -p merman-render --all-features`
- `cargo nextest run -p merman-core block`
- `cargo nextest run -p merman-render --lib`
- `cargo run -p xtask -- compare-block-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo clippy -p merman-core -p merman-render --all-targets --all-features -- -D warnings`
- `cargo run -p xtask -- verify --strict`
