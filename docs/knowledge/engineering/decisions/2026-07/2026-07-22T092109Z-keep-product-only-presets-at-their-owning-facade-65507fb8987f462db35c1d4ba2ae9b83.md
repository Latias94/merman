---
type: "Decision"
title: "Keep product-only presets at their owning facade"
description: "CLI-only capabilities must not become phantom features on the Rust facade; every public feature must expose a callable owner API."
timestamp: 2026-07-22T09:21:09Z
record_id: "65507fb8987f462db35c1d4ba2ae9b83"
producer_id: "codex-root"
run_id: "019f5be4-4389-72e0-9695-99fe14830072"
related_plan: "docs/plans/2026-07-22-001-refactor-capability-driven-feature-and-distribution-architecture-plan.md"
git_branch: "refactor/family-owned-architecture"
git_commit: "4e2a2d6c6"
---

# Decision

Keep each public preset on the narrowest facade that owns every capability it enables.
`preset-mmdc`, `network-icons`, `parallel-markdown`, and `shell-completions` belong to
`merman-cli`; they must not be forwarded through `merman`. A public `analysis` or `editor` leaf on
`merman` is valid only when the facade exposes the corresponding callable API or re-export. Runtime
capability reports come from one backend-owned compiled `CapabilitySet`, not per-binding `cfg`
tables.

# Context

The current `merman-cli --no-default-features` closure still includes rendering, analysis,
Reqwest/TLS, Rayon, shell-completion generation, raster/PDF backends, Jiff, and UUID. The active
plan's broad wording could be read as requiring every native preset on the Rust facade. That would
turn CLI-only tools into phantom features and preserve the same misleading closure under new names.

# Alternatives

- Put every globally named preset on every facade. Rejected because Cargo features would compile
  dependencies without a callable owner API.
- Keep only low-level leaves and remove presets. Rejected because users need ergonomic product
  recipes, while artifact profiles still require exact, non-default build closures.

# Consequences

- `merman` owns reusable Rust API presets such as basic/native/static SVG, editor, lint library,
  native SDK, and all-library capabilities.
- `merman-cli` owns `preset-ci-lint` and `preset-mmdc`, with source modules, commands, help, and
  dependencies gated together.
- Other manifests expose only presets meaningful to their artifact; they do not mirror the global
  descriptor mechanically.
- Artifact-profile verification proves the Cargo recipe. Focused facade and runtime tests prove the
  callable surface and reported capabilities.

# Citations

- `docs/plans/2026-07-22-001-refactor-capability-driven-feature-and-distribution-architecture-plan.md`
- `crates/merman/Cargo.toml`
- `crates/merman-cli/Cargo.toml`
- `crates/merman-bindings-core/src/metadata.rs`
