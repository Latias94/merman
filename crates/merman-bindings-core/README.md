# merman-bindings-core

Safe shared binding facade for Merman native bindings.

`merman-bindings-core` is an implementation crate used by the C ABI and UniFFI binding crates. It keeps the JSON options contract, error mapping, and feature-gated render entry points in one place so platform bindings expose the same behavior. It also owns metadata discovery for Mermaid core themes separately from host/editor theme presets.

Most applications should use one of the public packages instead:

- Rust: [`merman`](https://crates.io/crates/merman)
- C ABI and native hosts: [`merman-ffi`](https://crates.io/crates/merman-ffi)
- Python and Apple Swift binding generation: [`merman-uniffi`](https://crates.io/crates/merman-uniffi)

## Features

This crate has no default features by design. Select `all-diagrams` or required `diagram-*`
families independently from outputs. Existing distributed recipes retain all families; custom
source artifacts may narrow them. Parser metadata does not imply a local SVG or ASCII handler.
Binding artifacts select direct capabilities. The Rust facade's complete SVG aggregates
remain source conveniences, not binding features; math is an explicit positive leaf. In particular,
`layout-elk` is an explicit EPL-2.0 boundary and `math` brings the separately noticed RaTeX/font
closure. Feature names describe compile-time inputs; the generated runtime catalog, exact artifact
recipe, and bundled notices describe the resulting artifact.

- `svg` enables SVG rendering through the main Merman facade.
- `native-fonts` enables the Rust-only, constructor-owned native font context. It does not
  enable ambient system-font discovery or add font paths to binding JSON.
- `analysis` enables diagnostics analysis, validation JSON, document facts, and lint rule catalog helpers.
- `ascii` enables ASCII/Unicode text rendering.
- `layout-cytoscape` and `layout-elk` enable their named SVG layout engines.
- `math` enables the RaTeX math label backend.
- `native-runtime` enables the complete native clock, time-zone, and random adapter set for
  transport-owned runtime selection. It is atomic on binding crates; partial native-runtime
  combinations are not exposed there.

The ABI 3 operation catalog exposes real `png`, `jpeg`, and `pdf` byte outputs when their corresponding capabilities are compiled. A build that omits one returns the same structured missing-capability error as every other unavailable operation; it does not advertise a phantom backend.

## Runtime Policy

`BindingEngine::new()` is always deterministic. Transport bindings use `BindingEngine::from_options()`, where an omitted `runtime_policy` also selects the deterministic clock, UTC time zone, and fixed random seed even when `native-runtime` is compiled. Native host state is opt-in:

```json
{ "runtime_policy": "native" }
```

The native policy is available only when the binding artifact is built with `native-runtime`, which
compiles the `system-clock`, `system-timezone`, and `system-random` adapters as one transport-owned
unit. A missing adapter is reported with the `unsupported-operation` status and
`missing-capability` kind. Runtime catalogs and operation metadata continue to expose the concrete
adapter IDs (`system-clock`, `system-timezone`, and `system-random`) so hosts can attest the
environment that produced an output. Custom Rust operation contexts remain constructor-owned and
cannot be combined with the JSON selector.

## Requests, Results, and Host Services

Generic requests use constructors and fluent setters so future request fields remain additive:

```rust
use merman_bindings_core::BindingOperationRequest;

let request = BindingOperationRequest::new("document-analysis-json", source)
    .with_uri(b"file:///diagram.mmd")
    .with_options_json(options_json);
```

Operation results expose typed schema-1 metadata and retain the exact original metadata JSON. Known raster and PDF plans are typed, while future output-plan kinds decode as an open `Unknown` variant without discarding their JSON. `operation_metadata_contract()` and `binding_operation_expectations()` are the stable generator inputs for language projections and the descriptor-derived 14-operation test matrix. `xtask gen-binding-contract` materializes both the Node projection and the language-neutral fixture under `fixtures/bindings/generated/`; transports consume those generated values instead of maintaining parallel operation or metadata vocabularies.

When `ascii` is enabled, the ASCII output plan is adapted from `merman-ascii::AsciiOutput::metadata()`.
The CLI's report JSON is the same metadata payload plus the exact text projection from
`AsciiOutput::report()`. ASCII schema version 2, the required `encoding` field, field names, enum
strings, nullable fallback fields, and unknown-field behavior therefore remain one coordinated
contract across Rust, CLI, generated fixtures, Web metadata, and platform DTOs.

Reusable host services are immutable and constructor-owned. A transport wraps its callback and admission policy, then creates the engine through the single service-aware constructor:

```rust
use merman_bindings_core::{BindingEngine, BindingEngineServices};

let services = BindingEngineServices::new().with_host_text_measurer(measurer);
let engine = BindingEngine::from_options_and_services(options_json, services)?;
# Ok::<(), merman_bindings_core::BindingError>(())
```

This example requires the `svg` feature. An explicit `environment.text_measurement` selector, including `null`, conflicts with the constructor service in both base options and request overlays. Construction only installs the callback; it never invokes it. Foreign callback lifetime, close admission, and out-of-lock destruction remain transport responsibilities.

Rust hosts built with `native-fonts` can install an already loaded, explicitly authorized font
context instead. The same operation-local context then measures labels, creates SVG text
fallbacks, and paints enabled PNG, JPEG, or PDF targets through the main Merman facade:

```rust
use merman::svg::export::NativeFontContext;
use merman_bindings_core::{BindingEngine, BindingEngineServices};

fn engine(fonts: NativeFontContext) -> Result<BindingEngine, merman_bindings_core::BindingError> {
    let services = BindingEngineServices::new().with_native_font_context(fonts);
    BindingEngine::from_options_and_services(
        br#"{"svg":{"pipeline":"resvg-safe"}}"#,
        services,
    )
}
```

Load font sources and choose their limits before constructing these services. Each request shares
the admitted font data but creates fresh measurement caches, failure state, and operation control;
the reusable engine never retains a previous request's cancellation or resource outcome. A native
font context conflicts with `with_host_text_measurer` in either setter order and with an explicit
`environment.text_measurement` selector, including `null`, in base options or overlays. SVG fonts
are not embedded, so an external SVG viewer still needs the selected font. The `native-font-context`
service is available through Rust and the C ABI's separate `engine_new_with_fonts`
constructor when the artifact enables `native-fonts`. UniFFI, Android, Node, Web,
and Typst do not acquire a native font context through this API.

## SVG Output Contract

Successful native output can carry a `rejected` target-admission status in
`theme_execution_evidence`: this classifies strict portability, not operation success.
Default best-effort rendering still returns bytes. Reasons such as
`native_directional_filter_transform_residual`, `fallback_css_residual`, and
`pdf_implicit_viewport_clipping_residual` identify known fidelity limits. An explicit
portable-output requirement rejects them. Invalid geometry, resource exhaustion and
cancellation retain their operation errors.

`render_svg` and the cached engine `render_svg` entry point return SVG bytes. With empty options, that SVG is the Mermaid-parity contract. Hosts that pass SVG to strict SVG renderers or rasterizers should request the export contract with:

```json
{ "svg": { "pipeline": "resvg-safe" } }
```

Editor previews can select one experimental compiled diagram theme through top-level `theme`, using
exactly one `preset` or complete typed `spec`. The binding compiler maps that value to the same
`DiagramThemeSpec` / `ThemePreset` model used by Rust and installs the resulting immutable
`DiagramTheme` on the renderer. Raw Mermaid overrides remain top-level `site_config`, while layout,
environment, and SVG postprocessing choices retain their independent owners. The removed
`presentation` and `host_theme` groups are rejected rather than treated as compatibility aliases.
Theme compilation reports requirements; it does not by itself prove the unfinished cross-family,
cross-output portability matrix.

Hosts can enable `drop_native_duplicate_fallbacks` for generated labels tied to native SVG switch
fallbacks. Independent labels with equal text are preserved.

Hosts that inline SVG in a browser and want fallback text while retaining the original `<foreignObject>` nodes can use `"readable"` instead. PNG, JPEG, and PDF are available through the same generic binding operation route when their output capability is compiled; their format-specific resource limits remain part of the selected resource policy.

## Capability Metadata

`runtime_catalog()` reports one artifact's supported options and binding-payload schemas, named metadata IDs, sorted capability/output/operation/system-adapter IDs, text-measurement providers, registry facts, and resource-to-operation mappings. Consumers validate its flat schema and local relations while tolerating newly introduced stable IDs. Artifact owners must construct that catalog from callable endpoints rather than package names or transport-local booleans. `diagram_family_capabilities()` exposes the one complete pinned Mermaid language catalog: detector, semantic/editor parser, typed render, authoring-header, and config-namespace facts. This catalog is not selected by a build profile. The `analysis` capability is independent from `svg` and `ascii`; slim artifacts can expose ASCII or render output without compiling diagnostics and lint catalog support.

Diagnostics payloads and rich parser-only document facts are independent contracts: diagnostics use schema v1 and facts use schema v2. Other facts versions are rejected at the boundary before the body is decoded. Current facts writers always emit `rename_policy`; schema v2 contains generic semantic facts and no Flowchart-only rich graph. This does not revive the removed TextScan-capable decoder, executor, or parallel binding path. Transport and platform ABI versions are independent from these JSON schema versions.

With `svg` enabled, this crate centralizes the host text-measurement result-shape contract. The current transports expose 22 exact operations with contiguous codes `0..21`; operation 18 is `raw-bbox-height`, and operations 19 through 21 are signed direct-text bbox y offsets for ordinary, middle, and hanging baselines. The text-measurement protocol remains independently versioned at 1, while the native C ABI, UniFFI binding API, and browser WASM transport each report their own API version through the runtime catalog.

For product scope, diagram coverage, and compatibility policy, see the [project README](https://github.com/Latias94/merman#readme) and [alignment status](https://github.com/Latias94/merman/blob/main/docs/alignment/STATUS.md).

## License And Notices

Merman is available under MIT or Apache-2.0. Binding consumers must also review the selected
feature closure: ELK is EPL-2.0 and the embedded RaTeX fonts are OFL-1.1. The artifact-specific
recipe and [`THIRD_PARTY_NOTICES.md`](https://github.com/Latias94/merman/blob/main/THIRD_PARTY_NOTICES.md)
are the distribution evidence for those components.
