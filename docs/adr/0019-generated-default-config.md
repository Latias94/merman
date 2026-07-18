# ADR-0019: Generated Default Config

## Status

Accepted

## Context

Mermaid's parsing, detection, and layout-ready outputs depend on a large configuration object.
That object is not just the JSON Schema defaults: `defaultConfig.ts` replaces and extends selected
objects, adds functions and explicit `undefined` values, and `config.ts` clones the result through
`assignWithDepth`. Hand-maintaining the result in Rust is error-prone and drifts from the pinned
upstream tag.

At the same time, `merman-core` should not depend on executing Node/Vite tooling at runtime, and the
defaults should be stable across environments and CI.

## Decision

- Model the pinned Mermaid configuration as three independent planes:
  1. `default_config.json` is the pure JSON value projection of Mermaid's cloned runtime config.
     It contains the upstream six-key `secure` array and excludes functions, `undefined`, schema
     metadata, and separately generated theme variables.
  2. `default_config_shape.json` contains Mermaid's flat `configKeys` set plus the paths contributed
     by functions and explicit `undefined` values. Directive sanitization consumes this shape, so a
     legal key is not mistaken for a missing value.
  3. Merman's hardened ten-key `secure` policy is typed Rust policy. `default_site_config()` applies
     it after loading the pure upstream artifact; it is not written into either upstream artifact.
- Generate both artifacts directly from the installed Mermaid 11.16 runtime in
  `crates/xtask/src/cmd/default_config.rs`:
  - `cargo run -p xtask -- gen-default-config`
- Validate the declared package version, installed package version, and installed package-tree hash
  before generation. The runtime projection is the single generation authority; do not replay
  `defaultConfig.ts` in Rust and do not provide a general-purpose set/remove override mechanism.
- Verify committed artifacts with:
  - `cargo run -p xtask -- verify-default-config`
  - `cargo run -p xtask -- verify-generated`
- Commit both artifacts and load them with `include_str!()` in `merman-core`.

## Consequences

- Upstream values, legal directive keys, and local security policy can change independently without
  being conflated in one JSON file.
- Function and `undefined` keys remain legal to the sanitizer even though they have no JSON value.
- Runtime drift fails the normal generated-artifact gate instead of being compared against a second
  hand-maintained projection.
- Artifact generation and verification require the pinned Node package installation. Building and
  running `merman-core` only reads committed JSON and does not require Node.
