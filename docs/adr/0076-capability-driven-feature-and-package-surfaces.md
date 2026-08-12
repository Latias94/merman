# ADR 0076: Capability-Driven Feature And Package Surfaces

- Status: accepted; native prebuilt SKU policy superseded by ADR-0079
- Date: 2026-07-22
- Descriptor: `capabilities/feature-surface-v1.json`, schema `1`
- Artifact profiles: `capabilities/artifact-profiles-v1.json`, schema `1`

## Context

Merman's historical feature graph mixes user-visible products, diagram registry profiles,
implementation crate names, host behavior, and negative variants. ABI, Web, Typst, runtime, and
documentation code then repeat overlapping boolean catalogs. Cargo feature unification makes
features additive compilation choices, but it cannot make them a runtime policy selector or prove
that an omitted dependency is absent.

The architecture needs one stable semantic vocabulary and reproducible build recipes without
generating Cargo manifests or centralizing every protocol and package concern in one descriptor.

## Decision

`capabilities/feature-surface-v1.json` exclusively owns public capability IDs, output IDs,
descriptions, target restrictions, operations, and implications. It is a semantic vocabulary, not
a product-preset or release-profile catalog.

`capabilities/artifact-profiles-v1.json` owns exact Cargo recipes for capability-bearing compiled
artifacts. Each recipe identifies a real Cargo package and target, profile, `default-features`
choice, explicit feature set, build target, and expected capability/output report. It contains no
release-state field, documentation path, evidence prose, package bundle, or wire-layout copy.

Closure evidence remains owned by its verifier rather than the artifact descriptor. A `host` recipe
builds on the executing host, but its normal-dependency probe uses
`x86_64-unknown-linux-gnu` as the reference target and excludes build and proc-macro edges. The
verifier enforces readable required-package, forbidden-package, forbidden-feature, and declared
product-boundary claims rather than an opaque digest of the complete closure or a requirement that
incidental transitive packages remain present. Exact versions remain owned by `Cargo.lock`, while
legal reports, advisory policy, and artifact measurements retain their own evidence. A `target-set`
recipe must prove each descriptor-owned target separately.

Cargo manifests remain hand-written. Each protocol keeps its natural authority: the native ABI
descriptor and header own C layouts and symbols; UniFFI definitions own generated language
bindings; LSP owns its protocol surface; Web package exports own browser entry points; and the
Typst descriptor owns its wasm-minimal-protocol boundary. Package manifests and release checks own
distribution composition. No repository-wide transport catalog duplicates these authorities.
Capability/API build roots set `package.metadata.merman.artifact-profile-required = true`; the
artifact-profile verifier discovers that owner-local coverage requirement through Cargo metadata.

Public leaves name observable outputs, APIs, engines, adapters, or compiled tool commands. The
repository deliberately has no global `preset-*` Cargo feature lattice: additive Cargo features
cannot express exclusions, and a cross-product of product, transport, runtime, and release
profiles would make the public API misleading. The user-facing `merman` facade and
`merman-rustdoc` integration crate expose the same result-named `complete-svg` aggregate (`svg`,
both layout engines, and `math`); the Rustdoc default mirrors the facade so its accepted examples
render without extra feature study. Other products and artifact profiles select direct positive
leaves owned by their package. `complete-svg` is a convenience compile aggregate, not an absence
or runtime-policy contract. Runtime environment selection and resource profiles remain independent
from the compiled capability set.

The original decision kept one complete native SKU per published language surface. ADR-0079
supersedes that product choice with a shared default prebuilt capability set while retaining this
ADR's feature vocabulary, artifact-profile authority, and source-build rules. The C ABI remains a
source-only crate whose complete native artifact profile produces host reference libraries for
verification rather than a prebuilt release bundle.

Evidence for a second native SKU belongs to the proposal that introduces it. The proposal must set
its threshold before measuring same-revision, same-target final artifacts and must use existing
surface build and smoke entry points. Native size or memory measurements become standing CI or
release gates only after maintainers accept the SKU and a stable budget.

## Verification Boundary

Machine checks cover facts that can be derived without reading prose:

- descriptor schema, IDs, implication closure, and target legality;
- generated Rust, TypeScript, C, and Markdown projection freshness;
- Cargo package, target, profile, feature, crate-type, and target-triple existence;
- exact artifact-profile capability/output mappings;
- surface-owned ABI layouts, exports, runtime probes, package contents, dependency closures, and
  target builds.

A successful executable probe is evidence. There is no manually promoted
`migration-required`/`observed` artifact state. README wording, plan identifiers, private function
names, and source substrings are not machine evidence and are not release gates. User examples may
be compiled or executed because their behavior is machine-testable; ordinary prose remains a
review concern.

## Admission Rules

A new public leaf is accepted only when all of these are present:

- a user-observable API, output, engine, adapter, or compiled tool surface;
- typed absence or removal of that callable surface;
- a material closure boundary;
- at least one supported leaf build or exact artifact profile that exercises the capability and one
  valid build/profile that omits it;
- an executable gate owned by the affected API, artifact, dependency, or target.

Negative profiles, one-feature-per-diagram designs, and incidental dependency names are invalid.
Named reusable layout engines are valid because users select their Mermaid behavior directly;
`math` deliberately hides the current RaTeX implementation.

## Consequences

- Semantic IDs and implications have one reviewable source while Cargo files remain normal TOML.
- Exact build absence is proved by an explicit `default-features = false` recipe plus a build or
  closure probe, never inferred from a preset or a status field.
- Bundles such as VSIX, wheels, AARs, and npm packages compose compiled artifacts without inventing
  fake Cargo roots or duplicate wire contracts.
- Generated projections are byte-stable and share one semantic digest.
- Adding a feature requires a callable API and executable closure evidence, not only an optional
  dependency or a documentation claim.
- ADR-0006's tiny/full feature decision and ADR-0069's package/preset ownership are superseded.
  ADR-0066 retains its safe FFI crate boundary but no longer owns capability/output semantic IDs.
  ADR-0074 retains realm, runtime, lifecycle, benchmark, cache, and application ownership; only
  its package-surface selection is projected from the capability descriptor.
