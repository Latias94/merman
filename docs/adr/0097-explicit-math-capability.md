# ADR-0097: Explicit math capability

Accepted for the development branch on 2026-10-20, after published `0.9.1`.

Amended on 2026-10-25 by maintainer decision: retain the convenience aggregate names with
math-free membership, replacing this ADR's original aggregate-retirement decision.

Main-line Apple prebuilt selection is amended by
[ADR-0100](0100-prebuilt-product-entry-points.md), which selects `math` explicitly for that
binary product. Source defaults and published v0.9.x recipes retain this ADR's policy.

Math is a positive capability selected by a consumer or exact artifact recipe. Diagram family,
SVG and layout selection must not pull in its backend or font payload indirectly.
`merman` and `merman-rustdoc` retain `complete-svg = ["svg", "layout-cytoscape"]` and
`complete-svg-elk = ["complete-svg", "layout-elk"]`. These are output/layout conveniences,
not diagram selectors or artifact identities. Their previously math-enabled behavior changes;
the changelog and migration guide require explicit `math` for mathematical labels.

The facade defaults to `all-diagrams,svg,layout-cytoscape,layout-elk`. Rustdoc retains its existing
direct non-math defaults. CLI source defaults and its `rustdoc` leaf omit math; the official
`cli-release`/cargo-dist recipe includes it explicitly. Generic native/static SVG and Web
full/render recipes omit math. Explicit C reference/native SDK/exhaustive recipes retain their
audited selection; existing math-free native bindings, Node and Typst products keep theirs.
Custom WASM source builds may opt into the existing leaf without adding a published package.

This does not narrow formula parsing or replace a backend. Required mathematical labels return
the existing missing-capability error when math is unavailable, preserving family-specific literal
behavior. No network, JavaScript or literal-output fallback is introduced. Transport-owned
catalogs continue to constrain operations even if Cargo unifies a wider dependency feature set.

These clauses supersede the math-bearing facade/aggregate rules in ADR-0076/0085/0088/0089/0091,
the original Rustdoc default in ADR-0087, and the Web math recipe in ADR-0079.
ELK selection and its EPL-2.0 notices/provenance remain unchanged.
Installed `0.9.1` products and dated performance receipts retain their original capability sets.

Consumers can keep `complete-svg` or `complete-svg-elk` and add `math` to preserve their previous
mathematical-label support, or select the corresponding output/layout leaves directly. Add
`all-diagrams` or selected `diagram-*` leaves independently when defaults are disabled. Omit
math when unnecessary. Source-built CLI users add `--features math` when they need the official
archive's math support. Defaults and exact delivery recipes continue to use direct leaves;
retaining convenience names does not widen those recipes or relabel historical measurements.

The existing feature matrix, artifact profiles, installation, transport and legal generators
verify the recipes. Exact artifact closure and payload measurements establish absence; optional
entries remaining in `Cargo.lock` do not mean those dependencies are compiled.
