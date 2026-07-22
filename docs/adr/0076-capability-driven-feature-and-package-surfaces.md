# ADR 0076: Capability-Driven Feature And Package Surfaces

- Status: accepted
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
descriptions, target restrictions, implications, named presets, expected runtime capability sets,
and their additive semantic closure.

`capabilities/artifact-profiles-v1.json` owns exact Cargo recipes for capability-bearing compiled
artifacts. Each recipe identifies a real Cargo package and target, profile, `default-features`
choice, explicit feature set, build target, and expected capability/output report. It contains no
release-state field, documentation path, evidence prose, package bundle, or wire-layout copy.

Cargo manifests remain hand-written. Each protocol keeps its natural authority: the native ABI
descriptor and header own C layouts and symbols; UniFFI definitions own generated language
bindings; LSP owns its protocol surface; Web package exports own browser entry points; and the
Typst descriptor owns its wasm-minimal-protocol boundary. Package manifests and release checks own
distribution composition. No repository-wide transport catalog duplicates these authorities.

Public leaves name observable outputs, APIs, engines, adapters, or compiled tool commands. Presets
use the `preset-*` namespace and are additive inclusion bundles only. They never assert that an
omitted capability or dependency is absent. Runtime environment selection and resource profiles
remain independent from the compiled capability set.

## Verification Boundary

Machine checks cover facts that can be derived without reading prose:

- descriptor schema, IDs, implication closure, preset closure, and target legality;
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
- at least one supported preset inclusion and omission;
- an executable gate owned by the affected API, artifact, dependency, or target.

Negative profiles, one-feature-per-diagram designs, and incidental dependency names are invalid.
Named reusable layout engines are valid because users select their Mermaid behavior directly;
`math` deliberately hides the current RaTeX implementation.

## Consequences

- Semantic IDs and presets have one reviewable source while Cargo files remain normal TOML.
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
