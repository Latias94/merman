# Mermaid 12.2 Fixture Coverage Review

Reviewed on 2026-10-13 against the selected `mermaid@12.2.0` source, commit
`1fb7869866dd619168d840291c9c54b4f129758b`. This is a behavior-coverage review, not a claim
that fixture counts prove every Mermaid combination. Exact verification belongs to the tested revision.

## Inventory and conclusion

The 37-family primary matrix has 3,755 source inputs and semantic goldens, 3,752 layout goldens,
3,731 actual upstream SVGs and 24 explicitly excluded upstream inputs. Exclusions and unavailable
layout projections are recorded by the existing family policies. The top-level corpus contains
3,781 inputs including the external ZenUML lane and editor issue fixtures. Private historical and
family-local corpora are additional evidence; they are not included in these counts.

Core 12.2 behavior has executable regression coverage. The audit identified combination gaps,
not a need to add many interchangeable examples. Six small inputs address the most useful gaps:
four enter the primary matrix and two retain their actual browser reference as family-local evidence.
The new inputs exposed and fixed two real omissions: Mindmap rect preprocessing/padding and
HandDrawn Circle/DoubleCircle RoughJS shape groups.

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

Agentflow, Usecase, ABNF/EBNF/PEG Railroad and some recently admitted families have few primary
fixtures. Parser, editor, resource and family integration tests provide additional coverage; a small
primary count alone does not establish a missing feature. Their next useful review should map
upstream grammar productions and family-specific render branches to those tests rather than copy
large batches of similar diagrams.

No new diagram, syntax family, security rule, feature boundary or FFI/editor schema is introduced
by 12.2. Existing negative, recovery, security, resource and cross-surface tests therefore remain the
relevant regression layer. The next focused additions, if those paths change, are a cross-family
ELK label-guard rejection and a Swimlane nested/explicit/synthetic-lane security combination. These
are follow-up coverage opportunities, not untested new 12.2 behavior.
