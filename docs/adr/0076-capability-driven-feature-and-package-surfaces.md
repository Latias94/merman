# ADR 0076: Capability-Driven Feature And Package Surfaces

- Status: accepted
- Date: 2026-07-22
- Descriptor: `capabilities/feature-surface-v1.json`, schema `1`

## Context

Merman's historical feature graph mixes user-visible products, diagram registry profiles,
implementation crate names, host behavior, and negative variants. ABI, Web, Typst, runtime, and
documentation code then repeat overlapping boolean catalogs. Cargo feature unification makes
those names additive compilation choices, but it cannot make them a runtime policy selector.

The architecture needs one stable semantic vocabulary without making Cargo manifests generated
or opaque. It also needs a migration path that does not claim all existing consumers changed in
the same unit.

## Decision

`capabilities/feature-surface-v1.json` exclusively owns public capability IDs, output IDs,
descriptions, target restrictions, implications, named presets, expected runtime capability sets,
and Web/Typst surface mappings. Its schema version is independent from native ABI, diagnostics,
facts, editor token, text-measurement, resource, Typst transport, and package versions.

Cargo manifests remain hand-written declarations. Structured Cargo metadata and compiled runtime
reports will be checked against the descriptor as their owning surfaces migrate; source substring
matching and generated Cargo manifests are not contracts.

Public leaves name observable outputs, APIs, engines, adapters, or compiled tool commands. The
initial vocabulary includes SVG, analysis, editor, ASCII, PNG, JPEG, PDF, Cytoscape and ELK layout,
math, four native system adapters, and the three CLI tool leaves. Presets use the `preset-*`
namespace. The exact native and `preset-web-*` closures, together with the Typst `bridge`, `svg`,
and `publish` mappings, live only in the descriptor and its generated projections.

Runtime-only browser adapters and the Typst transport have semantic runtime IDs but are not Cargo
public leaves. Runtime environment selection and resource profiles remain independent from the
compiled capability set.

The semantic catalog digest is computed from a normalized descriptor with the migration ledger
removed. Native numeric output discriminants and ABI layouts remain owned by the ABI descriptor;
they reference semantic IDs and report their layout digest separately.

## Admission Rules

A new public leaf is accepted only when all of these are present and validated:

- a user-observable API, output, engine, adapter, or compiled tool surface;
- typed absence or removal of that callable surface;
- a material closure boundary;
- at least one supported preset include and exclude;
- observed measured evidence with a reproducible gate.

Negative profiles, one-feature-per-diagram designs, and incidental dependency names are invalid.
Named reusable layout engines are valid because users select their Mermaid behavior directly;
`math` deliberately hides the current RaTeX implementation.

## Staged Migration

U1 provides schema validation, deterministic generation, fixture validation, and an explicit
ledger for the still-live native ABI, Web, and Typst catalogs. It does not verify current manifests
against the target model.

Each U2-U8 surface-local unit must wire in its generated projection and enable a structured gate
that compares the descriptor with the actual compiled capability set, runtime report, or packaged
artifact. Only after that gate passes may the same change mark the affected evidence `observed`,
delete the corresponding legacy catalog, and remove its ledger entry. Until then, the old file is a
transitional consumer input, not a second semantic authority.

Strict mode is intentionally fail-closed during the transition. It rejects any evidence status
other than `observed`, a non-empty ledger, and stable retirement guards for known old catalog paths.
U12 enables strict mode only after all structured surface gates pass and all entries and old live
catalogs are gone.

## Consequences

- Semantic IDs and presets have one reviewable source while Cargo files remain normal TOML.
- Generated Rust, TypeScript, C, and documentation projections are byte-stable and share one
  semantic digest.
- Adding a feature requires API and measured closure evidence, not only an optional dependency.
- Clearing migration bookkeeping cannot hide a live legacy catalog.
- ADR-0006's tiny/full feature decision and ADR-0069's package/preset ownership are superseded.
  ADR-0066 retains its safe FFI crate boundary but no longer owns capability/output semantic IDs.
  ADR-0074 retains realm, runtime, lifecycle, benchmark, cache, and application ownership; only
  its package-surface selection is projected from this descriptor.
