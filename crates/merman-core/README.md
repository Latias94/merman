# merman-core

Headless Mermaid parser + semantic JSON model.

Baseline: Mermaid `@11.12.2` (upstream Mermaid is treated as the spec).

This crate focuses on:

- Diagram detection
- Parsing into a normalized semantic model (`serde_json::Value`)
- Mermaid config parsing / defaults used by downstream stages

If you also need layout and SVG, use `merman-render` (or the `merman` wrapper crate).

Parity policy and gates live in `docs/alignment/STATUS.md` in the repository.

