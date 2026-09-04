# Semantic Edge Label Parity

Status: Active

Baseline: Mermaid `11.17.2` at `dcb694ddb58dc5ad3502e7e903cac05fd812eac3`

Comparator: `semantic-label-v3`

This document defines the admission contract for edge labels whose meaning cannot be established
by SVG child order or text matching alone. The gate is verification-only. Production parsers,
layout engines, and renderers never read its residual catalog.

## Admission Boundary

`--check-dom` activates semantic-label admission independently of the selected canonical DOM
profile. DOM normalization cannot suppress a semantic-label failure, and changing
`--dom-decimals` does not change label geometry precision.

The registered adapters are:

| Diagram | Stable identity | Signed canary |
| --- | --- | --- |
| C4 | ordered relation index plus message or technology role | `upstream_docs_c4_c4_dynamic_diagram_c4dynamic_010` |
| Flowchart ELK | shared edge `data-id` | `upstream_cypress_flowchart_elk_spec_74_elk_handle_labels_for_multiple_edges_from_and_to_the_same_cou_034` |
| Architecture | direct owning edge group and path identity | `stress_architecture_batch3_parallel_edges_and_labels_057` |
| Requirement | stable edge `data-id` | `upstream_cypress_requirementdiagram_unified_spec_example_003` |
| State | stable edge `data-id` | `stress_state_batch5_parallel_edges_labels_styles_067` |
| Class | stable edge `data-id` | `stress_class_many_relations_labels_020` |
| ER | stable edge `data-id` | `upstream_cypress_erdiagram_spec_should_render_an_er_diagram_with_multiple_relationships_between_003` |

For every registered fixture, the comparator requires the complete identity set, exact text,
world-space anchor and affine basis, descendant dimensions and transforms, the owning edge path or
structured `data-points`, presentation attributes, and relevant raw stylesheet declarations.
Missing, orphaned, duplicated, non-finite, truncated, or ambiguously paired evidence fails closed.
DOM order is never an identity fallback.

Stylesheet comparison preserves selector and declaration order because both affect the cascade.
Only narrowly documented classic-Dagre Neo rules are excluded. XML comments, namespace-qualified
presentation attributes, and whitespace changes in stable identity values cannot hide a mutation.
Browser computed style remains a separate Playground gate; raw Rust SVG evidence does not claim to
execute the browser cascade.

## Geometry And Residuals

Semantic geometry is quantized to three decimals with a maximum rounding error of `0.0005`. This
precision is fixed even when canonical DOM comparison uses six decimals.

The reviewed catalog is `fixtures/_verification/label-geometry-residuals.json`. Schema 3 binds
every entry to:

- the Mermaid version, source commit, comparator revision, and fixed precision;
- diagram, fixture, semantic key, and exact text;
- input and signed upstream SVG SHA-256 digests;
- complete upstream and local geometry signatures;
- a non-empty evidence kind and rationale.

The current 30 entries are browser-measurement residuals: C4 5, Class 8, ER 2, Flowchart ELK 2,
Requirement 8, and State 5. Architecture requires exact label geometry. The largest admitted
anchor differences are approximately `24.948px` by `3px` for C4, `29.592px` by `0.333px` for
Class, `14.496px` for ER, `0.957px` for Flowchart ELK, `33.07px` by `0.125px` for Requirement, and
`0.19px` for State. The larger horizontal shifts are deterministic propagation of changed text and
node widths through the owning layout engine, not coordinate tolerances. These values are
observations only. Admission requires the entire signed signature to match exactly.

An entry becomes stale when local geometry converges, disappears, changes, or no longer belongs to
the signed artifact. Hash, text, key, version, schema, and comparator drift also fail. New residuals
must be generated as candidates, reviewed against pinned source and browser evidence, and accepted
as exact signatures. Broad coordinate tolerances and fixture-name production branches are not
allowed.

## Source-Side Evidence

Historical State and Class audits fed the exact same serialized production graph to pinned
`dagre-d3-es` and Dugong, including insertion order, compound parents, named multiedges, and label
dimensions. Those audits established parity for graph dimensions, node positions, edge-label
anchors, routed points, and identity sets. The retained contract is now expressed by focused
Dugong tests and signed State and Class semantic-label and SVG canaries instead of a standing JS
differential command.

Flowchart ELK preserves the source edge id through importer, model-order, routing, and SVG
`data-id`. Focused importer and renderer tests cover distinct labels, repeated labels, unlabeled
edges, reverse edges, width changes, and compound endpoints.

## Verification

Run from Windows PowerShell:

```powershell
cargo nextest run -p xtask compare::labels::tests --no-fail-fast
cargo run -p xtask -- compare-c4-svgs --filter upstream_docs_c4_c4_dynamic_diagram_c4dynamic_010 --check-dom --dom-mode parity --dom-decimals 3
cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity-root --dom-decimals 3
cargo run -p xtask -- compare-all-svgs --diagram class --check-dom --dom-mode parity-root --dom-decimals 3 --diagnostic-browser-text-layout
cargo run -p xtask -- compare-all-svgs --diagram flowchart --check-dom --dom-mode parity --dom-decimals 3 --diagnostic-browser-text-layout
cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --diagnostic-browser-text-layout
cargo nextest run -p dugong-graphlib -p dugong --no-fail-fast
```

The mutation suite must continue rejecting the historical C4 offset swap, path and label identity
swaps, owner changes, CSS cascade changes, non-finite geometry, zero-sample selector drift, and
stale catalog entries.
