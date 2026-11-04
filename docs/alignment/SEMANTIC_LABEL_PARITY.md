# Semantic Edge Label Parity

Status: Active

Baseline: the [selected upstream source](README.md#machine-authorities), bound independently by
the residual catalog and signed canaries. A historical investigation does not certify a later
baseline or an untested revision.

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

The reviewed catalog is
[`label-geometry-residuals.json`](../../fixtures/_verification/label-geometry-residuals.json).
Schema 3 binds
every entry to:

- the Mermaid version, source commit, comparator revision, and fixed precision;
- diagram, fixture, semantic key, and exact text;
- input and signed upstream SVG SHA-256 digests;
- complete upstream and local geometry signatures;
- a non-empty evidence kind and rationale.

The catalog owns the exact registered entries and their baseline identities. Architecture requires
exact label geometry. Browser text and node dimensions can propagate into larger layout shifts;
their magnitude is not a coordinate tolerance. Admission requires the entire signed signature to
match exactly.

An entry becomes stale when local geometry converges, disappears, changes, or no longer belongs to
the signed artifact. Hash, text, key, version, schema, and comparator drift also fail. New residuals
must be generated as candidates, reviewed against pinned source and browser evidence, and accepted
as exact signatures. Broad coordinate tolerances and fixture-name production branches are not
allowed.

<a id="class-elk-measurement-residuals"></a>

## Class HTML Measurement and ELK

The Class canary
[`stress_class_many_relations_labels_020.mmd`](../../fixtures/class/stress_class_many_relations_labels_020.mmd)
has eight `stable_edge` residual entries. Their rationale is the distinction between browser
HTML measurement and deterministic measurement of the emitted XHTML. Mermaid's `shapeUtil.ts`
uses `getBBox()` / `getBoundingClientRect()` for final bounds, separately from its wrapping-budget
probe. Merman's [`class_prepare_html_label`](../../crates/merman-render/src/class.rs) measures the
final XHTML with the effective CSS font, including the title's inherited `bolder` weight. The
wrapping probe must not replace those final metrics or reintroduce the old Markdown width grid.

Changed node dimensions propagate through ELK placement, strict boundary clipping, terminal-channel
straightening, label anchors, and Neo path-length masks. Numeric port rows and route bends can
therefore differ from the browser artifact. The residual requires preserved directed relation
identities, text, explicit presentation, markers, stylesheet semantics, and attachment to the
intended source/target side. Every admitted path and label geometry still has to match its complete
catalog signature; this rationale grants no general route-topology exemption.

The causal review originated against Mermaid 12.2. Current source, input, upstream artifact, and
local signatures must be verified from the catalog on each refresh. Candidate extraction alone
cannot admit a change or establish pixel parity. The executable owners are the
[Class HTML typography tests](../../crates/merman-render/tests/class_html_typography_test.rs),
[Class SVG tests](../../crates/merman-render/tests/class_svg_test.rs), shared
[ELK endpoint geometry](../../crates/merman-render/src/elk_edge_geometry.rs) and
[terminal-channel rules](../../crates/merman-render/src/elk_terminal_jogs.rs), and the
[semantic-label comparator and mutation tests](../../crates/xtask/src/cmd/compare/labels.rs).

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

Run from the repository root:

```sh
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
