# merman-uniffi

UniFFI bindings for Merman headless Mermaid rendering.

`merman-uniffi` exposes the shared native binding facade through UniFFI. It is primarily used by the
experimental Python package scaffold, while `merman-ffi` remains the stable C ABI entry point for
native hosts.

Generated bindings use the built-in headless measurer by default. GUI and WebView hosts that need
their platform font stack can use `MermanReusableEngine` with a `MermanTextMeasurer` callback.
Call `MermanReusableEngine::set_text_measurer` to install a host measurer later, and
`MermanReusableEngine::clear_text_measurer` to restore the built-in measurer. Return `None` from
the callback for requests the host deliberately leaves to fallback metrics; return `Err` only for
host failures that should make reusable render/layout calls fail with `MermanError`.
`ascii_capabilities()` exposes ASCII support grades and summary fallback metadata.
`diagram_family_capabilities()` exposes the same parser/render discovery information as the C ABI
metadata surface.
`analyze_document_json()` and `analyze_document_facts_json()` expose Markdown/MDX-aware diagnostics
and syntax facts for hosts that need editor, lint, or LSP-style document ranges.
`lint_rule_catalog()` and `configurable_lint_rule_catalog()` expose governed analyzer rule metadata,
including evidence references, for package settings, diagnostics UI, and LSP integrations.

Most applications should start with one of these package-level entry points:

- Python package notes: [`docs/bindings/PYTHON_UNIFFI.md`](https://github.com/Latias94/merman/blob/main/docs/bindings/PYTHON_UNIFFI.md)
- Python package scaffold: [`platforms/python/merman`](https://github.com/Latias94/merman/tree/main/platforms/python/merman#readme)
- C ABI: [`merman-ffi`](https://crates.io/crates/merman-ffi)
- Rust facade: [`merman`](https://crates.io/crates/merman)

## Features

- `render` enables SVG rendering.
- `ascii` enables ASCII/Unicode text rendering.
- `raster` enables PNG/JPG/PDF conversion.
- `ratex-math` enables the RaTeX math label backend.
- `bindgen-smoke` enables the local UniFFI binding generation smoke example.

For product scope, diagram coverage, and compatibility policy, see the
[project README](https://github.com/Latias94/merman#readme) and
[alignment status](https://github.com/Latias94/merman/blob/main/docs/alignment/STATUS.md).
