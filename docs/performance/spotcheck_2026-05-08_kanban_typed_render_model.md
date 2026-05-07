# Kanban Typed Render Model Spotcheck

This report captures a same-machine Criterion spotcheck for the kanban typed render-model
migration. The goal is a local regression anchor, not a release-wide performance guarantee.

## Parameters

- Date: 2026-05-08
- Parent baseline commit: `48ccb8ca`
- Typed commit: `d411a56d`
- Rust: `rustc 1.87.0 (17067e9ac 2025-05-09)`, host `x86_64-pc-windows-msvc`
- Fixture: `kanban_medium`
- Criterion options: `--noplot --sample-size 20 --warm-up-time 1 --measurement-time 1`
- Parent raw output: `target/bench/kanban_json_parent_2026-05-08.txt`
- Typed raw output: `target/bench/kanban_typed_current_2026-05-08.txt`

## Commands

Typed commit:

```text
cargo bench -p merman --features render --bench pipeline kanban_medium -- --noplot --sample-size 20 --warm-up-time 1 --measurement-time 1
```

Parent baseline:

```text
git worktree add -f E:\Rust\merman-kanban-json-baseline 48ccb8ca
$env:CARGO_TARGET_DIR='E:\Rust\merman\target\bench-parent-target'
cargo bench -p merman --features render --bench pipeline kanban_medium -- --noplot --sample-size 20 --warm-up-time 1 --measurement-time 1
git worktree remove --force E:\Rust\merman-kanban-json-baseline
```

## Mid Estimates

| bench | parent JSON render model | typed render model | change |
| --- | ---: | ---: | ---: |
| `parse/kanban_medium` | 129.03 us | 9.8001 us | -92.4% |
| `parse_known_type/kanban_medium` | 133.90 us | 143.55 us | +7.2% |
| `layout/kanban_medium` | 21.024 us | 17.235 us | -18.0% |
| `render/kanban_medium` | 21.018 us | 22.382 us | +6.5% |
| `end_to_end/kanban_medium` | 186.70 us | 50.931 us | -72.7% |

## Interpretation

- `parse/kanban_medium` measures `parse_diagram_for_render_model_sync`, so it captures the intended
  render-only migration from semantic JSON fallback to `KanbanDiagramRenderModel`.
- `parse_known_type/kanban_medium` still measures the stable semantic JSON API
  (`parse_diagram_as_sync`), so it is not expected to improve.
- `layout/kanban_medium` improves because render-layout dispatch no longer deserializes the kanban
  layout input from semantic JSON.
- `render/kanban_medium` is effectively unchanged. Kanban SVG rendering already consumes layout and
  config only, so the typed semantic model does not materially affect SVG emission.
- `end_to_end/kanban_medium` improves mostly through the render-model parse path.

## Verification

- `cargo nextest run -p merman-core kanban`
- `cargo nextest run -p merman-render kanban`
- `cargo nextest run -p merman-cli cli_renders_png_for_negative_viewbox_diagrams`
- `cargo run -p xtask -- compare-kanban-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo run -p xtask -- verify --strict`
