# Sequence Typed Render Model Spot-check

This report captures a post-migration baseline for the M2 fearless-refactor sequence typed render
model work. The numbers are local spot-checks, not release benchmark guarantees.

## Parameters

- Date: 2026-05-07
- Command:
  `cargo bench -p merman --features render --bench pipeline -- sequence --sample-size 20 --warm-up-time 1 --measurement-time 1`
- Fixtures: `sequence_tiny`, `sequence_medium`
- Text measurer: pipeline bench default
- Note: no immediate pre-migration sample was captured on the same machine. Treat this as the
  baseline for follow-up sequence renderer cleanup and future typed migrations.

## Criterion Results

Mid estimates:

| fixture | stage | time |
| --- | --- | ---: |
| `sequence_tiny` | `parse` | 2.0980 us |
| `sequence_tiny` | `parse_known_type` | 6.8336 us |
| `sequence_tiny` | `layout` | 9.9138 us |
| `sequence_tiny` | `render` | 18.117 us |
| `sequence_tiny` | `end_to_end` | 35.344 us |
| `sequence_medium` | `parse` | 42.821 us |
| `sequence_medium` | `parse_known_type` | 62.395 us |
| `sequence_medium` | `layout` | 113.67 us |
| `sequence_medium` | `render` | 67.527 us |
| `sequence_medium` | `end_to_end` | 210.04 us |

## Parse Timing Samples

Command:

`MERMAN_PARSE_TIMING=1 cargo run -q -p merman-cli -- render crates/merman/benches/fixtures/<fixture>.mmd --out target/<fixture>_timing.svg`

Warm command-run samples:

| fixture | model | total | preprocess | parse | sanitize | input bytes |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `sequence_tiny` | `sequence` | 452.7 us | 148.6 us | 296.8 us | 3.0 us | 34 |
| `sequence_medium` | `sequence` | 723.4 us | 126.2 us | 593.4 us | 1.9 us | 550 |

## Observations

- `parse_diagram_for_render_model_sync` now reports `model=sequence`, confirming the typed
  render-model path is active.
- The semantic JSON API remains covered separately by core tests; this spot-check is specifically
  for render-pipeline cost tracking.
