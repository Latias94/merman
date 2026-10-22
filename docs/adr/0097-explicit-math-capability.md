# ADR-0097: Explicit math capability

Accepted for the development branch on 2026-10-20, after published `0.9.1`.

Math is a positive capability selected by a consumer or exact artifact recipe. Diagram family,
SVG and layout selection must not pull in its backend or font payload indirectly. Retire
`complete-svg` and `complete-svg-elk` rather than change their historical meanings.

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

Consumers replace `complete-svg` with `svg,layout-cytoscape,math`, or `complete-svg-elk` with
`svg,layout-cytoscape,layout-elk,math`, adding `all-diagrams` or selected `diagram-*` leaves
independently. Omit math when unnecessary. Source-built CLI users add `--features math` when
they need the official archive's math support.

The existing feature matrix, artifact profiles, installation, transport and legal generators
verify the recipes. Exact artifact closure and payload measurements establish absence; optional
entries remaining in `Cargo.lock` do not mean those dependencies are compiled.
