## Deferred fixtures

This folder contains upstream Mermaid fixtures that are intentionally excluded from our SVG DOM parity gates.

Common reasons:

- The upstream renderer (`mermaid-cli` / `mmdc`) fails to baseline-render the input (parse/render/runtime error).
- The fixture is valid upstream but intentionally out-of-scope for parity gating (e.g. a family-specific `look: handDrawn` branch without rendered RoughJS evidence, unsupported non-public ELK behavior, or math/KaTeX in HTML labels).
- The fixture is a duplicate of an active parity fixture after removing frontmatter. For example, public Flowchart/Class `layout: elk` deferred copies are reported as absorbed once active source-backed fixtures cover the same Mermaid-reachable body.

Notes:

- These files are kept to preserve coverage and make future alignment work easier.
- `xtask import-* --with-baselines` may move candidates here instead of deleting them when baseline generation fails.
