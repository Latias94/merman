# Pie Typed Render Model Spotcheck

This report captures a same-machine Criterion spotcheck for the pie typed render-model migration.
Pie is a small diagram family, so this is mostly a regression anchor and a cleanup proof that the
render path can avoid duplicate JSON transport.

## Parameters

- Date: 2026-05-08
- Parent JSON baseline commit: `d704a17d`
- Typed worktree base: `d704a17d` plus the pie typed render-model change set
- Rust: `rustc 1.87.0 (17067e9ac 2025-05-09)`, host `x86_64-pc-windows-msvc`
- Fixture: `pie_medium`
- Criterion options: `--noplot --sample-size 20 --warm-up-time 1 --measurement-time 1`

## Commands

Typed worktree:

```text
cargo bench -p merman --features render --bench pipeline pie_medium -- --noplot --sample-size 20 --warm-up-time 1 --measurement-time 1
```

Parent JSON baseline:

```text
git worktree add -f E:\Rust\merman-pie-json-baseline d704a17d
$env:CARGO_TARGET_DIR='E:\Rust\merman\target\bench-pie-json-target'
cargo bench -p merman --features render --bench pipeline pie_medium -- --noplot --sample-size 20 --warm-up-time 1 --measurement-time 1
git worktree remove --force E:\Rust\merman-pie-json-baseline
```

## Mid Estimates

| bench | parent JSON render model | typed render model | change |
| --- | ---: | ---: | ---: |
| `parse/pie_medium` | 9.3219 us | 4.9986 us | -46.4% |
| `parse_known_type/pie_medium` | 8.2725 us | 6.3358 us | -23.4% |
| `layout/pie_medium` | 20.359 us | 14.694 us | -27.8% |
| `render/pie_medium` | 20.261 us | 18.626 us | -8.1% |
| `end_to_end/pie_medium` | 44.706 us | 41.566 us | -7.0% |

## Interpretation

- `parse/pie_medium` improves because `parse_diagram_for_render_model_sync` now returns
  `PieDiagramRenderModel` instead of building semantic JSON for the render path.
- `parse_known_type/pie_medium` still exercises the semantic JSON API, but it benefits from the
  parser sharing typed section construction before serializing the stable JSON payload.
- `layout/pie_medium` improves because the layout path no longer deserializes private pie structs
  from semantic JSON when called through render-model dispatch.
- `render/pie_medium` is roughly stable with a small midpoint improvement because SVG rendering now
  consumes the same typed model instead of deserializing a second private SVG model.
- `end_to_end/pie_medium` improves modestly; pie is already small, so absolute savings are in the
  low microseconds.

## Verification

- `cargo fmt --check`
- `cargo check --workspace --all-features`
- `cargo nextest run -p merman-core pie`
- `cargo nextest run -p merman-render pie`
- `cargo clippy -p merman-core -p merman-render --all-targets --all-features -- -D warnings`
- `cargo run -p xtask -- compare-pie-svgs --check-dom --dom-mode parity --dom-decimals 3`
