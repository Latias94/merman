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
