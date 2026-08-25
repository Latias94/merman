# ADR-0082: Independent Rustdoc Integration Paths

- Status: accepted
- Date: 2026-08-14
- Supersedes: the Rustdoc-specific single-integration assumption in ADR-0076

## Context

`merman-rustdoc` renders deterministic static SVG during proc-macro expansion. It provides a useful
one-step `cargo doc` experience, but Cargo resolves and builds its native renderer closure whenever
the dependency feature is selected. `cfg_attr(doc, ...)` controls macro expansion; it is not a
doc-only dependency mechanism. Even the base SVG configuration therefore remains a material Cargo
cost, while the complete default also carries Cytoscape, ELK, and math.

Some users prefer that ergonomics. Published libraries and dependency-sensitive workspaces often
need the opposite contract: Rustdoc should consume static files without compiling or executing any
Merman package. docs.rs also cannot be relied on to install or run an external generator. The
`complete-svg` default now carries the base deterministic SVG, Cytoscape layout, and math closure;
the optional ELK implementation is selected separately by `complete-svg-elk` when a documentation
artifact accepts its EPL-2.0 notice boundary.

## Decision

Merman supports two explicit, independently packaged Rustdoc integrations:

1. `merman-cli rustdoc build/check` is the checked-generation path. It reads declared local
   Markdown or Mermaid inputs, renders and validates a complete deterministic light/dark static-SVG
   bundle, and owns only `docs/generated/merman-rustdoc`. The consuming crate commits those files
   and uses Rust's native `#[doc = include_str!(...)]` or `#![doc = include_str!(...)]`.
2. `merman-rustdoc` remains the one-step attribute path. It renders item documentation during
   `cargo doc` and retains its native `complete-svg` default, smaller explicit feature choices,
   artifact profile, tests, and release surface.

Neither integration may depend on, execute, discover, or automatically fall back to the other.
The CLI feature and `cli-release` artifact recipe own the checked-generation command. The
`rustdoc-static-svg` artifact profile continues to own the proc-macro recipe. Both use the pinned
Mermaid semantics and static, JavaScript-free SVG identity, but their authoring and distribution
contracts are deliberately different.

The CLI path treats generated files as reviewable build artifacts. `build` may mutate only its
fixed managed output root and publishes the fragment set plus receipt transactionally. `check`
reconstructs the same expected bundle without writing. Git owns rollback of successful source and
generated states; the transaction journal only recovers interrupted publication. docs.rs consumes
the packaged fragments and never runs the generator. Each generated fragment may appear at most
once on a rendered page because its SVG DOM IDs are deterministic within the fragment.

## WASM Evidence

A bounded full-capability guest experiment tested whether an embedded WASM renderer should replace
the native macro backend in this change. It passed the preregistered host closure, package size,
exact representative SVG parity, sandbox, reproducibility, and offline-package gates. The measured
warm render median was `28.101x` native against a frozen `<=2x` admission gate. The experiment is
therefore rejected as a product backend for this decision, and no guest, host, binary, feature, or
fallback is retained. A future proposal must rerun the frozen gates rather than weakening them.

## Consequences

- Dependency-sensitive crates can publish offline static diagrams with zero attributable Merman
  packages in their normal or build Cargo graph.
- Macro users keep one-step item-documentation expansion and explicitly accept its build cost.
- CLI users add a generation/freshness step and commit generated Markdown; renderer upgrades can
  produce large but reviewable diffs.
- Crate-level docs and standard `include_str!` are supported by the CLI path, while macro tree
  traversal remains limited to syntax visible to the attribute.
- Release checks must keep both product recipes, docs, package contents, capabilities, completions,
  and manual pages current without merging their dependency graphs.
- Browser-side Mermaid.js, build-script downloads, CLI auto-discovery, and postprocessed Rustdoc
  HTML remain outside the supported architecture.
