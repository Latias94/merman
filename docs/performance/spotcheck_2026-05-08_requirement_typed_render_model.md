# Requirement Typed Render Model Spotcheck

This report captures a same-machine Criterion spotcheck for the requirement typed render-model
migration. Requirement previously had three duplicated semantic transport shapes: parser JSON
construction in `merman-core`, layout-only structs in `merman-render`, and another private SVG
semantic model in the parity renderer.

## Parameters

- Date: 2026-05-08
- Parent JSON baseline commit: `f46bb395`
- Typed worktree base: `f46bb395` plus the requirement typed render-model change set
- Rust: `rustc 1.87.0 (17067e9ac 2025-05-09)`, host `x86_64-pc-windows-msvc`
- Fixture: `requirement_medium`
- Criterion options: `--noplot --sample-size 20 --warm-up-time 1 --measurement-time 1`

## Commands

Parent JSON baseline:

```text
cargo bench -p merman --features render --bench pipeline requirement_medium -- --noplot --sample-size 20 --warm-up-time 1 --measurement-time 1
```

Typed worktree:

```text
cargo bench -p merman --features render --bench pipeline requirement_medium -- --noplot --sample-size 20 --warm-up-time 1 --measurement-time 1
```

The typed worktree was run twice. The first post-migration run showed `render/requirement_medium`
midpoint drift; after changing SVG lookup tables and node style access to borrow from the typed
model, the second run confirmed render returned to roughly baseline while parse/end-to-end stayed
faster. The table below uses that second typed run.

## Mid Estimates

| bench | parent JSON render model | typed render model | change |
| --- | ---: | ---: | ---: |
| `parse/requirement_medium` | 111.55 us | 9.3768 us | -91.6% |
| `parse_known_type/requirement_medium` | 122.09 us | 119.44 us | -2.2% |
| `layout/requirement_medium` | 155.79 us | 157.03 us | +0.8% |
| `render/requirement_medium` | 122.11 us | 119.78 us | -1.9% |
| `end_to_end/requirement_medium` | 542.35 us | 379.17 us | -30.1% |

## Interpretation

- `parse/requirement_medium` improves because `parse_diagram_for_render_model_sync` now returns
  `RequirementDiagramRenderModel` instead of constructing semantic JSON for render-only callers.
- `parse_known_type/requirement_medium` still exercises the semantic JSON API, so it only gets a
  small benefit from sharing typed model construction before serializing the stable JSON payload.
- `layout/requirement_medium` is essentially flat. The layout path no longer deserializes a private
  requirement transport model, but text measurement and graph layout dominate this fixture.
- `render/requirement_medium` is also essentially flat after avoiding per-node class/style Vec
  clones and using borrowed lookup maps in the typed SVG path.
- `end_to_end/requirement_medium` improves because the render-only parse path avoids the expensive
  semantic JSON construction/deserialization loop.

## Verification

- `cargo fmt`
- `cargo check -p merman-core -p merman-render --all-features`
- `cargo nextest run -p merman-core requirement`
- `cargo nextest run -p merman-render requirement`
- `cargo run -p xtask -- compare-requirement-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo clippy -p merman-core -p merman-render --all-targets --all-features -- -D warnings`
- `cargo run -p xtask -- verify --strict`
