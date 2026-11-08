# Mermaid 12.3 development migration

Current source development selects Mermaid `12.3.0` at
`2086904e76c692331e34a45d7cc3bc30860d040e`. Published `0.10.0-alpha.1`
artifacts retain their Mermaid `12.2.0` identity. Build wrappers and native/WASM
artifacts from the same source revision when using these development changes.

## Selected behavior graph

The [reference bundle](../../tools/upstreams/MERMAID_REFERENCE_BUNDLE.json),
[selection receipt](../../tools/upstreams/MERMAID_SELECTION_DECISION.json) and
[source lock](../../tools/upstreams/REPOS.lock.json) own the exact identities.
Both reference workspaces select `elkjs 0.12.1`, Cytoscape `3.34.4`,
`@braintree/sanitize-url 7.1.2`, DOMPurify `3.4.17`, and reference CLI `12.2.0`.
Parser `2.0.1`, Tidy Tree `1.0.1`, and ZenUML plugin `1.0.1` keep their
independent source identities. Selected ZenUML Core remains `3.50.1`; the
Mermaid source workspace's older Core resolution does not replace it.

The alpha.2 dependency refresh advances ELK from the target workspace's `0.12.0`
oracle to the compatible `0.12.1` patch. Source comparison identifies the opposing
self-loop port-order correction; Rust already sorts those sides and now has
explicit order-independence regressions. Public worker replays cover both versions,
while historical instrumented phase observations retain their `0.12.0` identity. The
[ELK delta record](../../crates/merman-elk-layered/MERMAID_12_3_DELTA.md)
distinguishes supported source changes, actual npm observations, and unexposed
options. Existing feature and license boundaries remain: defaults-off consumers
can omit `layout-elk` and its EPL algorithm closure. No additional Cargo feature
is introduced for these shapes or algorithms.

## Flowchart and shared shapes

Attached edge identifiers now preserve adjacent node shapes and shape-data
syntax. Brackets and label delimiters cannot occur inside edge identifiers.
Flowchart edge metadata accepts positive JavaScript integer `step` values;
repeated steps animate together and missing step numbers retain timed gaps.
`flowchart.stepDuration` defaults to `1`, and strict boolean
`flowchart.stepNumbers` enables numbered badges.

Raw parity SVG includes the fixed Step motion/highlight structure. Browser
admission validates that closed structure; arbitrary SMIL remains forbidden.
Reduced-motion preferences hide moving marks while retaining badges. Static
inline, resvg-safe SVG, PNG/JPEG/PDF keep original edges and badges while
removing animated carriers. See the
[SVG output contract](../rendering/SVG_OUTPUT_PIPELINE.md).

Nine `bpmn-*` shapes are available through existing Flowchart shape syntax.
They use notation-body intersections separately from caption bounds, and the
built-in BPMN glyph fallback respects registered packs. This adds neither a BPMN
diagram language nor public syntax for internal producer options such as
attachment, marker rows, or pool roles. Grid and the unwired anchor pipeline
remain upstream implementation groundwork, outside the registered layout surface.

## Rust source changes

Current development changes `JourneyTask::score` in `merman_model::families::journey`
and `JourneyTaskLayout::score` in `merman_render::model` from `i64` to `f64`.
Use floating-point literals such as `score: 4.0` and update integer arithmetic in
direct consumers. Fractional scores now retain their position and satisfaction
expression. Direct canonical/layout Serde output may represent whole scores as
`4.0`; Mermaid compatibility JSON retains `4` for representable whole scores and
`3.5` for fractional scores. The existing `scoreIsNaN` compatibility flag remains.

`merman_model::families::block::BlockEdge` adds `thickness: BlockEdgeThickness`
and `pattern: BlockEdgePattern`. Direct struct literals initialize them with
`BlockEdgeThickness::Normal` and `BlockEdgePattern::Solid`, or use
`..Default::default()`. Their Serde defaults preserve older inputs. Mermaid
compatibility edge records now include the upstream `thickness` and `pattern`
strings, and SVG consumes those semantics together with both endpoint markers.

Struct-literal consumers of `FlowEdge` add `step: None` when no step is authored.
`FlowchartModel` carries `dropped_subgraph_members` for shared-member provenance;
ordinary builders can use its default empty collection. These new data use
defaulted, omitted-when-empty JSON fields where applicable. Author metadata
cannot set internal `laneRole`, `laneIndex`, or `drawnExtent` values.

Direct Rectpacking `Options` literals add `regular_packing_minimum: None` when
they have no regular-pipeline title floor; literals using `..Default::default()`
need no change. That floor is distinct from `minimum_width`/`minimum_height`,
which also apply to the earlier Box branch. The ELK input importer now omits
empty-string layout labels, matching the selected worker; measured-only adapter
labels retain a nonempty placeholder. Raw owner-title dimensions still reserve
compound space before that text filter.

Mindmap compatibility JSON no longer includes `diagramId`. Its former generated
UUID owner, `diagram_id_from_context`, `PresentationContext::Mindmap`,
`PresentationContext::mindmap_diagram_id`, `CompatJsonMetadata::diagram_id`, and
`PresentationDataError::MissingOutputIdentity` are removed. Construct Mindmap
presentations with `PresentationContext::None`; SVG identity remains an explicit
render option. Independent projection no longer requires ambient runtime identity.

Agentflow's input syntax resolves its internal `lean_right` alias to
`lean-right`. Explicit authored shape names retain upstream validation. Usecase
Include/Extend edge labels render as `«include»` and `«extend»`; their semantic
relationship names stay unchanged. Swimlane preparation, label ranks, nested
bounds, directional layout and endpoint docking follow the selected source.

## Independent syntax package

`distribution/tree-sitter-mermaid` retains its own selected baseline and package
contracts. This Mermaid runtime move intentionally leaves that syntax baseline
unchanged. Tree-sitter output does not become semantic input to Merman.

## Compatibility evidence and limits

The [alignment authority map](../alignment/README.md) identifies the selected-source
fixture manifests and exact comparison receipts. The
[upgrade playbook](../release/MERMAID_UPGRADE_PLAYBOOK.md#7-refresh-generated-surfaces-and-evidence)
defines verification for a source build. A previous successful run does not qualify a
later revision or a separately built artifact.

Headless text measurement, browser `getBBox()` values, fonts, HTML labels and
hand-drawn output retain bounded artifact differences. Comparison receipts bind
specific inputs, SVG signatures and admitted modes; they do not grant a general
geometry tolerance or establish pixel parity.

Step's strict/sandbox marker attributes use the shared DOMPurify DOM-value
admission. The selected DOMPurify rejects `stroke:DarkGray` and `stroke:#123456`
while preserving ordinary colors. Loose mode retains the raw values; marker
identity is unchanged. Classic, Neo and Hand-drawn outputs share the animation,
document isolation and reduced-motion contracts described above.
