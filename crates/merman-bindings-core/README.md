# merman-bindings-core

Safe shared binding facade for Merman native bindings.

`merman-bindings-core` is an implementation crate used by the C ABI and UniFFI binding crates. It
keeps the JSON options contract, error mapping, and feature-gated render entry points in one place
so platform bindings expose the same behavior.

Most applications should use one of the public packages instead:

- Rust: [`merman`](https://crates.io/crates/merman)
- C ABI and native hosts: [`merman-ffi`](https://crates.io/crates/merman-ffi)
- Python/UniFFI packaging: [`merman-uniffi`](https://crates.io/crates/merman-uniffi)

## Features

- `render` enables SVG rendering through the main Merman facade.
- `ascii` enables ASCII/Unicode text rendering.
- `raster` enables PNG/JPG/PDF conversion through the main facade.
- `ratex-math` enables the RaTeX math label backend.

For product scope, diagram coverage, and compatibility policy, see the
[project README](https://github.com/Latias94/merman#readme) and
[alignment status](https://github.com/Latias94/merman/blob/main/docs/alignment/STATUS.md).
