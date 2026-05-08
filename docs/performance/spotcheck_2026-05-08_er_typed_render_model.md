# ER Typed Render Model Spotcheck

This report captures a same-machine Criterion spotcheck for the ER typed render-model migration.
ER diagrams previously constructed semantic JSON in `merman-core`, then `merman-render`
deserialized that JSON into private layout/SVG transport structs. The render-only path now returns
`ErDiagramRenderModel` directly, layout consumes the typed model, and SVG render-model dispatch
reads the same core model.

ER timing was visibly noisy with the shorter 20-sample command used for some smaller diagrams, so
this spotcheck uses a slightly longer 30-sample run.

## Parameters

- Date: 2026-05-08
- Parent JSON baseline commit: `c14839b3`
- Typed worktree base: `c14839b3` plus the ER typed render-model change set
- Rust: `rustc 1.87.0 (17067e9ac 2025-05-09)`, host `x86_64-pc-windows-msvc`
- Fixture: `er_medium`
- Criterion options: `--noplot --sample-size 30 --warm-up-time 2 --measurement-time 2`

## Commands

Parent JSON baseline:

```text
cargo bench -p merman --features render --bench pipeline er_medium -- --noplot --sample-size 30 --warm-up-time 2 --measurement-time 2
```

Typed worktree:

```text
cargo bench -p merman --features render --bench pipeline er_medium -- --noplot --sample-size 30 --warm-up-time 2 --measurement-time 2
```

## Mid Estimates

| bench | parent JSON render model | typed render model | change |
| --- | ---: | ---: | ---: |
| `parse/er_medium` | 116.85 us | 31.780 us | -72.8% |
| `parse_known_type/er_medium` | 98.871 us | 127.77 us | +29.2% |
| `layout/er_medium` | 394.80 us | 367.08 us | -7.0% |
| `render/er_medium` | 629.69 us | 532.37 us | -15.5% |
| `end_to_end/er_medium` | 1.4385 ms | 1.1664 ms | -18.9% |

## Interpretation

- `parse/er_medium` improves because render-only parse now returns `ErDiagramRenderModel` without
  materializing the semantic JSON payload.
- `parse_known_type/er_medium` still exercises the semantic JSON compatibility API. That path is
  now a typed-model round-trip through JSON serialization, so keep it visible but do not treat it
  as the target of the render-only migration.
- `layout/er_medium` and `render/er_medium` improve in the final paired run, which is enough for a
  render-model cleanup that mostly deletes transport code and keeps the semantic JSON API intact.
- `end_to_end/er_medium` improves because the render-only pipeline removes both semantic JSON
  construction and render-side JSON transport deserialization from the hot path.

## Verification

- `cargo fmt`
- `cargo check -p merman-core -p merman-render --all-features`
- `cargo nextest run -p merman-core er`
- `cargo nextest run -p merman-render er`
- `cargo run -p xtask -- compare-er-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo clippy -p merman-core -p merman-render --all-targets --all-features -- -D warnings`
- `cargo run -p xtask -- verify --strict`
