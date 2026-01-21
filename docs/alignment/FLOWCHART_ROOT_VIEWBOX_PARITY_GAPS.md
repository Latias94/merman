# Flowchart Root SVG Parity Gaps (Mermaid@11.12.2)

This note tracks current gaps when comparing Flowchart Stage-B output against upstream Mermaid
SVG baselines **including** root `<svg>` viewport attributes (`viewBox`, `style="max-width: ..."`).

## Why This Exists

`merman`'s default Flowchart SVG parity checks focus on DOM structure (`--dom-mode parity`) and
intentionally ignore the root `<svg>` `viewBox` and `style` attributes while the layout and text
measurement subsystems are still converging.

For "full SVG DOM" parity work (closer to SVG XML parity), use `parity-root` mode.

## How To Run

- Generate a report and include root viewport deltas (does not fail unless `--check-dom` is set):
  - `cargo run -p xtask -- compare-flowchart-svgs --dom-mode parity-root --dom-decimals 3 --report-root`

- Generate a report **and** fail on full DOM parity-root mismatches:
  - `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root`

The report is written to:

- `target/compare/flowchart_report.md`

## Current Status

At the time of writing:

- `--dom-mode parity-root` is expected to fail for many Flowchart fixtures, primarily due to numeric
  layout drift that is driven by headless text measurement differences.
- The `--report-root` output helps quantify which fixtures have the largest viewport deltas so we
  can iteratively close the gap.

