# Merman Render Assets

This directory contains runtime-only renderer support files loaded relative to
`CARGO_MANIFEST_DIR`.

- `katex_flowchart_probe.cjs` is used by the optional Node.js KaTeX probe backend for HTML/math
  measurement audits.

Compile-time assets belong beside their owning module under `src/`; this directory must not contain
inputs embedded into the Rust library.
