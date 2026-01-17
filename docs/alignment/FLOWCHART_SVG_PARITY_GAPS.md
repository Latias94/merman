# Flowchart SVG Parity Gaps (Mermaid@11.12.2)

This note tracks known remaining `compare-flowchart-svgs` DOM parity mismatches for the Stage B
flowchart renderer.

Reproduce:

- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3`

## Known mismatches

None (last checked 2026-01-17).

## Next steps

- Keep `compare-flowchart-svgs` in CI (or run it locally) to catch regressions when expanding the
  fixture set or refactoring the Dagre adapter / SVG renderer.
