# UniFFI Bindings

`merman-uniffi` is Merman's direct object binding for Swift, Kotlin, and Python hosts. It calls
`merman-bindings-core` directly and has its own binding API version. It is not a wrapper around the
C ABI, and applications must not mix a generated UniFFI source projection with a different native
library build.

The current direct binding API is `3`. Its runtime contract is schema `1`; the C ABI and the
text-measurement protocol have separate version ownership.

## Public Model

The generated API exposes:

- `MermanEngine` for one-shot operations;
- `MermanReusableEngine` for operations that share baseline options and optional host text
  measurement;
- `MermanOperationRequest` and `MermanOperationResult` for generic descriptor-owned dispatch;
- `resource_options_json` / generated `resourceOptionsJson` for versioned resource profiles;
- `MermanTextMeasurer` for synchronous host measurement; and
- structured `MermanError::Binding { code, code_name, kind, capability_id, message }` failures.

`MermanEngine::binding_api_version()` reports `3`. Use `runtime_catalog_json()` to inspect the
atomic runtime catalog: loaded package/options versions, capability and output IDs, registry facts,
resource limits, and the descriptor-owned vocabulary used to validate those identifiers. Do not
copy capability IDs into a language wrapper.

Every operation is available through `execute(request)`, and `MermanOperationRequest.options_json`
owns the generic operation's options. Named methods such as
`render_svg`, `render_png`, `render_jpeg`, `render_pdf`, `render_ascii`, `parse_json`,
`layout_json`, `analyze_json`, and `validate` are convenience wrappers over that same operation
catalog. An unavailable operation returns a structured missing-capability error instead of a
transport-specific stub result.

One-shot execution constructs a fresh engine from the request options, so a request may explicitly
select `runtime_policy`. A reusable engine instead deeply merges each request's options over its
construction baseline; nested objects merge recursively, while arrays and scalar leaves replace
the baseline value. The reusable engine's baseline remains unchanged, and request options cannot
change its constructor-owned runtime policy. Reusable named methods expose the same request-local
`options_json` argument and follow the same merge rules.

`MermanErrorKind::UnknownOperation` identifies an operation outside the descriptor vocabulary and has no
capability ID. `MermanErrorKind::MissingCapability` identifies a valid request whose artifact lacks
the named descriptor capability. Other failures use `Generic` and a null capability ID.

The generated API shape is stable across feature profiles. A build without `analysis` still
exposes `lint_rule_catalog()` and `configurable_lint_rule_catalog()`, but those calls return
`MissingCapability` with capability ID `analysis`. A build without `svg` still exposes
`MermanTextMeasurer` and the reusable-engine text-measurer constructor and mutation methods; those
entrypoints return `MissingCapability` with capability ID `svg`. Consumers can therefore use one
generated projection and handle artifact capability differences as typed runtime errors.

## Build Profiles

`merman-uniffi` has no default features. The complete native language SDK artifact lists analysis,
ASCII, SVG, PNG, JPEG, PDF, Cytoscape and ELK layouts, RaTeX math, and native
clock/timezone/random/timing adapters directly. `bindgen-smoke` is only for foreign-language
generation and does not belong in a distributed runtime artifact.

```bash
cargo build -p merman-uniffi --release --no-default-features --features 'svg,analysis,ascii,png,jpeg,pdf,layout-cytoscape,layout-elk,math,system-clock,system-timezone,system-random'
```

Small artifacts can select `analysis`, `ascii`, `svg`, `png`, `jpeg`, `pdf`,
`layout-cytoscape`, `layout-elk`, `math`, and the required system-adapter features explicitly.
`png`, `jpeg`, `pdf`, `layout-cytoscape`, `layout-elk`, and `math` all imply `svg`.

## Python

The repository ships a Python package layout. Generate it from the exact cdylib that will be
packaged:

```bash
cargo build -p merman-uniffi --release --no-default-features --features 'svg,analysis,ascii,png,jpeg,pdf,layout-cytoscape,layout-elk,math,system-clock,system-timezone,system-random'
cargo run -p merman-uniffi --no-default-features --features 'svg,analysis,ascii,png,jpeg,pdf,layout-cytoscape,layout-elk,math,system-clock,system-timezone,system-random,bindgen-smoke' \
  --example generate_python_package -- \
  --cdylib target/release/libmerman_uniffi.dylib \
  --package-dir platforms/python/merman
```

Use `.so` on Linux and `merman_uniffi.dll` on Windows. The generator enables `bindgen-smoke`, but
the copied production library is the separately built release artifact without that tool feature.

See [Python UniFFI](PYTHON_UNIFFI.md) for package and wheel details.

## Apple Swift

Apple is a direct UniFFI lane. Its checked-in generated `Merman.swift` imports the `MermanFFI`
module inside the matching `Merman.xcframework`; no hand-written C façade remains. Generate the
Swift source, header, and module map from the exact static library:

```bash
cargo run -p merman-uniffi --no-default-features --features 'svg,analysis,ascii,png,jpeg,pdf,layout-cytoscape,layout-elk,math,system-clock,system-timezone,system-random,bindgen-smoke' \
  --example generate_swift_bindings -- \
  --library target/aarch64-apple-darwin/release/libmerman_uniffi.a \
  --output-dir platforms/apple/Sources/Merman/Generated
```

Use `scripts/build-apple-xcframework.sh` for normal packaging. It builds every selected slice,
regenerates the three source artifacts, and embeds the generated header/module map in each slice.
See [Apple Swift](APPLE_SWIFT.md) for the SwiftPM API and smoke command.

## Text Measurement

Merman uses a deterministic vendored text measurer by default. A UI host can install a
`MermanTextMeasurer` on a reusable engine when its Core Text, Android, or another platform font
stack must determine the layout. The callback receives the independent text-measurement protocol
version `1` and a typed operation. Return `None`/`nil` for work that is unavailable or cannot be
answered synchronously; Merman falls back to its vendored implementation for that operation.

Do not re-enter or replace a reusable engine while a measurement callback is active. See
[host text measurement](HOST_TEXT_MEASUREMENT.md) for its operation, result-shape, and lifecycle
contract.

## Verification

```bash
cargo nextest run -p merman-uniffi --no-default-features \
  --features 'svg,analysis,ascii,png,jpeg,pdf,layout-cytoscape,layout-elk,math,system-clock,system-timezone,system-random,bindgen-smoke' --test bindgen_smoke
```

The binding smoke builds a library, generates foreign-language source from its embedded UniFFI
metadata, and exercises the public API. Apple CI additionally rebuilds its XCFramework, compiles
the Swift package and smoke, and rejects a changed checked-in Swift projection.
