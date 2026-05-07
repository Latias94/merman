# Sankey Typed Render Model Spotcheck

This report captures a same-machine Criterion spotcheck for the sankey typed render-model
migration. Sankey previously parsed CSV into semantic JSON for render-only callers, then the layout
layer deserialized that JSON into private renderer structs.

## Parameters

- Date: 2026-05-08
- Parent JSON baseline commit: `6c40baf7`
- Typed worktree base: `6c40baf7` plus the sankey typed render-model change set
- Rust: `rustc 1.87.0 (17067e9ac 2025-05-09)`, host `x86_64-pc-windows-msvc`
- Fixture: `sankey_medium`
- Criterion options: `--noplot --sample-size 20 --warm-up-time 1 --measurement-time 1`

## Commands

Parent JSON baseline:

```text
cargo bench -p merman --features render --bench pipeline sankey_medium -- --noplot --sample-size 20 --warm-up-time 1 --measurement-time 1
```

Typed worktree:

```text
cargo bench -p merman --features render --bench pipeline sankey_medium -- --noplot --sample-size 20 --warm-up-time 1 --measurement-time 1
```

The typed worktree was run multiple times while tightening the materialization boundary. The final
sample below uses a single parsed DB with two output materializers: direct semantic JSON for
`parse_diagram_sync`, and moved typed model data for `parse_diagram_for_render_model_sync`.

## Mid Estimates

| bench | parent JSON render model | typed render model | change |
| --- | ---: | ---: | ---: |
| `parse/sankey_medium` | 109.12 us | 5.4373 us | -95.0% |
| `parse_known_type/sankey_medium` | 101.45 us | 106.81 us | +5.3% |
| `layout/sankey_medium` | 9.2922 us | 7.8167 us | -15.9% |
| `render/sankey_medium` | 19.111 us | 19.278 us | +0.9% |
| `end_to_end/sankey_medium` | 132.26 us | 34.704 us | -73.8% |

## Interpretation

- `parse/sankey_medium` improves because render-only parsing now returns
  `SankeyDiagramRenderModel` without constructing the full semantic JSON graph.
- `parse_known_type/sankey_medium` still exercises the semantic JSON API. The final sample shows a
  small midpoint drift, so keep this visible in future broad benchmark passes.
- `layout/sankey_medium` improves because render-layout dispatch consumes the typed graph directly
  instead of deserializing JSON into private layout structs.
- `render/sankey_medium` is essentially flat because sankey SVG rendering consumes the layout only.
- `end_to_end/sankey_medium` improves because parse/layout savings dominate.

## Verification

- `cargo fmt`
- `cargo check -p merman-core -p merman-render --all-features`
- `cargo nextest run -p merman-core sankey`
- `cargo nextest run -p merman-render sankey`
- `cargo run -p xtask -- compare-sankey-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo clippy -p merman-core -p merman-render --all-targets --all-features -- -D warnings`
- `cargo run -p xtask -- verify --strict`
