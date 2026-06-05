# merman-uniffi

UniFFI bindings for Merman headless Mermaid rendering.

`merman-uniffi` exposes the shared native binding facade through UniFFI. It is primarily used by the
experimental Python package scaffold, while `merman-ffi` remains the stable C ABI entry point for
native hosts.

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
