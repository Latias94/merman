# C4 Direct Render Model Parse Spotcheck

This note records the C4 render-parse cleanup that routes `parse_c4_model_for_render` through
`C4Db::to_render_model()` instead of constructing the full semantic JSON `Value` tree and then
deserializing it into `C4DiagramRenderModel`.

## Parameters

- Date: 2026-05-09
- Git state: working tree after the C4 direct render-model parse cleanup
- Code path: `crates/merman-core/src/diagrams/c4.rs`
- Bench command:
  `cargo bench -p merman --features render --bench pipeline -- --noplot --sample-size 10 --warm-up-time 0.5 --measurement-time 0.5 c4_medium`
- Verification commands:
  - `cargo nextest run -p merman-core c4`
  - `cargo nextest run -p merman --features render c4`
  - `cargo nextest run -p merman --features render pipeline_bench_fixtures_are_benchmarkable`
  - `cargo run -p xtask -- compare-c4-svgs --check-dom --dom-mode parity --dom-decimals 3`

## Results

| benchmark | observed time range | midpoint |
| --- | ---: | ---: |
| `parse/c4_medium` | 36.946-40.355 us | 38.937 us |
| `parse_known_type/c4_medium` | 176.14-194.27 us | 184.51 us |
| `layout/c4_medium` | 65.640-79.054 us | 73.622 us |
| `render/c4_medium` | 75.015-84.618 us | 78.257 us |
| `end_to_end/c4_medium` | 176.19-191.27 us | 182.37 us |

Criterion compared this run against the local stored baseline and reported:

- `parse/c4_medium`: `-82.827%` midpoint change, performance improved.
- `end_to_end/c4_medium`: `-39.428%` midpoint change, performance improved.
- `layout/c4_medium`: `+13.144%` midpoint change, performance regressed.

## Observations

- The previous C4 pipeline smoke recorded `parse/c4_medium` at `207.87-241.46 us` and
  `end_to_end/c4_medium` at `301.68-352.39 us`; this cleanup removes the render-only JSON bridge
  from that path.
- `parse_known_type/c4_medium` intentionally remains the compatibility semantic-JSON parse path, so
  it is not expected to benefit from this change.
- The layout regression warning is recorded but not attributed to this change: the layout code was
  untouched, the run used a low sample size, and the end-to-end path still improved materially.
- The same-day cross-repo reports were refreshed after this cleanup; the refreshed C4/XYChart
  comparison now shows C4 end-to-end at about `1.4x`, and the stage spotcheck shows C4 parse at
  about `1.8x` while Architecture layout and XyChart layout/render stay farther from parity.
