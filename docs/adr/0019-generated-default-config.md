# ADR-0019: Generated Default Config

## Status

Accepted

## Context

Mermaid's parsing, detection, and layout-ready outputs depend on a large set of configuration defaults.
Hand-maintaining these defaults in Rust is error-prone and tends to drift from the pinned upstream tag.

At the same time, `merman-core` should not depend on executing Node/Vite tooling at runtime, and the
defaults should be stable across environments and CI.

## Decision

- Treat the pinned upstream schema (`repo-ref/mermaid/.../schemas/config.schema.yaml`) as the source of truth
  for configuration defaults.
- Add an `xtask` command to generate a JSON defaults artifact from the schema:
  - `cargo run -p xtask -- gen-default-config`
  - Output: `crates/merman-core/src/generated/default_config.json`
- Add a verification command for CI/local checks:
  - `cargo run -p xtask -- verify-generated`
- Commit the generated artifact to the repository and load it via `include_str!()` inside `merman-core`.
- Keep a small explicit override layer only when Mermaid defaults are known to differ from schema defaults
  (or when upstream introduces non-schema defaults), backed by parity tests.

## Consequences

- Default behavior is more likely to stay aligned with Mermaid across diagrams.
- Diffs in default behavior become reviewable as changes to a single generated artifact.
- The generator is intentionally simple and may not perfectly model all JSON-schema features; when a mismatch
  is discovered, we either improve the generator or add a small override layer with a regression test.
