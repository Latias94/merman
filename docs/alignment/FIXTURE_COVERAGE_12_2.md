# Mermaid 12.2 Fixture Coverage Review

Reviewed on 2026-10-13 against the selected `mermaid@12.2.0` source, commit
`1fb7869866dd619168d840291c9c54b4f129758b`. This is a behavior-coverage review, not a claim
that fixture counts prove every Mermaid combination. Exact verification belongs to the tested revision.

## Inventory and conclusion

The 37-family primary matrix has 3,761 source inputs and semantic goldens, 3,758 layout goldens,
3,737 actual upstream SVGs and 24 explicitly excluded upstream inputs. Exclusions and unavailable
layout projections are recorded by the existing family policies. The top-level corpus contains
3,787 inputs including the external ZenUML lane and editor issue fixtures. Private historical and
family-local corpora are additional evidence; they are not included in these counts.

Core 12.2 behavior has executable regression coverage. The audit identified combination gaps,
not a need to add many interchangeable examples. Twelve small audit inputs address the most useful
gaps: ten enter the primary matrix and two retain their actual browser reference as family-local
evidence. The first group exposed Mindmap rect preprocessing/padding and HandDrawn Circle/DoubleCircle
RoughJS shape-group omissions. The follow-up exposed Agentflow self-loop merging and label order,
Usecase SVG label/background measurement, and Swimlane synthetic colors and label anchors.

The new Rough circle path also has explicit resource regressions: work is charged before
sampling or hachure allocation, checked arithmetic rejects oversized counts, cancellation is
observed before each ring, and `stroke:none` omits the outline without changing fill seed order.

## Changed behavior

| Behavior | Existing evidence | Added audit evidence | Remaining boundary |
| --- | --- | --- | --- |
| Mindmap branch/depth, irregular indentation, insertion stability, eleven-color cycle, root and child-edge colors | Core semantic/typed/editor regressions; depth SVG fixture | `mindmap/local_mermaid_12_2_neo_depth_palette_cycle.mmd`: thirteen-depth Neo chain, second branch and custom palette | Browser text and computed-color presentation remain host dependent; exact CSS selectors/values are asserted |
| Mindmap SVG centering, shapes, font priority, Markdown image/entity behavior | Seven-shape SVG fixture and rendering integration | `_family_local/mermaid-12-2/mindmap/local_mermaid_12_2_svg_markdown_font_entities.mmd`: actual reference SVG checked for characters, emphasis, ignored images, font choice and rect padding | Narrow browser wrapping may split tspans differently; this input does not weaken the primary comparator |
| Circle/DoubleCircle minimum-width opt-out and Neo padding | Dagre/ELK × classic/Neo × HTML/SVG × min 0/120 integration; primary circle fixture; forty-five controlled browser layouts and exact endpoint regression | `_family_local/mermaid-12-2/flowchart/local_mermaid_12_2_handdrawn_circle_min_width.mmd`: Rough ring grouping, order, classes, fill/stroke roles, seed determinism and source-derived dimensions | Rough paint bbox/control points and browser text wrapping are not pixel-perfect admission |
| ELK label-aware terminal straightening | Both iteration orders and directions, label reprojection, compound origin, Liang–Barsky boundary, resource accounting and atomic failure tests; controlled State 039/040 | `flowchart/local_mermaid_12_2_elk_label_clearance_8368.mmd`: complete Draft/Review/Published input from the upstream issue regression | Different measured text widths can choose different legal guard branches; exact reviewed State receipts retain this boundary |
| Swimlane source order, terminal pass, title rails/direction, scoped IDs and actual-engine clipping | Four upstream 12.2 regressions; mixed-frontier, fixed-stage, DOM and clipping tests; thirty historical DDLT inputs | `swimlane/local_mermaid_12_2_mixed_source_order.mmd`: LR connected/isolated nodes and non-alphabetical lanes | Locale-sensitive identifier collation remains explicitly bounded |
| Swimlane label-derived dimensions | HTML/SVG minimum-width and circle opt-out integration | `swimlane/local_mermaid_12_2_label_min_width.mmd`: classic SVG labels, minimum 180, long/short rectangles, a short diamond and circles | Empty labels are tested at the model level: upstream rejects the proposed empty quoted DSL input |

The two private browser snapshots retain exact selected source/runtime identities and input/SVG
hashes in `_family_local/mermaid-12-2/_reference-provenance.json`. Ordinary family integration tests
consume their source and reference SVG. This record is evidence, not another admission registry.

## Broader corpus limits

The follow-up maps small-corpus grammar and render combinations to actual selected-runtime SVGs.
All six inputs have semantic and layout goldens and enter the existing primary DOM policies.

| Combination | Added primary input | Executable assertion |
| --- | --- | --- |
| ABNF numeric alternatives, case-insensitive numeric prefixes, fixed/bounded repetition and optional groups | `railroadAbnf/local_numeric_groups_bounds.mmd` | Numeric bounds, nested optional structure, ordered text and loop/choice DOM against the actual reference |
| EBNF exception inside a sequence and grouped alternative postfixes | `railroadEbnf/local_exception_group_postfix.mmd` | Exception ownership, optional/one-or-more wrappers, rule/terminal text and branch classes |
| PEG predicates over grouped suffixes and wildcard | `railroadPeg/local_predicate_group_suffix.mmd` | Positive/negative predicate ownership, optional/one-or-more/zero-or-more wrappers and reference text order |
| Agentflow nested expanded/collapsed groups, crossing routes and self-loop | `agentflow/local_nested_collapse_routes.mmd` | Hidden child ownership, collapsed target rewriting, three unmerged Dagre helper segments, markers and label/background sibling order |
| Usecase compound ELK label guard | `usecase/local_elk_boundary_label_guard.mmd` | A prior route moves; a later eligible candidate would cover that route and is rejected. The opposite direction remains a control; raw provider geometry and ports stay unchanged |
| Distinct nested, explicit and synthetic Swimlane lanes under strict security | `swimlane/local_nested_mixed_lane_security.mmd` | LR/RL ownership, scoped unique IDs, cross-lane endpoints, HTML/SVG entities, unsafe-link filtering and declaration-slot palette colors |

Usecase labels retain distinct text and layout boxes: SVG edge backgrounds add two pixels per
side before provider layout and label-clearance checks, while the text bbox controls centering.
The new test compares wrapper, background and marker contracts with the actual reference. A single
instrumented upstream ELK render confirms that the provider receives the padded dimensions.

Two existing Swimlane inputs, `upstream_7_car_sales_constr` and `upstream_docs_basic`, exposed
paint outside the root in Linux CI and the same Chromium replay on Windows. The repair follows
the source's updated-path signal when positioning labels. Regression tests preserve original
layout anchors for unchanged pinned routes and still apply the delta for a genuinely updated path;
the root bounds and comparator policies are unchanged.

Parser, editor, resource and family integration tests remain additional evidence. A small primary
count alone does not establish a missing feature. Browser-dependent text measurement, Rough paint
coordinates and locale-sensitive identifier ordering retain the documented boundaries above.

No new diagram, syntax family, security rule, feature boundary or FFI/editor schema is introduced
by 12.2. Existing negative, recovery, security, resource and cross-surface tests therefore remain the
relevant regression layer. Future additions should target an uncovered source branch or a concrete
reported failure rather than duplicate the covered combinations.

## Product API follow-up

The 2026-10-13 review of issues
[#197](https://github.com/Latias94/merman/issues/197),
[#198](https://github.com/Latias94/merman/issues/198),
[#199](https://github.com/Latias94/merman/issues/199) and
[#200](https://github.com/Latias94/merman/issues/200) identifies additional product contracts:

| Issue | Current source evidence | Repair and regression boundary |
| --- | --- | --- |
| #197: C4 presentation labels | The presentation compiler maps C4 element fills/borders; `c4/render.rs` still defaults element text to white and relationship text/lines to fixed gray | Carry text/line roles into C4 defaults, preserving explicit authored element/relationship colors; test light/dark bindings output and readable labels |
| #198: ZenUML presentation theme | `zenuml_css()` receives only the diagram ID and emits fixed colors/fonts; fragment section paint also contains explicit colors | A presentation-aware external-family adapter must cover frames, participants, messages and fragments, while preserving the selected plugin's plain Mermaid behavior |
| #199: cross-family presentation omissions | Wardley anchors/dividers are fixed black; Gantt tick/domain `currentColor` lacks a themed `color`; XYChart calls config-independent base CSS; the Mindmap gray background comes from the shared fallback cascade's `labelBkg` default | Test renderer and fallback role propagation, explicit style precedence, root-scoped typography and SVG/PNG equivalence across the four families |
| #200: exported label transforms | The current CLI reproduces the rotated Swimlane title in parity SVG and horizontal text in `resvg-safe`; the shared fallback accumulates only translations before hoisting text to the root | Preserve complete affine transforms for text and background, including list order, nested ancestors and local coordinates exactly once; test the actual Swimlane input and PNG, plus rotation pivots, scale, matrix and metadata preservation |

These are P2 product/theme and export defects that remain outside the release alignment repair.
The shared export transform failure should be addressed first. A Swimlane-only rotation adjustment
or a global color replacement would miss the common contract. Presentation fixes must retain
ordinary Mermaid reference output and explicit DSL styling.
