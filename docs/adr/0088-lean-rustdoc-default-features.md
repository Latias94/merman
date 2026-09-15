# ADR-0088: Lean Rustdoc Default Features

## Status

Accepted for the next release after `0.8.0-alpha.6`.

## Date

2026-09-10

## Context

The attribute macro renders Mermaid into static SVG during Rust documentation compilation. It
avoids a browser or JavaScript runtime, but consumers still compile the selected native renderer
and its dependencies. Optional documentation dependencies protect ordinary builds only while the
consumer's documentation feature is disabled.

The dependency review found that math accounts for 56 of the 173 packages in the macro's former
default graph on the measured Windows host. This is an active dependency count, not a build-time
measurement. Math is useful for some documentation, but it is not required for ordinary API
diagrams. Sharing a default aggregate with the general rendering facade made that cost implicit.

## Decision

1. `merman-rustdoc` defaults to the positive leaves `svg` and `layout-cytoscape`. Math is explicit.
   Keeping Cytoscape preserves the existing default layout capability while removing the math and
   font closure from the default macro build.
2. `complete-svg` continues to mean `svg + layout-cytoscape + math` in both `merman-rustdoc` and
   `merman`. `complete-svg-elk` continues to add `layout-elk`. The facade's default remains
   `complete-svg`; no new aggregate or per-diagram feature is introduced.
3. Missing optional capabilities must produce actionable diagnostics identifying the feature to
   enable. The macro does not silently drop mathematical content or select an external backend.
4. Keep the two Rustdoc integration paths. The optional macro serves inline item documentation;
   CLI-generated fragments serve consumers that need no renderer dependency during their builds.
   The shared `merman-doc` library continues to own Markdown discovery and embedding, without
   choosing rendering capabilities.
5. Validate the changed Cargo closure and representative rendering behavior using existing checks.
   This change introduces no performance benchmark suite, timing budget, global render cache,
   repeated-render optimization, or font implementation replacement.

This decision supersedes the Rustdoc-default coupling in ADR-0076 and ADR-0085. Their positive
feature vocabulary, explicit aggregate membership, ELK boundary, and artifact-profile authority
remain in force.

## Migration

The published `0.8.0-alpha.6` macro retains its original `complete-svg` default, including math.
This decision applies to the current source and the next release; it does not change that
published artifact or require a release/version operation now.

Consumers using mathematical labels should add `features = ["math"]` to their existing
`merman-rustdoc` dependency, or select `features = ["complete-svg"]` to state the former aggregate
explicitly. Existing `complete-svg` and `complete-svg-elk` selections retain their behavior.
Consumers already using `default-features = false` keep their explicit capability selections.

An optional consumer feature should continue to gate the macro dependency and the `cfg_attr`.
`--all-features` enables that optional dependency too; Rust's `cfg(doc)` does not suppress Cargo
dependency compilation. Users requiring no renderer dependency can generate and package fragments
through `merman-cli rustdoc build/check`.

## Verification

- Cargo metadata and representative closure checks prove that the default macro has SVG and
  Cytoscape without math/font or ELK dependencies, and that explicit math remains available.
- Default-feature examples and regressions exercise ordinary diagrams and actionable unavailable
  capability errors; explicit full-feature tests retain mathematical rendering coverage.
- Feature-matrix contracts distinguish the macro default from the explicit complete SVG profile.
  Existing release artifacts keep their descriptor-owned recipes.
- README, API documentation, package-surface guidance, and the Unreleased changelog describe the
  same migration without rewriting published release history.

## Consequences

The macro has a smaller default dependency closure while retaining its static, offline rendering
contract. Mathematical documentation requires explicit feature selection. The remaining SVG
closure is substantial, so the macro is not described as dependency-free or as a replacement for
the pre-generated fragment path. Package-count changes alone do not establish faster builds.
