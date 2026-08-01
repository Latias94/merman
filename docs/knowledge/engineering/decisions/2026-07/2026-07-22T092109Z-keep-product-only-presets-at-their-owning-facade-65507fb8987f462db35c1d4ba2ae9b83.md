---
type: "Decision"
title: "Retire the global Cargo preset lattice"
description: "Use direct capability leaves in owner-specific artifact recipes; keep only the result-named complete-svg aggregate on the Rust facade."
timestamp: 2026-07-22T09:21:09Z
record_id: "65507fb8987f462db35c1d4ba2ae9b83"
producer_id: "codex-root"
run_id: "019f5be4-4389-72e0-9695-99fe14830072"
related_plan: "docs/plans/2026-07-22-001-refactor-capability-driven-feature-and-distribution-architecture-plan.md"
git_branch: "refactor/family-owned-architecture"
git_commit: "4e2a2d6c6"
---

# Decision

Retire the repository-wide `preset-*` Cargo feature lattice. Public Cargo features are direct,
positive capability leaves such as `svg`, `analysis`, `layout-elk`, `math`, `png`, and
`system-timezone`. The `merman` facade keeps one ergonomic result-named aggregate,
`complete-svg`, which expands to SVG, both supported layout engines, and math. It is a compile
convenience only; it does not select runtime policy or promise that another feature is absent.

CLI, Web, Typst, FFI, UniFFI, Node, and platform packages select their direct leaf sets in
owner-specific artifact profiles. Their product names and artifact profiles are not Cargo feature
names and must not be forwarded through unrelated crates. Runtime capability reports come from one
backend-owned compiled catalog, not per-binding `cfg` tables.

# Context

Cargo feature unification is additive. A global table of product, transport, runtime, and release
presets cannot express exclusions and makes a feature list look like an exact artifact recipe when
it is not. It also encourages phantom features that compile dependencies without a callable owner
API. Exact absence claims belong to `default-features = false` artifact profiles and their
executable closure probes.

# Alternatives

- Keep product presets on each owning facade. Rejected: it still creates a second public vocabulary
  and keeps profile/runtime concerns in additive Cargo flags.
- Keep a global `preset-*` table for every workflow. Rejected: it creates a combinatorial lattice and
  cannot guarantee exclusion or transport correctness.
- Expose only leaves with no aggregate. Rejected for the Rust facade: `complete-svg` is a stable,
  result-named convenience that covers the ordinary SVG use case without pretending to be a
  product or release profile.

# Consequences

- `merman-cli` owns its command and export leaves directly; its default and lint recipes are
  declared by the CLI artifact profiles.
- Artifact profiles prove exact package, target, default-feature policy, leaf set, runtime IDs,
  outputs, dependency closure, and size evidence.
- A new aggregate is accepted only when its name describes a stable result and repeated user
  composition; `static`, `native-sdk`, `editor`, `web`, `all`, and negative `no-*` variants remain
  profile or package concepts rather than Cargo features.
- The old product-only-preset proposal is retained by this file's history, but this decision is
  the current successor for the capability-driven refactor.

# Citations

- `docs/plans/2026-07-22-001-refactor-capability-driven-feature-and-distribution-architecture-plan.md`
- `crates/merman/Cargo.toml`
- `crates/merman-cli/Cargo.toml`
- `crates/merman-bindings-core/src/metadata.rs`
