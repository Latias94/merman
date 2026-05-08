# Core Context Cleanup Spotcheck

This note captures an isolated same-machine Criterion spotcheck for the core flowchart/state
context cleanup work. The numbers are local regression anchors, not release performance
guarantees.

## Parameters

- Date: 2026-05-08
- Parent baseline commit: `cefe26b3`
- Current commit: `e7e761db`
- Rust: `rustc 1.87.0 (17067e9ac 2025-05-09)`, host `x86_64-pc-windows-msvc`
- Baseline target dir: `target/bench-core-baseline`
- Current target dir: `target/bench-core-current`
- Fixtures: `flowchart_medium`, `state_medium`
- Criterion options: `--noplot --sample-size 20 --warm-up-time 1 --measurement-time 1`

## Commands

Baseline worktree:

```text
git worktree add -f E:\Rust\merman-core-baseline e7e761db^
$env:CARGO_TARGET_DIR='E:\Rust\merman\target\bench-core-baseline'
cargo bench -p merman --features render --bench pipeline flowchart_medium -- --noplot --sample-size 20 --warm-up-time 1 --measurement-time 1
cargo bench -p merman --features render --bench pipeline state_medium -- --noplot --sample-size 20 --warm-up-time 1 --measurement-time 1
```

Current worktree:

```text
$env:CARGO_TARGET_DIR='E:\Rust\merman\target\bench-core-current'
cargo bench -p merman --features render --bench pipeline flowchart_medium -- --noplot --sample-size 20 --warm-up-time 1 --measurement-time 1
cargo bench -p merman --features render --bench pipeline state_medium -- --noplot --sample-size 20 --warm-up-time 1 --measurement-time 1
```

## Flowchart Medium

| bench | baseline | current | change |
| --- | ---: | ---: | ---: |
| `parse/flowchart_medium` | 426.73 us | 383.53 us | -10.1% |
| `parse_known_type/flowchart_medium` | 726.18 us | 763.01 us | +5.1% |
| `layout/flowchart_medium` | 6.8675 ms | 6.8645 ms | flat |
| `render/flowchart_medium` | 439.12 us | 437.22 us | flat |
| `end_to_end/flowchart_medium` | 7.4503 ms | 7.1727 ms | -3.7% |

## State Medium

| bench | baseline | current | change |
| --- | ---: | ---: | ---: |
| `parse/state_medium` | 89.472 us | 75.691 us | -15.4% |
| `parse_known_type/state_medium` | 413.87 us | 397.05 us | -4.1% |
| `layout/state_medium` | 773.72 us | 806.92 us | +4.3% |
| `render/state_medium` | 1.6204 ms | 1.5955 ms | -1.5% |
| `end_to_end/state_medium` | 2.6655 ms | 2.7801 ms | +4.3% |

## Notes

- The first run on each isolated target was noisier; the table above uses the warmed rerun from the
  isolated target directories.
- Flowchart parse/layout/render stayed flat or improved enough to treat the core context cleanup as
  performance-neutral to slightly positive.
- State parse improved, while layout/end-to-end drift stayed in the low single digits and should be
  rechecked if state-specific tuning becomes the next target.
- Verification gates for this batch still passed:
  - `cargo clippy -p merman-core --all-targets --all-features -- -D warnings`
  - `cargo nextest run -p merman-core`
  - `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-decimals 3`
  - `cargo run -p xtask -- compare-state-svgs --check-dom --dom-decimals 3`
  - `cargo run -p xtask -- verify --strict`
