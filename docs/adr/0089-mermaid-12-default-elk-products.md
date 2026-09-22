# ADR-0089: Mermaid 12 Default ELK Products

## Status

Accepted for `0.8.0-alpha.7`. Supersedes the product-default selections in ADR-0085 and
ADR-0088. Their explicit feature aggregates, artifact-specific license boundaries, and Rustdoc
math opt-in decision remain in force.

## Date

2026-09-22

## Context and decision

Mermaid 12.0.0 includes ELK in its standard runtime and selects it for its ordinary graph layout.
Keeping ELK optional in ordinary Merman products would make the first render use a different
algorithm from the selected reference. The maintainer accepted the larger dependency and EPL-2.0
distribution closure as part of the Mermaid 12 alignment plan.

The `merman` facade therefore defaults to `complete-svg-elk`; the CLI defaults include
`layout-elk`; and `merman-rustdoc` defaults to `svg + layout-cytoscape + layout-elk`. Rustdoc math
remains explicit. The existing `complete-svg` aggregate still means
`svg + layout-cytoscape + math` and excludes ELK. Renderer and transport crates retain empty
defaults, with artifact recipes selecting their positive capability leaves.

Explicit lean consumers disable default features and select `complete-svg` or narrower leaves.
When a graph family permits Mermaid's unregistered-layout fallback, an unavailable ELK loader
resolves to Dagre before execution. A selected backend's failure, cancellation, or resource
exhaustion propagates to the caller; it never triggers a second backend. This fallback does not
claim ELK geometry parity and does not apply to missing math support.

## Alternatives considered

- Keep ordinary defaults without ELK: preserves the smaller source dependency closure, but fails
  the agreed default-layout contract with Mermaid 12.
- Put ELK into `complete-svg`: changes existing explicit lean selections and removes the useful
  no-ELK aggregate. Retaining its meaning preserves an intentional consumer choice.
- Add ELK to product defaults through existing leaves and `complete-svg-elk`: selected. This
  matches upstream while keeping explicit lean builds and lower-level feature composition.

## License and verification boundary

Translated algorithms remain in the EPL-2.0 `merman-elk-layered` crate, including the additional
registered algorithms. Merman-owned code retains its MIT/Apache-2.0 grant. Distributed artifacts
must carry the notices, license text, and translated-source provenance selected by
`capabilities/artifact-profiles-v1.json` and `docs/release/THIRD_PARTY_COMPONENTS.json`.

The existing feature-matrix, artifact-profile, CLI installation, and legal-material checks own
verification. Default products must contain ELK; explicit no-ELK recipes must exclude its
dependency closure; Rustdoc defaults must exclude math. Layout tests must distinguish successful
loader fallback from backend errors. This decision records the intended boundary and does not
replace those executable checks or declare the Mermaid 12 admission complete.
