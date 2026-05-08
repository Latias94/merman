# ZenUML Typed Render Model Spotcheck

This report captures a same-machine Criterion spotcheck for the ZenUML render-only pipeline
migration. ZenUML remains a conservative compatibility layer that translates supported ZenUML
syntax into sequence syntax; the render-only path now feeds that translated text into
`SequenceDiagramRenderModel` instead of constructing semantic JSON and deserializing it again in
`merman-render`.

## Parameters

- Date: 2026-05-08
- Parent JSON baseline commit: `91d8afba`
- Typed worktree base: `91d8afba` plus the ZenUML typed render-model change set
- Rust: `rustc 1.87.0 (17067e9ac 2025-05-09)`, host `x86_64-pc-windows-msvc`
- Fixture: `zenuml_medium`
- Criterion options: `--noplot --sample-size 20 --warm-up-time 1 --measurement-time 1`

The parent JSON baseline was captured from a temporary detached worktree at `91d8afba`, then the
temporary worktree was removed.

## Commands

Parent JSON baseline:

```text
cargo bench -p merman --features render --bench pipeline zenuml_medium -- --noplot --sample-size 20 --warm-up-time 1 --measurement-time 1
```

Typed worktree:

```text
cargo bench -p merman --features render --bench pipeline zenuml_medium -- --noplot --sample-size 20 --warm-up-time 1 --measurement-time 1
```

## Mid Estimates

| bench | parent JSON render model | typed sequence render model | change |
| --- | ---: | ---: | ---: |
| `parse/zenuml_medium` | 21.577 us | 12.264 us | -43.2% |
| `parse_known_type/zenuml_medium` | 20.106 us | 20.284 us | +0.9% |
| `layout/zenuml_medium` | 30.394 us | 21.463 us | -29.4% |
| `render/zenuml_medium` | 30.898 us | 22.672 us | -26.6% |
| `end_to_end/zenuml_medium` | 137.74 us | 61.892 us | -55.1% |

## Interpretation

- `parse/zenuml_medium` improves because render-only parse now translates ZenUML once and returns
  `SequenceDiagramRenderModel` directly.
- `parse_known_type/zenuml_medium` still exercises the semantic JSON API, so it is effectively
  unchanged in the parent-vs-typed spotcheck.
- `layout/zenuml_medium` and `render/zenuml_medium` improve because the render path no longer
  deserializes sequence data from JSON fallback transport.
- `end_to_end/zenuml_medium` improves because this migration removes both semantic JSON
  construction and render-side JSON deserialization from the ZenUML render-only path.

## Verification

- `cargo fmt`
- `cargo check -p merman-core -p merman-render --all-features`
- `cargo nextest run -p merman-core zenuml`
- `cargo nextest run -p merman-render --no-tests pass zenuml`
- `cargo clippy -p merman-core -p merman-render --all-targets --all-features -- -D warnings`
- `cargo run -p xtask -- verify --strict`
