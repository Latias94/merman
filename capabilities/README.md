# Capability And Artifact Surfaces

Merman has two repository-wide machine authorities in this directory:

- `feature-surface-v1.json` owns public capability and output IDs, operations, implications, and target legality. It is not a product-preset or release-profile catalog.
- `artifact-profiles-v2.json` owns exact Cargo build recipes for capability-bearing artifacts. A recipe names the package, target, profile, `default-features` choice, explicit features, build target, and expected capability/output set plus exact compiled logical parser families.

Cargo manifests remain hand-written compilation declarations. C ABI, UniFFI, LSP, Web exports, and the Typst transport retain their own interface authorities. This directory does not copy those wire contracts or package release metadata into a generic transport catalog.

## Generated Projections

Run:

```text
cargo run -p xtask -- gen-capability-surface
cargo run -p xtask -- verify-capability-surface
cargo run -p xtask -- verify-artifact-profiles
```

The generator writes byte-stable Rust, TypeScript, C, and Markdown projections under `capabilities/generated/`. Verification checks schema, implication closure, target legality, generated-file freshness, Cargo package/target existence, Cargo feature names, and exact profile-to-capability mappings.

Fixture validation can use an alternate descriptor without generating files:

```text
cargo run -p xtask -- verify-capability-surface --descriptor path/to/fixture.json
cargo run -p xtask -- verify-artifact-profiles --descriptor path/to/fixture.json
```

The semantic SHA-256 digest covers the complete capability descriptor. Project plans, migration units, release status, and documentation paths deliberately do not live in either machine contract.

The capability descriptor schema version 1 is still a pre-stable contract. Its object shape may evolve incompatibly during
the 0.x release line, including new required fields, only when the semantic digest and every strict
projection or verifier change in lockstep. Consumers must use the digest—not the schema number
alone—as the exact compatibility identity. A schema-version bump is reserved for a representation
whose meaning is no longer the same v1 model.

Artifact profile schema 2 requires `expected.diagram_families`: a sorted, duplicate-free list of
compiled logical parser family IDs, excluding Error infrastructure. Language-bearing product
recipes explicitly select `all-diagrams`; export-only recipes have an empty parser set. Consumers
must migrate the descriptor path and required field together. Exact parser availability is probed
in the built target through its family catalog, not inferred from the verifier's own feature set.
This field does not imply local renderer, ASCII, or editor support. `feature-surface-v1.json` and
its runtime capability namespace are unchanged. See
[ADR-0091](../docs/adr/0091-selectable-diagram-families.md).

## Contract Boundaries

An artifact profile says which capabilities are requested for one concrete product build. Cargo features are additive, so a feature list cannot prove that an omitted feature or dependency is absent. An artifact profile can make that claim only when it records `default_features: false` and the corresponding build or dependency-closure probe passes. There is no hand-maintained `observed` status: executable evidence is the successful probe.

Build targets and closure-evidence targets are separate. A `host` recipe builds for the machine running the recipe, while its normal-dependency probe is explicitly scoped to the `x86_64-unknown-linux-gnu` reference target and excludes build and proc-macro edges. Cargo still resolves the normal dependencies of a proc-macro root for the running host, so complete host-profile package/version evidence is authoritative only when the verifier itself runs on that Linux reference host. Other hosts still enforce the readable required-package, forbidden-package, and forbidden-feature claims. That reference is not a full clean-build cost metric, a macOS/Windows closure union, or a cross-platform absence claim. A `target-set` recipe probes every descriptor-owned target and applies the same semantic claims without freezing the complete closure behind an opaque digest.

RustSec exception coverage follows the same authority boundary. Every verifier host checks `target-set` profile membership, while host-profile membership participates only when the verifier runs on that profile's closure reference host. The Linux reference gates therefore check the complete ledger, and other hosts cannot turn their host-local dependency graph into Linux evidence.

Artifact profiles describe compiled components, not every package that redistributes one. Wheels, AARs, XCFrameworks, npm packages, and other release bundles keep their package manifests and release checks at the owning surface. A bundle may compose one or more artifact profiles without inventing another Cargo root or copying an ABI definition.

## CLI Font Policy

Default and release CLI binaries compile `native-fonts` and `system-fonts` so native font
measurement is available. Mermaid SVG defaults to `deterministic` measurement, with
`default_font_source: none` and `default_system_discovery: disabled`, scoped by
`default_policy_scope: "mermaid-svg-input"`. Mermaid PNG/JPEG/PDF default to native measurement
with bounded system discovery before layout, sharing one font context between measurement and
painting. `encoded_output_defaults` reports that separate policy. These encoded-output defaults
already shipped in `0.9.3`; they are not a new development-line capability.

Providing `--font-file` or `--font-dir` selects native measurement automatically. Explicit font
inputs form a closed set unless `--font-system on` is supplied. Without explicit font inputs,
`--font-measurement native` permits bounded system discovery; `--font-system on` also explicitly
opts into system fonts. Use `--font-measurement deterministic` without explicit font sources to
retain the previous encoded-output estimates and CSS path. Encoded painting may still acquire
system fonts through its existing export environment. The experimental system-font query backend
remains outside the default and release feature recipes; ordinary releases use bounded discovery.
These defaults apply to Mermaid input in `render`, `batch`, and `mmdc`; raw SVG conversion paints
existing geometry without relayout. CLI capabilities schema 2, CLI contract 6, and text-measurement
protocol 1 retain their versions; the current measurement contract has 23 operations. See the
[CLI font guide](../crates/merman-cli/README.md#explicit-fonts).

## Main-Line Native Products

The current main-line Android, Apple, Python, and Flutter artifact profiles select all families
and one shared native capability set through these Cargo features:

```text
all-diagrams,analysis,ascii,fixes,layout-cytoscape,layout-elk,math,svg
```

Those products omit `png`, `jpeg`, `pdf`, and `native-runtime`; selecting math does not add
native font services or system discovery. Their generated
language APIs retain the complete operation vocabulary; the loaded runtime catalog reports the
actual subset, and an unavailable valid operation returns a typed `missing-capability` error.
Constrained policy rejects opaque math rendering even when math is compiled. Select an appropriate
interactive/trusted policy for admitted formula workloads and retain host execution and memory limits.

These expanded native products are unreleased. Published Android, Python, and Flutter `0.9.2`
and legacy Apple `0.9.3` retain their math-free recipes. The
[prebuilt product guide](../docs/release/PREBUILT_PRODUCTS.md) records installed-version and host
boundaries. [ADR-0100](../docs/adr/0100-prebuilt-product-entry-points.md) and
[ADR-0101](../docs/adr/0101-binding-product-delivery.md) amend the original
[ADR-0079](../docs/adr/0079-default-native-prebuilt-capability-sku.md) selection.

Native C/C++ consumers should start with `sdk-native-full`: SVG, PNG, JPEG, PDF, ASCII, math,
both layout backends, and caller-owned font bytes. Native C and WASI SDKs already have a published
`0.9.3` release; main's contracts must still be paired with main's headers and libraries. The
`c-abi-native` profile remains a complete host reference build for ABI verification and custom
source embedding, rather than the downloadable default SDK. See the
[SDK guide](../distribution/sdk/README.md) for full/reduced recipes and supported targets.

Other cross-language artifacts are interface-shaped rather than forced into that native SKU.
The current Typst recipe selects SVG, canonical analysis, theme authoring, and both layout backends.
The experimental Node native and Node-WASM recipes select SVG with math and both layout backends,
without analysis, ASCII, or binary export; published Node `0.9.2` remains math-free. Browser WASM
keeps package-specific full and slim profiles because npm package identity is its capability-selection
mechanism. Web full/render include math, as published `0.9.2` already does; npm users do not select
Cargo features.

Rust source defaults and the `complete-svg` / `complete-svg-elk` convenience aggregates remain
math-free on main. Custom source consumers select `math` explicitly and choose diagram families
separately. CLI source defaults and its `rustdoc` leaf omit math, while the official `cli-release`
recipe selects it explicitly. Exact product recipes do not change those source defaults.

The verifier does not parse README prose, plan text, or private symbol names. User documentation is reviewed and example-tested where useful, but prose is not a release authority. Generated reference tables may have freshness checks because their source is structured machine data.

## Theme Typography

Themes retain font-family names, sizes, weights, spacing, canvas layers and effects. Font bytes
and native theme shaping are outside the current product. No artifact or Cargo feature enables
embedded theme font processing; resource-bearing recipes fail explicitly instead of silently
substituting system fonts. Native PNG/PDF backends retain their existing system-font handling.

The former implementation is preserved on the local `preserve/embedded-fonts-theme` branch at
`dfe54f279`. Its portable-font qualifications are historical and do not describe current artifacts.

## Admitting A Public Leaf

A public leaf uses a positive kebab-case name for an observable API, output, selectable engine, environment adapter, or compiled tool command. It must have:

- a callable public behavior and typed missing-capability result when omitted;
- a material dependency, target, license, security, resource, build-time, or artifact-size boundary;
- at least one applicable leaf build or exact artifact profile that exercises it and one valid build/profile that omits it;
- an executable API, artifact, dependency, or target probe owned by the affected surface.

Negative and incidental dependency-named public features are rejected. Positive `diagram-*`
selectors and their `all-diagrams` union occupy a separate Cargo selector namespace owned by
`merman_core::diagram_family_selectors()`. They are not runtime capability IDs. Layout and math names describe selectable Mermaid behavior rather than their current implementation crates. Runtime environment selection and resource policy remain separate contracts: compiling an adapter does not select native or deterministic behavior for an operation.
