# UniFFI Bindings

`merman-uniffi` is Merman's direct object binding for Swift, Kotlin, and Python hosts. It calls
`merman-bindings-core` directly and has its own binding API version. It is not a wrapper around the
C ABI, and applications must not mix a generated UniFFI source projection with a different native
library build.

The current direct binding API is `3`. Its runtime contract is schema `1`; the C ABI and the
text-measurement protocol have separate version ownership.

## Public Model

The generated API exposes:

- `Merman` for discovery, metadata, and one-shot operations;
- `MermanEngine` for reusable operations with immutable constructor options and services;
- `MermanEngineServices`, `MermanIconPack`, and `MermanIconRegistry` for constructor-owned host
  services;
- `MermanOperationRequest` and `MermanOperationResult` for generic descriptor-owned dispatch;
- `resource_options_json` / generated `resourceOptionsJson` for Options JSON schema `2` profiles and request-local overrides;
- `MermanTextMeasurer` for synchronous host measurement; and
- structured `MermanError::Binding { code, code_name, kind, capability_id, resource, message }` failures, where `resource` is optional typed limit evidence.

`Merman::binding_api_version()` reports `3`. Use `runtime_catalog_json()` to inspect the
atomic runtime catalog: loaded package/options versions, capability and output IDs, registry facts,
resource limits, and the descriptor-owned vocabulary used to validate those identifiers. Do not
copy capability IDs into a language wrapper.

Every operation is available through `execute(request)`, and `MermanOperationRequest.options_json`
owns the generic operation's options. Named methods such as
`render_svg`, `render_png`, `render_jpeg`, `render_pdf`, `render_ascii`, `parse_json`,
`layout_json`, `analyze_json`, and `validate` are convenience wrappers over that same operation
catalog. An unavailable operation returns a structured missing-capability error instead of a
transport-specific stub result.

The generated resource helper takes an optional profile. Leave it unset when a reusable request
must inherit its constructor ceiling; only generated override IDs can be serialized into
`resources.limits`. `Merman` one-shot execution constructs a fresh engine from the request options,
so a request may explicitly select `runtime_policy`. `MermanEngine::new(options_json, services)`
instead deeply merges each request's options over its construction baseline; nested objects merge
recursively, while arrays and scalar leaves replace the baseline value. The engine's baseline
remains unchanged, and request options cannot change its constructor-owned runtime policy.
Reusable named methods expose the same request-local `options_json` argument and follow the same
merge rules.

`MermanErrorKind::UnknownOperation` identifies an operation outside the descriptor vocabulary and has no
capability ID. `MermanErrorKind::MissingCapability` identifies a valid request whose artifact lacks
the named descriptor capability. Other failures use `Generic` and a null capability ID.

The generated API shape is stable across feature profiles. A build without `analysis` still
exposes `lint_rule_catalog()` and `configurable_lint_rule_catalog()`, but those calls return
`MissingCapability` with capability ID `analysis`. A build without `svg` still exposes
`MermanTextMeasurer` and `MermanEngineServices`; constructing an engine with either icon or text
measurement services returns `MissingCapability` with capability ID `svg`. Consumers can therefore
use one generated projection and handle artifact capability differences as typed runtime errors.

`MermanEngine::close()` is explicit and idempotent. A busy or re-entrant close preserves the
complete engine and service graph for retry; a successful close detaches the engine under the
admission lock and destroys foreign callbacks only after the lock is released. Always close an
engine that retains a foreign callback because the callback may capture the engine and form a
reference cycle.

## Build Profiles

`merman-uniffi` has no default features. The complete native language SDK artifact lists analysis,
ASCII, SVG, PNG, JPEG, PDF, Cytoscape and ELK layouts, RaTeX math, and native
clock/timezone/random adapters directly. Timing is not part of the UniFFI artifact contract.
`bindgen-smoke` is only for foreign-language
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

Merman uses a deterministic vendored text measurer by default. A UI host can place a
`MermanTextMeasurer` in `MermanEngineServices` when it directly constructs a `MermanEngine` whose
Core Text, Android, or other platform font stack must determine layout. The callback is immutable
for that engine. It receives
the independent text-measurement protocol version `1` and a typed operation. Return `None`/`nil`
for work that is unavailable or cannot be answered synchronously; Merman falls back to its
vendored implementation for that operation.

Callback-free reusable engines admit concurrent operations. A callback engine serializes operation
admission and returns `Busy` to a competitor; an operation started while the same engine's callback
is active returns `ReentrantCall`. A close attempted from the callback also returns
`ReentrantCall`, retains the callback, and can be retried later. UniFFI's generated trampoline can
report a returned callback error to Rust, but Merman does not catch arbitrary foreign unwinds,
exceptions, or longjmps that bypass that boundary. See
[host text measurement](HOST_TEXT_MEASUREMENT.md) for the operation, result-shape, and lifecycle
contract.

## Migrating From The Previous Prerelease API

- Replace one-shot `MermanEngine` values with `Merman`.
- Replace `MermanReusableEngine` and facade factories such as `reusable_engine(...)` with the
  direct `MermanEngine(options_json, services)` constructor.
- Put icon registries and text measurement in `MermanEngineServices`; there are no
  callback-specialized constructors or post-construction service mutators.
- Call `close()` deterministically. Do not rely on garbage collection to release foreign callback
  cycles.
- Use result-returning binary methods when callers need operation metadata or the effective output
  plan; byte-returning conveniences remain available.
- Treat runtime operation, metadata, option-group, constructor-service, and resource-limit IDs as
  open discovery values. Closed request-input vocabularies remain generated enums/value sets.

## Verification

```bash
cargo nextest run -p merman-uniffi --no-default-features \
  --features 'svg,analysis,ascii,png,jpeg,pdf,layout-cytoscape,layout-elk,math,system-clock,system-timezone,system-random,bindgen-smoke' --test bindgen_smoke
```

The binding smoke builds a library, generates foreign-language source from its embedded UniFFI
metadata, and exercises the public API. Apple CI additionally rebuilds its XCFramework, compiles
the Swift package and smoke, and rejects a changed checked-in Swift projection.
