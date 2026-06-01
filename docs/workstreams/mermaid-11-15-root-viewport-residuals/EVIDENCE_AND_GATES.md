# Mermaid 11.15 Root Viewport Residuals - Evidence And Gates

Status: Active
Last updated: 2026-06-01

## Starting Evidence

Fresh gates from 2026-06-01:

- `cargo run -p xtask -- verify-generated`: passed.
- `cargo run -p xtask -- check-alignment`: passed.
- `cargo run -p xtask -- report-overrides --check-no-growth`: passed with root viewport
  overrides = 282, text metric lookup overrides = 495, SVG text metric table rows = 186, and
  Flowchart font metric table rows = 3774.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`:
  passed for the implemented matrix.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3`:
  failed normally after the bounded-summary xtask fix.
- `cargo nextest run -p xtask root_parity`: passed, 5 tests.
- `cargo fmt --check`: passed.
- `git diff --check`: passed with a line-ending warning for the parent workstream
  `CONTEXT.jsonl` only.

Full `parity-root` accepted existing policy residuals:

- class: 2 accepted rows.
- sequence: 1 accepted row.
- gitgraph: 1 accepted row.
- mindmap: 4 accepted rows.

Fresh unaccepted residual summary from the full `parity-root` failure:

| Diagram | Unaccepted residuals | Report |
| --- | ---: | --- |
| Sequence | 168 | `target/compare/sequence_report_parity_root.md` |
| Flowchart | 61 | `target/compare/flowchart_report_parity_root.md` |
| Architecture | 32 | `target/compare/architecture_report_parity_root.md` |
| Class | 18 | `target/compare/class_report_parity_root.md` |
| C4 | 15 | `target/compare/c4_report_parity_root.md` |
| Timeline | 7 | `target/compare/timeline_report_parity_root.md` |
| ER | 3 | `target/compare/er_report_parity_root.md` |
| Sankey | 3 | `target/compare/sankey_report_parity_root.md` |
| Journey | 2 | `target/compare/journey_report_parity_root.md` |

Total unaccepted residuals: 309.

## Gate Set

Run after any code or generated-data change:

```bash
cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3
cargo run -p xtask -- report-overrides --check-no-growth
cargo fmt --check
git diff --check
```

Run when changing root policy, root overrides, or emitted bounds:

```bash
cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3
```

The `parity-root` command is allowed to fail while this lane is active, but it must fail with
bounded summaries and fresh per-diagram reports.
