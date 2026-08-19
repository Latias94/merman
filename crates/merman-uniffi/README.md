# merman-uniffi

[![Crates.io](https://img.shields.io/crates/v/merman-uniffi.svg)](https://crates.io/crates/merman-uniffi) [![Documentation](https://docs.rs/merman-uniffi/badge.svg)](https://docs.rs/merman-uniffi) [![License: MIT](https://img.shields.io/badge/license-MIT-yellow)](https://github.com/Latias94/merman/blob/main/LICENSE-MIT) [![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue)](https://github.com/Latias94/merman/blob/main/LICENSE-APACHE)

Generate high-level native bindings for Merman's headless Mermaid parser, analyzer, layout engine, and renderers with [UniFFI](https://mozilla.github.io/uniffi-rs/). This crate is the source of the published Python package and the Apple SwiftPM package; native hosts that need a stable language-neutral C boundary should use [`merman-ffi`](https://crates.io/crates/merman-ffi).

> **Alpha:** the direct UniFFI binding API is `5`. It is not the C ABI and not the text-measurement protocol; all three have independent version ownership. Regenerate bindings and ship them with the exact native library used to generate them.

## Use A Published Wrapper

Most applications should not generate UniFFI bindings themselves:

- Python users install [`merman` from PyPI](https://pypi.org/project/merman/).
- Swift users consume the [Merman Apple package](https://github.com/Latias94/merman/tree/main/platforms/apple#readme).
- Hosts that need a language-neutral ABI use [`merman-ffi`](https://crates.io/crates/merman-ffi).

Use this crate directly when maintaining one of those projections or building a new UniFFI host.

## Build A Binding Library

```sh
cargo build -p merman-uniffi --release --no-default-features --features 'svg,analysis,ascii,png,jpeg,pdf,layout-cytoscape,layout-elk,math,native-runtime'
```

For the repository's Python package generator:

```sh
cargo build -p merman-uniffi --release --no-default-features --features 'svg,analysis,ascii,png,jpeg,pdf,layout-cytoscape,layout-elk,math,native-runtime'
export MERMAN_UNIFFI_LIBRARY=target/release/libmerman_uniffi.dylib
cargo run -p merman-uniffi --no-default-features --features binding-generation --example generate_python_package -- \
  --cdylib "$MERMAN_UNIFFI_LIBRARY" \
  --package-dir platforms/python/merman
```

The library filename is `libmerman_uniffi.dylib` on macOS, `libmerman_uniffi.so` on Linux, and `merman_uniffi.dll` on Windows. The repository's wheel builder selects the descriptor-owned target and path automatically.

For the Apple SwiftPM projection, build a static library for the intended Apple target and generate the checked-in source/header/module map from that exact artifact:

```sh
cargo run -p merman-uniffi --no-default-features --features binding-generation \
  --example generate_swift_bindings -- \
  --library target/aarch64-apple-darwin/release/libmerman_uniffi.a \
  --output-dir platforms/apple/Sources/Merman/Generated
```

Normally use `bash scripts/build-apple-xcframework.sh`, which performs both steps and packages all selected slices.

## Exposed Capabilities

Generated bindings provide `Merman` for discovery and one-shot calls and `MermanEngine` for reusable calls that share options or constructor-owned services. They expose semantic JSON and, when the matching output capability is selected, SVG, PNG, JPEG, PDF, terminal rendering, layout JSON, validation, diagram/document analysis, parser facts, Mermaid themes, the open-ended presentation catalog, lint metadata, ASCII support grades, and diagram-family capability discovery.

The generated API shape remains stable for smaller feature profiles. Lint catalog calls return a structured `analysis` missing-capability error when analysis is absent. `MermanTextMeasurer` and its reusable-engine entrypoints remain generated when SVG is absent and return a structured `svg` missing-capability error when called.

`MermanOperationRequestV4` and `execute()` are the transport-neutral path for every output. Named methods such as `render_svg()` and `render_png()` are convenience wrappers over that same descriptor-owned dispatch. Generic request options live in `MermanOperationRequestV4.options_json`; `execute()` has no parallel options argument. Set `MermanOperationRequestV4.control` to a `MermanOperationControl` when the host needs to cancel or deadline one operation. Construct the control with an optional relative `timeout_ms`, retain another reference, and call `cancel()` from a worker or callback thread; the request clones the shared control before synchronous execution and does not hold a registry lock while rendering. Cancellation is cooperative, so an opaque callback may finish before the next checkpoint. One-shot requests construct a fresh engine and may select `runtime_policy`. Reusable request options deeply merge over the construction baseline without mutating it and cannot change its constructor-owned runtime policy. The package uses the same versioned options as the C ABI. Diagnostics remain schema `1` and parser facts use schema `2`, independently of UniFFI binding API `5`; other facts versions are rejected at the boundary, the removed TextScan shape is not retained, and the Flowchart-only rich graph is no longer part of the facts payload.

API 5 changes `MermanAsciiCapability`: replace `summary_fallback` with
`structured_text_fallback`, consume `semantic_coverage` and `primary_projection` directly, and
treat `support_level` as a derived compatibility view. `MermanError.Binding` also adds structured
diagnostic details.

API 5 replaces the API 4 `transport_api_version()` probe with
`binding_api_version_v5()`. This forces stale generated bindings to fail before decoding the
changed capability and structured-error record layouts.

Call `Merman.runtime_catalog_json()` to inspect the atomic runtime catalog before constructing reusable engines. It contains flat schema `1`, including the loaded transport API and package identity, supported options and binding-payload schema IDs, named metadata IDs, sorted transport-callable capability/output/operation/system-adapter IDs, constructor services, text-measurement providers, registry facts, and the resource descriptor with per-limit operation applicability. The native clock, time-zone, and random adapters appear only as a complete selectable set, and timing instrumentation is never exposed through binding JSON. Validate local relations and tolerate newly added stable IDs; do not maintain a second language-specific copy of Merman's global vocabulary. This is the authoritative source for profile values. Every artifact reports the shared resource descriptor because source limits apply before an output backend is selected.

Omitting `runtime_policy` from `options_json` always selects deterministic runtime state, even when
`native-runtime` is compiled into the library. Set `{"runtime_policy":"native"}` to opt into the
system clock, time-zone, and random adapters. UniFFI artifacts compile those adapters atomically;
the runtime catalog still exposes their concrete `system-clock`, `system-timezone`, and
`system-random` IDs. An artifact without `native-runtime` returns a typed unsupported-operation
error. `MermanOperationResult.metadata` records the selected policy, byte length, raw schema-1 JSON,
and an optional open `MermanOutputPlan` record when the operation has an output plan. Switch on its
string `kind`; `raster` and `pdf_filter_images` provide typed payloads for known plans, while
`raw_json` preserves every current or future plan without a closed foreign-language enum.

Generated `MermanError.Binding` values expose `MermanErrorKind`, an optional `capability_id`, optional `MermanResourceErrorDetails`, optional `MermanDiagnosticErrorDetails`, and optional `MermanCancelledDetails`. Unknown operations have no capability ID; known requests missing a backend preserve the exact descriptor capability ID. Resource failures preserve the stable cause (`ceiling` or `arithmetic_overflow`), limit ID, phase, actual value, effective maximum, and selected profile. Parser and ASCII failures may preserve a stable diagnostic code, optional source span, and bounded field or diagram context without retaining complete source text. Cancellation remains a separate terminal class with `reason` (`requested` or `deadline_exceeded`) and the observed `phase`; none of these cases should be inferred from display text.

## Text Measurement Ownership

Generated bindings use Merman's deterministic vendored measurer unless a host constructs `MermanEngine` with a `MermanEngineServices` value containing `MermanTextMeasurer`. The same immutable service bundle may carry a sealed `MermanIconRegistry` built transactionally from bounded `MermanIconPack` values. Text-measurement protocol 1 exposes 19 exact operations (`0..18`) and requires a matching tagged result kind for every handled operation. Return `None` for work the host cannot answer synchronously; invalid results and errors returned through UniFFI's generated callback trampoline fall back to the operation's vendored implementation. Foreign callback code must not unwind, throw, or longjmp across the generated FFI boundary; Merman does not claim to catch arbitrary foreign unwinds.

Create services with the zero-argument constructor, then chain `with_icon_registry(...)` and
`with_text_measurer(...)`. Each method returns a new immutable bundle and leaves the original
unchanged, so future service additions do not expand a positional constructor.

GUI and WebView integrations should measure with the font stack that displays the final SVG. Server, CLI, test, and documentation workloads should normally retain Merman's built-in measurer. Services are immutable after construction. Callback-free engines admit concurrent operations; callback engines serialize operation admission and return typed `Busy` to a competitor. While a host callback is active, every new operation on that same engine fails immediately with typed `ReentrantCall`, on any thread. Use a separate engine with independently synchronized host state for work that must remain independent. Call `close()` deterministically when an engine retains foreign callbacks; busy or reentrant close remains retryable, and successful close is idempotent.

Python implements the generated `measure(self, request)` callback. One-shot engine methods accept `options_json` per call; reusable engine methods merge request-local options over their construction baseline. The wheel builder runs `platforms/python/merman/examples/smoke.py` against the installed final artifact instead of maintaining a second staged-module smoke.

See the [UniFFI contract](https://github.com/Latias94/merman/blob/main/docs/bindings/UNIFFI.md), [Python binding guide](https://github.com/Latias94/merman/blob/main/docs/bindings/PYTHON_UNIFFI.md), and [host measurement guide](https://github.com/Latias94/merman/blob/main/docs/bindings/HOST_TEXT_MEASUREMENT.md) for generated names and lifecycle rules.

## Features

This crate has no default features. `analysis`, `svg`, and `ascii` can be selected independently; `layout-cytoscape`, `layout-elk`, and `math` imply `svg` and add their named SVG backends. `png`, `jpeg`, and `pdf` each imply `svg` and expose their matching byte-returning methods. There is intentionally no broad `render` or `raster` feature: applications opt into the outputs they actually call. `native-runtime` is the one binding-owned aggregate and enables the complete system clock, time-zone, and random adapter set; partial native-runtime combinations are not supported by this crate. `binding-generation` is a development feature for local Python and Swift source generation; do not include it in a distributed native library.

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

Merman is available under MIT or Apache-2.0. The crate archive includes the release-matched `LICENSE-MIT` and `LICENSE-APACHE` texts. Project-wide source provenance and third-party legal materials are recorded in [`THIRD_PARTY_NOTICES.md`](https://github.com/Latias94/merman/blob/main/THIRD_PARTY_NOTICES.md).
