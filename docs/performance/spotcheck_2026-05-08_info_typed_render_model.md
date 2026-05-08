# Info Typed Render Model Spotcheck

This report captures a same-machine Criterion spotcheck for the info typed render-model migration.
Info is layout-only for current render purposes, so the migration mainly removes semantic JSON
transport from the render-only parse path and gives the diagram an explicit core/render typed
boundary.

## Parameters

- Date: 2026-05-08
- Fixture-added JSON-fallback baseline commit: `1146a12a` plus `info_medium` bench fixture
- Typed worktree base: `1146a12a` plus the info typed render-model change set
- Rust: `rustc 1.87.0 (17067e9ac 2025-05-09)`, host `x86_64-pc-windows-msvc`
- Fixture: `info_medium`
- Criterion options: `--noplot --sample-size 20 --warm-up-time 1 --measurement-time 1`

The project did not have an `info_medium` pipeline fixture before this change. The JSON-fallback
baseline was captured after adding only that fixture, before adding the typed render-model
dispatcher.

## Commands

Fixture-added JSON-fallback baseline:

```text
cargo bench -p merman --features render --bench pipeline info_medium -- --noplot --sample-size 20 --warm-up-time 1 --measurement-time 1
```

Typed worktree:

```text
cargo bench -p merman --features render --bench pipeline info_medium -- --noplot --sample-size 20 --warm-up-time 1 --measurement-time 1
```

## Mid Estimates

| bench | fixture-added JSON fallback | typed render model | change |
| --- | ---: | ---: | ---: |
| `parse/info_medium` | 1.4884 us | 1.2063 us | -19.0% |
| `parse_known_type/info_medium` | 1.0691 us | 1.1150 us | +4.3% |
| `layout/info_medium` | 129.18 ns | 128.76 ns | -0.3% |
| `render/info_medium` | 3.3577 us | 3.4091 us | +1.5% |
| `end_to_end/info_medium` | 5.5714 us | 5.4206 us | -2.7% |

## Interpretation

- `parse/info_medium` improves because `parse_diagram_for_render_model_sync` now returns
  `InfoDiagramRenderModel` without constructing the semantic JSON object for render-only use.
- `parse_known_type/info_medium` still exercises the semantic JSON API. Criterion classified this
  sample as a small regression, but the midpoint movement is about 46 ns; keep it visible rather
  than over-interpreting such a small fixture.
- `layout/info_medium` is unchanged in practice because info layout does not depend on semantic
  fields today.
- `render/info_medium` shows a small positive midpoint drift in this sample, but
  `end_to_end/info_medium` remains slightly faster.

## Verification

- `cargo fmt`
- `cargo check -p merman-core -p merman-render --all-features`
- `cargo nextest run -p merman-core info`
- `cargo nextest run -p merman-render --no-tests pass info`
- `cargo run -p xtask -- compare-info-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo clippy -p merman-core -p merman-render --all-targets --all-features -- -D warnings`
- `cargo run -p xtask -- verify --strict`
