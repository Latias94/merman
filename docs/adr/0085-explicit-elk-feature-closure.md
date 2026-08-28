# ADR-0085: Explicit ELK Feature Closure And Default Distribution Boundary

## Status

Accepted.

## Date

2026-08-25

## Context

The Rust facade and the Rustdoc integration historically exposed a `complete-svg` aggregate that
compiled SVG, Cytoscape layout, ELK layout, and math. That name was convenient, but it made a normal
dependency declaration pull an EPL-2.0 translated ELK implementation into the consumer's Cargo
graph. The crate metadata still correctly described Merman's own MIT/Apache-2.0 grant, while a
feature-enabled build had a larger third-party license closure. Users had to inspect transitive
features and release notices to discover that distinction.

The CLI and native binding products have a different distribution boundary. CLI release archives
and the prebuilt platform profiles intentionally provide both layout engines, while FFI and UniFFI
source crates need to let an embedding application choose a direct closure. One feature name cannot
serve all of those artifacts without hiding the legal and size consequences of the selection.

## Decision

1. `merman` and `merman-rustdoc` keep `complete-svg` as the ergonomic default, but define it as
   `svg + layout-cytoscape + math`. It does not enable ELK.
2. Both crates expose `complete-svg-elk = ["complete-svg", "layout-elk"]`. This is the explicit
   opt-in for the EPL-2.0 ELK implementation and its translated-source, attribution, and source-
   provenance obligations. The existing `layout-elk` leaf remains available for direct recipes.
3. `merman-cli` defaults omit `layout-elk`. The `cli-release` artifact and cargo-dist release
   recipe retain ELK so the published complete archive remains feature-complete; its archive must
   carry the matching third-party notices and provenance.
4. FFI, UniFFI, bindings-core, WASM, and other transport crates retain empty defaults and direct
   feature leaves. They do not grow a misleading cross-product aggregate. Their README and release
   recipes identify ELK (EPL-2.0) and math/font (OFL-1.1) closures separately.
5. The package's `MIT OR Apache-2.0` metadata continues to describe Merman-owned code only. Legal
   compliance is evaluated per selected feature closure and per distributed artifact through the
   artifact profile, generated capability surface, third-party component inventory, notices, and
   source provenance. A facade license expression is not widened to include dependency licenses.

## Migration

Applications that require ELK from the facade or Rustdoc integration must select
`complete-svg-elk` (or the direct `layout-elk` leaf) and distribute the corresponding notices. An
ordinary dependency declaration now avoids the ELK closure by default. CLI users who need ELK may
select `layout-elk` for a source build or use the published release archive; `cargo install` keeps
the lean default unless features are explicitly requested.

This is an aggregate-membership change, not removal of the `layout-elk` capability. The feature
matrix, profile contracts, calibration tool, generated capability projections, and legal notice
projections are updated together so a stale recipe fails validation rather than silently changing
the artifact.

## Verification

The following are the owning checks for this boundary:

- `cargo run --locked -p xtask -- verify-feature-matrix`;
- `cargo run --locked -p xtask -- verify-artifact-profiles`;
- `python scripts/verify-third-party-licenses.py` and
  `python scripts/sync-release-legal-materials.py --check`;
- CLI installation/profile contracts and the default/release `profile_contract` tests;
- representative dependency-closure checks for the explicit ELK and non-ELK recipes.

## Consequences

- A normal Rust dependency has a smaller and more legible legal/compile-time closure.
- Users who need ELK make the EPL-2.0 boundary visible in the feature selection and artifact
  recipe.
- Release artifacts may still be complete and include ELK, but their notices are artifact-specific
  rather than inferred from the crate's top-level license field.
- Existing direct leaf recipes remain expressible, while documentation and generated contracts
  prevent defaults, release profiles, and legal projections from drifting apart.
