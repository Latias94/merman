# merman-uniffi

[![Crates.io](https://img.shields.io/crates/v/merman-uniffi.svg)](https://crates.io/crates/merman-uniffi)
[![Documentation](https://docs.rs/merman-uniffi/badge.svg)](https://docs.rs/merman-uniffi)
[![License: MIT](https://img.shields.io/badge/license-MIT-yellow)](https://github.com/Latias94/merman/blob/main/LICENSE-MIT)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue)](https://github.com/Latias94/merman/blob/main/LICENSE-APACHE)

Generate high-level native bindings for Merman's headless Mermaid parser, analyzer, layout engine, and renderers with [UniFFI](https://mozilla.github.io/uniffi-rs/). This crate is the source of the published Python package; native hosts that need a stable language-neutral boundary should use [`merman-ffi`](https://crates.io/crates/merman-ffi).

> **Alpha:** the generated contract reports ABI `2`, but prerelease ABI 2 records can still be replaced in place before the stable release. Regenerate bindings and ship them with the exact native library used to generate them.

## Build A Binding Library

```sh
cargo build -p merman-uniffi --release
```

For the repository's Python package generator:

```sh
cargo build -p merman-uniffi --features bindgen-smoke
cargo run -p merman-uniffi --features bindgen-smoke --example generate_python_package -- \
  --package-dir platforms/python/merman
```

Most Python users should install [`merman` from PyPI](https://pypi.org/project/merman/) instead of generating bindings locally.

## Exposed Capabilities

Generated bindings provide `MermanEngine` for independent calls and `MermanReusableEngine` for calls that share options or a host text measurer. They expose SVG and terminal rendering, semantic and layout JSON, validation, diagram/document analysis, parser facts, themes, lint metadata, ASCII support grades, and diagram-family capability discovery.

The package uses the same versioned options as the C ABI. Diagnostics and parser-facts payloads remain schema `1`, independently of UniFFI ABI `2`; the current facts v1 contract is parser-only and does not retain the removed TextScan alpha shape.
Call `MermanEngine.runtime_contract_json()` to inspect runtime-contract schema `1`, including the
loaded ABI/package/options versions, feature set, registry facts, and resource descriptor. This is
the authoritative source for profile values; render-disabled generated artifacts report
`resources: null`.

## Text Measurement Ownership

Generated bindings use Merman's deterministic vendored measurer unless a host installs `MermanTextMeasurer` on a reusable engine. ABI 2 exposes 19 exact operations (`0..18`) and requires a matching tagged result kind for every handled operation. Return `None` for work the host cannot answer synchronously; invalid results and callback errors fall back to the operation's vendored implementation.

GUI and WebView integrations should measure with the font stack that displays the final SVG. Server, CLI, test, and documentation workloads should normally retain Merman's built-in measurer. Do not re-enter or replace the measurer on the same reusable engine while its render callback is active.

Python implements the generated `measure(self, request)` callback. One-shot engine methods accept
`options_json` per call; reusable engine methods inherit options from construction. The repository
bindgen test runs `platforms/python/merman/examples/smoke.py` against a freshly generated module and
native library instead of maintaining a second prose-only API contract.

See the [UniFFI contract](https://github.com/Latias94/merman/blob/main/docs/bindings/UNIFFI.md), [Python binding guide](https://github.com/Latias94/merman/blob/main/docs/bindings/PYTHON_UNIFFI.md), and [host measurement guide](https://github.com/Latias94/merman/blob/main/docs/bindings/HOST_TEXT_MEASUREMENT.md) for generated names and lifecycle rules.

## Features

Defaults enable the full diagram registry, host environment, SVG rendering, analysis, and terminal output. `analysis`, `render`, and `ascii` can be selected independently; `ratex-math` and `raster` add their shared backends. `bindgen-smoke` is a development feature for the local Python generator.

Unavailable feature-gated operations report a structured binding error. The generated API is synchronous, and each generated language wrapper remains responsible for native-library packaging and platform lifecycle integration.

## Links

- [Python package](https://pypi.org/project/merman/)
- [Python package source and changelog](https://github.com/Latias94/merman/tree/main/platforms/python/merman#readme)
- [C ABI crate](https://crates.io/crates/merman-ffi)
- [Merman project and coverage](https://github.com/Latias94/merman#readme)
- [Project changelog](https://github.com/Latias94/merman/blob/main/CHANGELOG.md)

## License And Notices

Merman is available under MIT or Apache-2.0. The crate archive includes the release-matched
`LICENSE-MIT` and `LICENSE-APACHE` texts. Project-wide source provenance and third-party legal
materials are recorded in [`THIRD_PARTY_NOTICES.md`](https://github.com/Latias94/merman/blob/main/THIRD_PARTY_NOTICES.md).
