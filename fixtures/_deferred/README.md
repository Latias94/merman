## Deferred fixtures

This folder contains upstream Mermaid fixtures that are intentionally excluded from our SVG DOM parity gates.

Common reasons:

- The upstream renderer (`mermaid-cli` / `mmdc`) fails to baseline-render the input (parse/render/runtime error).
- The fixture is valid upstream but intentionally out-of-scope for parity gating (e.g. non-classic look, ELK layout, math/KaTeX in HTML labels).

Notes:

- These files are kept to preserve coverage and make future alignment work easier.
- `xtask import-* --with-baselines` may move candidates here instead of deleting them when baseline generation fails.
