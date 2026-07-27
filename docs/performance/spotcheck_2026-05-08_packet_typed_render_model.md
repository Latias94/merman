# Packet Typed Render Model Spotcheck

This report captures a same-machine Criterion spotcheck for the packet typed render-model
migration. Packet is a useful small-diagram migration because its previous semantic JSON payload
included a full cloned effective config, making the render-only path pay a large transport cost.

## Parameters

- Date: 2026-05-08
- Parent JSON baseline commit: `fa9dc8b1`
- Typed worktree base: `fa9dc8b1` plus the packet typed render-model change set
- Rust: `rustc 1.87.0 (17067e9ac 2025-05-09)`, host `x86_64-pc-windows-msvc`
- Fixture: `packet_medium`
- Criterion options: `--noplot --sample-size 20 --warm-up-time 1 --measurement-time 1`

## Commands

Parent JSON baseline:

```text
cargo bench -p merman --features render --bench pipeline packet_medium -- --noplot --sample-size 20 --warm-up-time 1 --measurement-time 1
```

Typed worktree:

```text
cargo bench -p merman --features render --bench pipeline packet_medium -- --noplot --sample-size 20 --warm-up-time 1 --measurement-time 1
```

## Mid Estimates

| bench | parent JSON render model | typed render model | change |
| --- | ---: | ---: | ---: |
| `parse/packet_medium` | 130.35 us | 1.8412 us | -98.6% |
| `parse_known_type/packet_medium` | 113.50 us | 99.735 us | -12.1% |
| `layout/packet_medium` | 852.30 ns | 561.00 ns | -34.2% |
| `render/packet_medium` | 4.9642 us | 5.0466 us | +1.7% |
| `end_to_end/packet_medium` | 126.81 us | 8.1710 us | -93.6% |

## Interpretation

- `parse/packet_medium` improves sharply because `parse_diagram_for_render_model_sync` now returns
  `PacketDiagramRenderModel` without constructing semantic JSON or cloning the full effective
  config into the render-only payload.
- `parse_known_type/packet_medium` still exercises the semantic JSON API, but it benefits from
  sharing typed packet construction before serializing the stable JSON payload.
- `layout/packet_medium` improves because render-model layout dispatch no longer deserializes a
  private packet transport model from semantic JSON.
- `render/packet_medium` is effectively stable; the midpoint is slightly slower in this sample,
  but the absolute delta is under `0.1 us`.
- `end_to_end/packet_medium` captures the intended win: the public render path avoids the previous
  large JSON transport cost while keeping the semantic JSON compatibility API unchanged.

## Verification

- `cargo fmt`
- `cargo check -p merman-core -p merman-render --all-features`
- `cargo nextest run -p merman-core packet`
- `cargo nextest run -p merman-render --no-tests pass packet`
- `cargo run -p xtask -- compare-packet-svgs --check-dom --dom-mode parity --dom-decimals 3`
