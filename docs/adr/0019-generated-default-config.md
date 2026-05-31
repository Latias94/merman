# ADR-0019: Generated Default Config

## Status

Accepted

## Context

Mermaid's parsing, detection, and layout-ready outputs depend on a large set of configuration defaults.
Hand-maintaining these defaults in Rust is error-prone and tends to drift from the pinned upstream tag.

At the same time, `merman-core` should not depend on executing Node/Vite tooling at runtime, and the
defaults should be stable across environments and CI.

## Decision

- Treat the pinned upstream schema (`repo-ref/mermaid/.../schemas/config.schema.yaml`) plus
  Mermaid's `src/defaultConfig.ts` overlay behavior as the source of truth for configuration
  defaults.
- Add an `xtask` command to generate a JSON defaults artifact from the schema:
  - `cargo run -p xtask -- gen-default-config`
  - Output: `crates/merman-core/src/generated/default_config.json`
- Apply the reviewed local override manifest after schema extraction:
  - `crates/xtask/default_config_overrides.json`
  - The manifest records JSON replacements for upstream non-JSON defaults, empirically required
    parity overrides, and explicitly deferred/out-of-scope 11.15 config families.
- Add verification commands for CI/local checks:
  - default config only: `cargo run -p xtask -- verify-default-config`
  - umbrella generated-artifact check: `cargo run -p xtask -- verify-generated`
- Commit the generated artifact to the repository and load it via `include_str!()` inside `merman-core`.
- Keep the explicit override manifest small and reviewable. Use it only when Mermaid defaults are
  known to differ from generated schema defaults, when upstream introduces non-JSON defaults, or
  when local support scope intentionally excludes a newly introduced config family/key. Back
  behavior-affecting overrides with parity tests.

## Consequences

- Default behavior is more likely to stay aligned with Mermaid across diagrams.
- Diffs in default behavior become reviewable as changes to a single generated artifact.
- The generator is intentionally simple and may not perfectly model all JSON-schema features or
  Mermaid's JavaScript overlay defaults; when a mismatch is discovered, we either improve the
  generator or add a small manifest override with a regression test.
