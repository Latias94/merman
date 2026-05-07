# Journey Typed Render Model Spotcheck

This report captures a same-machine Criterion spotcheck for the journey typed render-model
migration. Journey is a small-to-moderate diagram family with duplicated parser/render transport
structs and a renderer that only needs a small slice of semantic data at SVG time.

## Parameters

- Date: 2026-05-08
- Parent JSON baseline commit: `fdefbab0`
- Typed worktree base: `fdefbab0` plus the journey typed render-model change set
- Rust: `rustc 1.87.0 (17067e9ac 2025-05-09)`, host `x86_64-pc-windows-msvc`
- Fixture: `journey_medium`
- Criterion options: `--noplot --sample-size 20 --warm-up-time 1 --measurement-time 1`

## Commands

Parent JSON baseline:

```text
cargo bench -p merman --features render --bench pipeline journey_medium -- --noplot --sample-size 20 --warm-up-time 1 --measurement-time 1
```

Typed worktree:

```text
cargo bench -p merman --features render --bench pipeline journey_medium -- --noplot --sample-size 20 --warm-up-time 1 --measurement-time 1
```

The typed worktree was run twice because the first post-migration run showed render midpoint drift.
The table below uses the second typed run as the more conservative confirmation sample.

## Mid Estimates

| bench | parent JSON render model | typed render model | change |
| --- | ---: | ---: | ---: |
| `parse/journey_medium` | 14.175 us | 4.3940 us | -69.0% |
| `parse_known_type/journey_medium` | 11.416 us | 9.0062 us | -21.1% |
| `layout/journey_medium` | 9.2614 us | 7.3721 us | -20.4% |
| `render/journey_medium` | 28.128 us | 33.457 us | +18.9% |
| `end_to_end/journey_medium` | 60.108 us | 42.919 us | -28.6% |

## Interpretation

- `parse/journey_medium` improves because `parse_diagram_for_render_model_sync` now returns
  `JourneyDiagramRenderModel` instead of constructing semantic JSON for render-only callers.
- `parse_known_type/journey_medium` still exercises the semantic JSON API, but it benefits from
  sharing typed task construction before serializing the stable JSON payload.
- `layout/journey_medium` improves because render-model layout dispatch no longer deserializes a
  private journey transport model from semantic JSON.
- `render/journey_medium` is slower in this local sample. The typed SVG path does not add extra SVG
  work, but this should remain visible in future benchmark passes before calling journey rendering
  optimized.
- `end_to_end/journey_medium` still improves because parse/layout savings dominate the render
  midpoint drift.

## Verification

- `cargo fmt`
- `cargo check -p merman-core -p merman-render --all-features`
- `cargo nextest run -p merman-core journey`
- `cargo nextest run -p merman-render journey`
- `cargo clippy -p merman-core -p merman-render --all-targets --all-features -- -D warnings`
- `cargo run -p xtask -- compare-journey-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo run -p xtask -- verify --strict`
