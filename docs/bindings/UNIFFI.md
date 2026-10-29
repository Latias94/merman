# UniFFI Bindings

This guide owns the direct object-binding contract implemented by
[`merman-uniffi`](../../crates/merman-uniffi/src/lib.rs). Swift and Python packages call
`merman-bindings-core` through UniFFI and have an independent binding API version. Android's
[Kotlin package](ANDROID_JNI.md) uses its own JNI transport. Applications must deploy each generated
UniFFI projection with its matching native library.

The current direct binding API is `7`. Its runtime contract is schema `1`; the C ABI and the
text-measurement protocol have separate version ownership.

Theme authoring errors preserve `details_json` with the versioned `theme_authoring`
diagnostic envelope (`code`, `path`, and `details`). Use `theme_catalog_json()` for
the shared catalog; authoring operations remain available through `execute()`.

## Public Model

The generated API exposes:

- `Merman` for discovery, metadata, and one-shot operations;
- `MermanEngine` for reusable operations with immutable constructor options and services;
- `MermanEngineServices`, `MermanIconPack`, and `MermanIconRegistry` for constructor-owned host
  services;
- `MermanOperationRequestV4` and `MermanOperationResult` for generic descriptor-owned dispatch;
- `MermanOperationControl` for caller-owned cancellation and optional monotonic deadlines;
- `resource_options_json` / generated `resourceOptionsJson` for Options JSON schema `3` profiles and request-local overrides;
- `MermanTextMeasurer` for synchronous host measurement; and
- structured `MermanError::Binding { code, code_name, kind, capability_id, resource, diagnostic, icon_registry, cancellation, details_json, message }` failures, where resource, diagnostic, icon-registry, and cancellation evidence remain separate optional records.

`Merman::binding_api_version_v7()` reports `7`. Use `runtime_catalog_json()` to inspect the
atomic runtime catalog: loaded package/options versions, capability and output IDs, registry facts,
resource limits, and the descriptor-owned vocabulary used to validate those identifiers. Do not
copy capability IDs into a language wrapper.

The versioned probe symbol rejects older generated bindings before they decode revised records.
UniFFI method checksums alone do not cover all nested record-field changes. Regenerate the complete
projection when the binding API changes and deploy it with the matching library; the probe and
generated code form one deployment unit. For changes between published releases, use the
[versioned upgrade index](../release/README.md#workspace-upgrades).

Every operation is available through `execute(request)`, and `MermanOperationRequestV4.options_json`
owns the generic operation's options. Named methods such as
`render_svg`, `render_png`, `render_jpeg`, `render_pdf`, `render_ascii`, `parse_json`,
`layout_json`, `analyze_json`, and `validate` are convenience wrappers over that same operation
catalog. An unavailable operation returns a structured missing-capability error instead of a
transport-specific stub result.

Use result-returning binary methods when operation metadata or the effective output plan matters.
`MermanOutputPlan` is an open record: switch on `kind`, inspect optional `raster`,
`pdf_filter_images`, or `ascii`, and preserve `raw_json` for future kinds. The schema-3 ASCII plan
includes projection, encoding, emitted dimensions, viewport fallback, `requested_layout_profile`,
and `compact_attempted`. Its effective `layout_profile` is Canonical or Compact; the request may
select Auto. `MermanAsciiCapability` reports `layout_profiles`, `width_profiles`, `encodings`, and
`fallback_encodings`. Runtime operation, metadata, option-group, constructor-service, and
resource-limit IDs remain open discovery values; request-input vocabularies use generated closed
types.

Set `MermanOperationRequestV4.control` when a host needs to cancel or deadline one synchronous
operation. Retain another reference and call `cancel()` from a different thread or callback. The
request clones the shared control before execution and reports `MermanCancelledDetails` with the
observed reason and phase; cancellation is not projected as a resource limit. Opaque host callbacks
and single-call encoders can only be checked before and after invocation, so hard interruption
still requires worker or process isolation.

The generated resource helper takes an optional profile. Leave it unset when a reusable request
must inherit its constructor ceiling; only generated override IDs can be serialized into
`resources.limits`. `Merman` one-shot execution constructs a fresh engine from the request options,
so a request may explicitly select `runtime_policy`. `MermanEngine::new(options_json, services)`
instead deeply merges each request's options over its construction baseline; nested objects merge
recursively, while arrays and scalar leaves replace the baseline value. The engine's baseline
remains unchanged, and request options cannot change its constructor-owned runtime policy.
Reusable named methods expose the same request-local `options_json` argument and follow the same
merge rules.

`MermanResourceErrorDetails.cause` is the stable resource failure discriminator: `ceiling` means
the effective policy maximum was exceeded, while `arithmetic_overflow` means safe work accounting
could not represent the required amount. Consumers should branch on this field rather than parse
the display message.

`MermanCancelledDetails.reason` is `requested` or `deadline_exceeded`; `phase` identifies the
checkpoint that observed the terminal state. A cancelled operation returns no partial output.

`MermanErrorKind::UnknownOperation` identifies an operation outside the descriptor vocabulary and has no
capability ID. `MermanErrorKind::MissingCapability` identifies a valid request whose artifact lacks
the named descriptor capability. Other failures use `Generic` and a null capability ID.

The generated API shape is stable across feature profiles. A build without `analysis` still
exposes `lint_rule_catalog()` and `configurable_lint_rule_catalog()`, but those calls return
`MissingCapability` with capability ID `analysis`. A build without `svg` still exposes
`MermanTextMeasurer` and `MermanEngineServices`; constructing an engine with either icon or text
measurement services returns `MissingCapability` with capability ID `svg`. Consumers can therefore
use one generated projection and handle artifact capability differences as typed runtime errors.
Create an empty service bundle first, then use the immutable `with_icon_registry` and
`with_text_measurer` builders; adding a future service does not change the constructor signature.

`MermanEngine::close()` is explicit and idempotent. A busy or re-entrant close preserves the
complete engine and service graph for retry; a successful close detaches the engine under the
admission lock and destroys foreign callbacks only after the lock is released. Always close an
engine that retains a foreign callback because the callback may capture the engine and form a
reference cycle.

## Build Profiles

`merman-uniffi` has no default features. The default Python and Apple prebuilt SKU selects
analysis, ASCII, SVG, and both Cytoscape and ELK layouts. It omits PNG, JPEG, PDF, RaTeX math, and
the binding-owned `native-runtime` aggregate. `binding-generation` is only for foreign-language
generation and does not belong in a distributed runtime artifact.

Exact features, diagram selectors, Cargo profiles, and target sets belong to the
`python-uniffi-native` and `apple-uniffi-native`
[artifact recipes](../../capabilities/artifact-profiles-v2.json). Use the platform builders below
rather than maintaining a second recipe here.

Custom artifacts can select `analysis`, `ascii`, `svg`, `png`, `jpeg`, `pdf`,
`layout-cytoscape`, `layout-elk`, and `math` independently. Add `native-runtime` only when native
clock, time-zone, and random behavior is required; UniFFI does not expose partial adapter feature sets.
`png`, `jpeg`, `pdf`, `layout-cytoscape`, `layout-elk`, and `math` all imply `svg`.
Runtime discovery still reports `system-clock`, `system-timezone`, and `system-random` as concrete
adapter IDs rather than exposing the Cargo aggregate as a capability.

## Python

The [Python guide](PYTHON_UNIFFI.md) owns package generation, wheel assembly, and installation
verification through `scripts/build-python-uniffi-wheel.py`. The
[package generator](../../crates/merman-uniffi/examples/generate_python_package.rs) reads metadata
from the matching Rust library and packages the separately built production cdylib without
`binding-generation`.

## Apple Swift

The [Apple guide](APPLE_SWIFT.md) owns SwiftPM and XCFramework assembly through
`scripts/build-apple-xcframework.sh`. Its
[Swift generator](../../crates/merman-uniffi/examples/generate_swift_bindings.rs) reads the matching
static library and emits the checked-in Swift source, C header, and module map. Apple CI rebuilds
the XCFramework, compiles its Swift consumer, and rejects projection drift.

## Text Measurement

Merman uses a deterministic, font-agnostic text measurer by default. A UI host can place a
`MermanTextMeasurer` in `MermanEngineServices` when it directly constructs a `MermanEngine` whose
Core Text, Android, or other platform font stack must determine layout. The callback is immutable
for that engine. It receives
the independent text-measurement protocol version `1` and a typed operation. Return `None`/`nil`
for work that is unavailable or cannot be answered synchronously; Merman falls back to its
deterministic fallback for that operation.

Callback-free reusable engines admit concurrent operations. A callback engine serializes operation
admission and returns `Busy` to a competitor; an operation started while the same engine's callback
is active returns `ReentrantCall`. A close attempted from the callback also returns
`ReentrantCall`, retains the callback, and can be retried later. UniFFI's generated trampoline can
report a returned callback error to Rust, but Merman does not catch arbitrary foreign unwinds,
exceptions, or longjmps that bypass that boundary. See
[host text measurement](HOST_TEXT_MEASUREMENT.md) for the operation, result-shape, and lifecycle
contract.
