# merman-uniffi

[![Crates.io](https://img.shields.io/crates/v/merman-uniffi.svg)](https://crates.io/crates/merman-uniffi)
[![Documentation](https://docs.rs/merman-uniffi/badge.svg)](https://docs.rs/merman-uniffi)
[![License: MIT](https://img.shields.io/badge/license-MIT-yellow)](https://github.com/Latias94/merman/blob/main/LICENSE-MIT)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue)](https://github.com/Latias94/merman/blob/main/LICENSE-APACHE)

Generate high-level native bindings for Merman's headless Mermaid parser, analyzer, layout engine, and renderers with [UniFFI](https://mozilla.github.io/uniffi-rs/). This crate is the source of the published Python package and the Apple SwiftPM package; native hosts that need a stable language-neutral C boundary should use [`merman-ffi`](https://crates.io/crates/merman-ffi).

> **Alpha:** the direct UniFFI binding API is `3`. It is not the C ABI and not the text-measurement protocol; all three have independent version ownership. Regenerate bindings and ship them with the exact native library used to generate them.

## Build A Binding Library

```sh
cargo build -p merman-uniffi --release --no-default-features --features 'svg,analysis,ascii,png,jpeg,pdf,layout-cytoscape,layout-elk,math,system-clock,system-timezone,system-random'
```

For the repository's Python package generator:

```sh
cargo build -p merman-uniffi --release --no-default-features --features 'svg,analysis,ascii,png,jpeg,pdf,layout-cytoscape,layout-elk,math,system-clock,system-timezone,system-random'
cargo run -p merman-uniffi --no-default-features --features 'svg,analysis,ascii,png,jpeg,pdf,layout-cytoscape,layout-elk,math,system-clock,system-timezone,system-random,bindgen-smoke' --example generate_python_package -- \
  --cdylib target/release/libmerman_uniffi.dylib \
  --package-dir platforms/python/merman
```

Most Python users should install [`merman` from PyPI](https://pypi.org/project/merman/) instead of generating bindings locally.

For the Apple SwiftPM projection, build a static library for the intended Apple target and generate
the checked-in source/header/module map from that exact artifact:

```sh
cargo run -p merman-uniffi --no-default-features --features 'svg,analysis,ascii,png,jpeg,pdf,layout-cytoscape,layout-elk,math,system-clock,system-timezone,system-random,bindgen-smoke' \
  --example generate_swift_bindings -- \
  --library target/aarch64-apple-darwin/release/libmerman_uniffi.a \
  --output-dir platforms/apple/Sources/Merman/Generated
```

Normally use `bash scripts/build-apple-xcframework.sh`, which performs both steps and packages all
selected slices.

## Exposed Capabilities

Generated bindings provide `MermanEngine` for independent calls and `MermanReusableEngine` for calls that share options or a host text measurer. They expose semantic JSON and, when the matching output capability is selected, SVG, PNG, JPEG, PDF, terminal rendering, layout JSON, validation, diagram/document analysis, parser facts, themes, lint metadata, ASCII support grades, and diagram-family capability discovery.

The generated API shape remains stable for smaller feature profiles. Lint catalog calls return a
structured `analysis` missing-capability error when analysis is absent. `MermanTextMeasurer` and
its reusable-engine entrypoints remain generated when SVG is absent and return a structured `svg`
missing-capability error when called.

`MermanOperationRequest` and `execute()` are the transport-neutral path for every output. Named
methods such as `render_svg()` and `render_png()` are convenience wrappers over that same
descriptor-owned dispatch. Generic request options live in
`MermanOperationRequest.options_json`; `execute()` has no parallel options argument. One-shot
requests construct a fresh engine and may select `runtime_policy`. Reusable request options deeply
merge over the construction baseline without mutating it and cannot change its constructor-owned
runtime policy. The package uses the same versioned options as the C ABI. Diagnostics remain schema
`1` and parser-facts are schema `1`, independently of UniFFI binding API `3`; other versions are
rejected at the boundary and the removed TextScan shape is not retained.

Call `MermanEngine.runtime_catalog_json()` to inspect the atomic runtime catalog. It contains
flat schema `1`, including the loaded transport API and package identity, sorted compiled
capability/output/operation/system-adapter IDs, text-measurement providers, registry facts, and the
resource descriptor. Validate local relations and tolerate newly added stable IDs; do not maintain a
second language-specific copy of Merman's global vocabulary. This is the authoritative source for
profile values. Every artifact reports the shared resource descriptor because source limits apply
before an output backend is selected.

Omitting `runtime_policy` from `options_json` always selects deterministic runtime state, regardless
of which system adapters were compiled into the library. Set `{"runtime_policy":"native"}` to opt
into the system clock, time-zone, and random adapters; a slim artifact that lacks one returns a
typed unsupported-operation error. `MermanOperationResult.metadata_json` records the selected
policy for each successful generic operation.

Generated `MermanError.Binding` values expose `MermanErrorKind` plus an optional `capability_id`.
Unknown operations have no capability ID; known requests missing a backend preserve the exact
descriptor capability ID.

## Text Measurement Ownership

Generated bindings use Merman's deterministic vendored measurer unless a host installs `MermanTextMeasurer` on a reusable engine. Text-measurement protocol 1 exposes 19 exact operations (`0..18`) and requires a matching tagged result kind for every handled operation. Return `None` for work the host cannot answer synchronously; invalid results and callback errors fall back to the operation's vendored implementation.

GUI and WebView integrations should measure with the font stack that displays the final SVG. Server, CLI, test, and documentation workloads should normally retain Merman's built-in measurer. While a host measurement callback is active, every new operation or measurer mutation on that same reusable engine fails immediately with typed `MermanErrorKind::ReentrantCall`, on any thread. This deliberately includes otherwise independent callers because UniFFI cannot prove whether a foreign thread was callback-derived; use a separate engine with independently synchronized host state for work that must remain independent.

Python implements the generated `measure(self, request)` callback. One-shot engine methods accept
`options_json` per call; reusable engine methods merge request-local options over their construction
baseline. The repository bindgen test runs `platforms/python/merman/examples/smoke.py` against a
freshly generated module and native library instead of maintaining a second prose-only API
contract.

See the [UniFFI contract](https://github.com/Latias94/merman/blob/main/docs/bindings/UNIFFI.md), [Python binding guide](https://github.com/Latias94/merman/blob/main/docs/bindings/PYTHON_UNIFFI.md), and [host measurement guide](https://github.com/Latias94/merman/blob/main/docs/bindings/HOST_TEXT_MEASUREMENT.md) for generated names and lifecycle rules.

## Features

This crate has no default features. `analysis`, `svg`, and `ascii` can be selected independently;
`layout-cytoscape`, `layout-elk`, and `math` imply `svg` and add their named SVG backends.
`png`, `jpeg`, and `pdf` each imply `svg` and expose their matching byte-returning methods. There
is intentionally no broad `render` or `raster` feature: applications opt into the outputs they
actually call. The complete native SDK artifact lists its direct output and system-adapter features
explicitly. `bindgen-smoke` is a development feature for local Python and Swift source generation;
do not include it in a distributed native library.

Unavailable feature-gated operations report a structured binding error. The generated API is synchronous, and each generated language wrapper remains responsible for native-library packaging and platform lifecycle integration.

## Links

- [Python package](https://pypi.org/project/merman/)
- [Python package source and changelog](https://github.com/Latias94/merman/tree/main/platforms/python/merman#readme)
- [Apple Swift package](https://github.com/Latias94/merman/tree/main/platforms/apple#readme)
- [UniFFI binding guide](https://github.com/Latias94/merman/blob/main/docs/bindings/UNIFFI.md)
- [C ABI crate](https://crates.io/crates/merman-ffi)
- [Merman project and coverage](https://github.com/Latias94/merman#readme)
- [Project changelog](https://github.com/Latias94/merman/blob/main/CHANGELOG.md)

## License And Notices

Merman is available under MIT or Apache-2.0. The crate archive includes the release-matched
`LICENSE-MIT` and `LICENSE-APACHE` texts. Project-wide source provenance and third-party legal
materials are recorded in [`THIRD_PARTY_NOTICES.md`](https://github.com/Latias94/merman/blob/main/THIRD_PARTY_NOTICES.md).
