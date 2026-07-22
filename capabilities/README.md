# Capability Surface

`feature-surface-v1.json` is the only authority for public capability IDs, output IDs,
implications, presets, expected runtime reports, target restrictions, and Web/Typst surface
mappings. Cargo manifests remain hand-written compilation declarations. They are not generated
from this file.

The descriptor establishes the target architecture before every consumer has migrated. It does
not claim that current Cargo manifests, runtime reports, native bindings, Web packages, or Typst
artifacts already match the target model.

## Generated Projections

Run:

```text
cargo run -p xtask -- gen-capability-surface
cargo run -p xtask -- verify-capability-surface
```

The generator writes byte-stable Rust, TypeScript, C, and Markdown projections under
`capabilities/generated/`. These files are descriptor-owned staging artifacts. A projection does
not become a live consumer contract until its owning migration unit wires it into that surface and
removes the superseded catalog.

`verify-generated` includes the non-strict capability-surface freshness check. Fixture validation
can use an alternate descriptor without generating files:

```text
cargo run -p xtask -- verify-capability-surface --descriptor path/to/fixture.json
```

The semantic SHA-256 digest excludes the migration ledger. Clearing transitional debt therefore
does not masquerade as a capability-catalog change.

## Migration Ledger

The descriptor's `migration_ledger` implements a three-stage transition:

1. U1 validates the canonical schema and generated projections while explicitly recording every
   legacy live ABI, Web, and Typst catalog.
2. Each U2-U8 surface migration consumes the canonical projection and enables a structured,
   surface-local gate that compares the descriptor with the actual compiled capability set,
   runtime report, or packaged artifact. Only after that gate passes may the same change mark the
   affected evidence `observed`, delete the legacy catalog, and remove its ledger entry.
3. U12 enables `verify-capability-surface --strict`. Strict mode rejects every non-`observed`
   evidence record, any remaining ledger entry, and known legacy catalog paths, so changing only
   bookkeeping cannot produce a false pass.

The legacy paths are retirement guards, not alternate capability authorities.

## Admitting A Public Leaf

A public leaf must use a positive kebab-case name for an observable API, output, selectable
engine, environment adapter, or compiled tool command. Its descriptor record must define:

- a typed missing-capability contract or an explicitly removed callable surface;
- a material dependency, target, license, security, resource, build-time, or artifact-size
  boundary;
- at least one applicable preset that includes it and one that explicitly excludes it;
- measured evidence and the gate that reproduces that evidence.

New leaves require `observed` evidence. The plan-mandated U1 leaves are temporarily marked
`migration-required` to state explicitly that their consumer surfaces have not yet been measured.
U2-U8 may replace that status only after the owning structured surface gate passes, and must update
the evidence, live consumer, legacy catalog, and matching ledger entry atomically. Diagram-specific,
negative, and incidental dependency-named public features are rejected. Layout and math names
describe selectable Mermaid behavior rather than their current implementation crates.

Runtime environment selection and resource policy remain separate contracts. A compiled adapter
does not select native or deterministic behavior by itself.
