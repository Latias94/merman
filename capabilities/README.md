# Capability And Artifact Surfaces

Merman has two repository-wide machine authorities in this directory:

- `feature-surface-v1.json` owns public capability and output IDs, operations, implications, and
  target legality. It is not a product-preset or release-profile catalog.
- `artifact-profiles-v1.json` owns exact Cargo build recipes for capability-bearing artifacts. A
  recipe names the package, target, profile, `default-features` choice, explicit features, build
  target, and expected capability/output set.

Cargo manifests remain hand-written compilation declarations. C ABI, UniFFI, LSP, Web exports,
and the Typst transport retain their own interface authorities. This directory does not copy those
wire contracts or package release metadata into a generic transport catalog.

## Generated Projections

Run:

```text
cargo run -p xtask -- gen-capability-surface
cargo run -p xtask -- verify-capability-surface
cargo run -p xtask -- verify-artifact-profiles
```

The generator writes byte-stable Rust, TypeScript, C, and Markdown projections under
`capabilities/generated/`. Verification checks schema, implication closure, target legality,
generated-file freshness, Cargo package/target existence, Cargo feature names, and exact
profile-to-capability mappings.

Fixture validation can use an alternate descriptor without generating files:

```text
cargo run -p xtask -- verify-capability-surface --descriptor path/to/fixture.json
cargo run -p xtask -- verify-artifact-profiles --descriptor path/to/fixture.json
```

The semantic SHA-256 digest covers the complete capability descriptor. Project plans, migration
units, release status, and documentation paths deliberately do not live in either machine contract.

## Contract Boundaries

An artifact profile says which capabilities are requested for one concrete product build. Cargo
features are additive, so a feature list cannot prove that an omitted feature or dependency is
absent. An artifact profile can make that claim only when it records `default_features: false` and
the corresponding build or dependency-closure probe passes. There is no hand-maintained `observed`
status: executable evidence is the successful probe.

Artifact profiles describe compiled components, not every package that redistributes one. Wheels,
AARs, XCFrameworks, npm packages, and other release bundles keep their package manifests and
release checks at the owning surface. A bundle may compose one or more artifact profiles without
inventing another Cargo root or copying an ABI definition.

The verifier does not parse README prose, plan text, or private symbol names. User documentation is
reviewed and example-tested where useful, but prose is not a release authority. Generated reference
tables may have freshness checks because their source is structured machine data.

## Admitting A Public Leaf

A public leaf uses a positive kebab-case name for an observable API, output, selectable engine,
environment adapter, or compiled tool command. It must have:

- a callable public behavior and typed missing-capability result when omitted;
- a material dependency, target, license, security, resource, build-time, or artifact-size boundary;
- at least one applicable leaf build or exact artifact profile that exercises it and one valid
  build/profile that omits it;
- an executable API, artifact, dependency, or target probe owned by the affected surface.

Diagram-specific, negative, and incidental dependency-named public features are rejected. Layout
and math names describe selectable Mermaid behavior rather than their current implementation
crates. Runtime environment selection and resource policy remain separate contracts: compiling an
adapter does not select native or deterministic behavior for an operation.
