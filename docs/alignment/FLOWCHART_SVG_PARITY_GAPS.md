# Flowchart SVG Parity Gaps (Mermaid@11.12.2)

This note tracks known remaining `compare-flowchart-svgs` DOM parity mismatches for the Stage B
flowchart renderer.

Reproduce:

- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3`

## Known mismatches

None (last checked 2026-01-20).

## Deferred fixtures (not yet parity-gated)

The upstream docs include directive-driven examples that currently would not pass Stage B DOM parity
if imported as fixtures, so they are intentionally deferred until full support is implemented:

- `repo-ref/mermaid/packages/mermaid/src/docs/config/directives.md`
  - legacy `graph TD` example with `%%{init: { "flowchart": { "curve": "linear" } } }%%`
  - legacy `graph TD` example with `%%{init: { "theme": "forest" } }%%`

## Next steps

- Keep `compare-flowchart-svgs` in CI (or run it locally) to catch regressions when expanding the
  fixture set or refactoring the Dagre adapter / SVG renderer.
