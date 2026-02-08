# Compare All SVGs (Mermaid Parity)

This note documents the `xtask compare-all-svgs` helper, which runs the per-diagram SVG parity
checks in one shot and aggregates failures.

## Run

- Full suite, DOM parity enabled:
  - `cargo run -p xtask -- compare-all-svgs --check-dom --dom-decimals 3`

- Use a specific DOM comparison mode for all diagrams:
  - `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3`

- Only run a subset of diagrams:
  - `cargo run -p xtask -- compare-all-svgs --check-dom --diagram flowchart --diagram sequence`

- Skip some diagrams:
  - `cargo run -p xtask -- compare-all-svgs --check-dom --skip gantt --skip flowchart`

## Outputs

- Local SVGs are written under `target/compare/<diagram>/`.
- Per-diagram reports are written under `target/compare/`.
  - When `--dom-mode` is provided, reports are mode-suffixed to avoid overwriting across runs:
    - `target/compare/<diagram>_report_<mode>.md` (e.g. `target/compare/state_report_parity_root.md`)
  - When `--dom-mode` is omitted, per-diagram compare tasks use their default report paths
    (typically `target/compare/<diagram>_report.md`).

## Flowchart-specific options

`compare-all-svgs` forwards these only to the Flowchart compare task:

- `--flowchart-text-measurer vendored`
- `--report-root`

Example:

- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --flowchart-text-measurer vendored --report-root`

## Notes

- `parity-root` depends on the headless `getBBox()`-like bounds approximation in `merman-render`.
  It treats `<a>` as a transform container (so link-wrapped nodes contribute correctly), and it
  ignores non-rendered containers like `<defs>`/`<marker>` when deriving the root viewport.

## Precision

- `--dom-decimals 3` is the current stability gate for `parity-root`.
- `--dom-decimals 6` is a useful stress test for root viewport parity (`viewBox` + `max-width`),
  but it is expected to surface small residual numeric drift as we continue to tighten the
  headless bbox + viewport pipeline.
  - Some of this drift is inherent to browser float/serialization behavior. For known upstream
    fixture deltas, we keep exact `parity-root` by applying fixture-derived root viewport overrides
    keyed by `diagram_id` (fixture stem) under `crates/merman-render/src/generated/*_root_overrides_11_12_2.rs`.
  - To review the current override footprint, run `cargo run -p xtask -- report-overrides`.
