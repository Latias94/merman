# GitGraph Typed Render Model Spotcheck

This report captures a same-machine Criterion spotcheck for the gitGraph typed render-model
migration. gitGraph previously constructed semantic JSON in `merman-core`, then `merman-render`
deserialized that JSON into private layout structs. The render-only path now returns
`GitGraphRenderModel` directly, layout consumes borrowed typed data where possible, and SVG
render-model dispatch reads accessibility fields from the typed model.

## Parameters

- Date: 2026-05-08
- Parent JSON baseline commit: `94a03c71`
- Typed worktree base: `94a03c71` plus the gitGraph typed render-model change set
- Rust: `rustc 1.87.0 (17067e9ac 2025-05-09)`, host `x86_64-pc-windows-msvc`
- Fixture: `gitgraph_medium`
- Criterion options: `--noplot --sample-size 20 --warm-up-time 1 --measurement-time 1`

## Commands

Parent JSON baseline:

```text
cargo bench -p merman --features render --bench pipeline gitgraph_medium -- --noplot --sample-size 20 --warm-up-time 1 --measurement-time 1
```

Typed worktree:

```text
cargo bench -p merman --features render --bench pipeline gitgraph_medium -- --noplot --sample-size 20 --warm-up-time 1 --measurement-time 1
```

## Mid Estimates

| bench | parent JSON render model | typed render model | change |
| --- | ---: | ---: | ---: |
| `parse/gitgraph_medium` | 127.88 us | 10.985 us | -91.4% |
| `parse_known_type/gitgraph_medium` | 133.12 us | 121.11 us | -9.0% |
| `layout/gitgraph_medium` | 18.715 us | 11.033 us | -41.0% |
| `render/gitgraph_medium` | 57.498 us | 55.547 us | -3.4% |
| `end_to_end/gitgraph_medium` | 203.17 us | 82.820 us | -59.2% |

## Interpretation

- `parse/gitgraph_medium` improves because render-only parse now returns `GitGraphRenderModel`
  without materializing the full semantic JSON tree.
- `parse_known_type/gitgraph_medium` still exercises the semantic JSON API, but the JSON
  materializer now reuses the typed parser output instead of duplicating model construction logic.
- `layout/gitgraph_medium` improves because render-layout dispatch consumes the typed model
  directly and the layout pass now borrows sorted commits and lookup indexes instead of cloning
  private transport structs.
- `render/gitgraph_medium` is roughly flat because gitGraph SVG work is dominated by layout output,
  text overrides, and SVG string emission after the model has already been laid out.
- `end_to_end/gitgraph_medium` improves because semantic JSON construction and render-side JSON
  deserialization are removed from the render-only path.

## Verification

- `cargo fmt`
- `cargo check -p merman-core -p merman-render --all-features`
- `cargo nextest run -p merman-core gitgraph`
- `cargo nextest run -p merman-render gitgraph --no-tests pass`
- `cargo run -p xtask -- compare-gitgraph-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo clippy -p merman-core -p merman-render --all-targets --all-features -- -D warnings`
- `cargo run -p xtask -- verify --strict`
