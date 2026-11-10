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

## Empty Class programs

The selected Class grammar requires a statement after `classDiagram`. A header-only program
rejects. Use a direction statement to render a diagram with no classes or relationships:

```mermaid
classDiagram
direction TB
```

## Rust source changes

Current development changes `JourneyTask::score` in `merman_model::families::journey`
and `JourneyTaskLayout::score` in `merman_render::model` from `i64` to `f64`.
Use floating-point literals such as `score: 4.0` and update integer arithmetic in
direct consumers. Fractional scores now retain their position and satisfaction
expression. Direct canonical/layout Serde output may represent whole scores as
`4.0`; Mermaid compatibility JSON retains `4` for representable whole scores and
`3.5` for fractional scores. The existing `scoreIsNaN` compatibility flag remains.

`QuadrantChartStyles::radius` changes from `Option<i64>` to `Option<f64>` to retain JavaScript
integer rounding and overflow. Direct Rust consumers use literals such as `radius: Some(5.0)`.
Canonical point coordinates remain `f64` and now include the Number-coerced results of the selected
wildcard lexer, including NaN for nonnumeric tokens. Compatibility JSON retains integer radius
values where representable and projects nonfinite values as `null`.

C4 layout row counts use `f64`. `C4Relation::offset_x`, `offset_y`, and the matching
`C4RelLayout` fields use `Option<C4RelationOffset>`, with `Number(f64)` for converted offset slots
and `String(String)` for named offsets supplied in color slots. Direct producers must preserve
that distinction because the SVG renderer applies JavaScript `parseInt(String(offset))` before
positioning relation labels. Use `Some(C4RelationOffset::Number(10.0))` for an ordinary numeric
offset. Journey infinity scores remain signed numeric values and no longer set `scoreIsNaN`;
canonical JSON represents them as the exact strings `"Infinity"` and `"-Infinity"` so that
model round trips retain their face geometry and expression. The canonical decoder accepts
numbers and those two strings. Compatibility JSON represents infinities as `null`, and terminal
text retains `Infinity` or `-Infinity`.

`merman_model::families::block::BlockEdge` adds `thickness: BlockEdgeThickness`
and `pattern: BlockEdgePattern`. Direct struct literals initialize them with
`BlockEdgeThickness::Normal` and `BlockEdgePattern::Solid`, or use
`..Default::default()`. Their Serde defaults preserve older inputs. Mermaid
compatibility edge records now include the upstream `thickness` and `pattern`
strings, and SVG consumes those semantics together with both endpoint markers.

`BlockNode::columns`, `width_in_columns`, and `width` change from `Option<i64>` to
`Option<f64>`. Use literals such as `columns: Some(3.0)` and preserve authored zero spans.
Counts use binary64 rounding, including values beyond `i64`; column counts do not allocate
empty slots. Only `space` materializes placeholders, with a cumulative 200,000-item parser
limit. Zero and nonfinite column counts fail when a nonempty composite reaches layout,
matching the selected upstream stage.
Nonfinite layout bounds still fail the shared finite-root SVG admission contract instead of
emitting a browser-dependent invalid viewBox.

`PacketBlock::start`, `end`, and `bits`, together with `PacketBlockLayout::start` and `end`,
use `f64`. `DEFAULT_PACKET_BITS_PER_ROW`, `validate_packet_bits_per_row`, and
`PacketDiagramLayout::bits_per_row` also use `f64`; row-width configuration accepts finite
numbers at least `1.0`, including fractions. Preserve the DB's `bits` value separately from
the inclusive endpoint width used for geometry. Row expansion retains its existing item
limit and rejects Number arithmetic that cannot advance the split.
`PacketBitsPerRowError::value` also uses `f64`; the error retains `PartialEq` and no longer
implements `Eq`.
Fractional splitting can produce a short fragment with `end < start` and negative retained
`bits`; use `end - start + 1.0` for its actual width rather than imposing integer interval rules.

`RadarOptions::ticks` and `min` use `f64`, `max` uses `Option<f64>`, and
`RadarCurve::entries` uses `Vec<f64>` instead of JSON values. Construct numeric values
directly, for example `ticks: 5.0`, `max: Some(10.0)`, and `entries: vec![2.0, 4.0]`.
The DB applies the final authored ticks value before enforcing its upper limit of 32.

`WardleyAnnotation::number` and `WardleyAnnotationLayout::number` change from `u64` to
`f64`. Use `number: 1.0` in direct literals. Annotation labels use JavaScript Number text,
and the annotation box preserves authored order when rounded or overflowing numbers tie.
`WardleyNode::label_offset_x` and `label_offset_y` use `Option<f64>` instead of
`Option<i64>`. Use values such as `Some(-40.0)`. Component and pipeline-child offsets retain
Number rounding, signed zero, and overflow; their editor ranges include an authored minus sign
and intervening whitespace. SVG label coordinates use JavaScript Number text, including
nonfinite values, while the map's root viewport retains its ordinary finite dimensions.

These Block, Packet, Radar, Wardley, Quadrant, C4 layout, and XY Chart canonical numeric fields
retain finite integer JSON shapes where representable, preserve negative zero, and encode nonfinite
values as exactly `"Infinity"`, `"-Infinity"`, or `"NaN"`. Canonical decoders accept numbers and those strings;
optional fields additionally accept `null` as absence. Compatibility JSON uses numeric
values and projects nonfinite values as `null`, following `JSON.stringify`. Source grammars
retain their own numeric token rules; the canonical strings do not introduce source syntax.

C4 relation offsets retain their existing finite-number and literal-string JSON forms. Nonfinite
numeric offsets use `{"number":"NaN"}`, `{"number":"Infinity"}`, or `{"number":"-Infinity"}`.
This keeps a numeric NaN distinct from the authored string `"NaN"`, which the renderer coerces
differently. Compatibility output continues to project nonfinite numeric offsets as `null` and
preserves literal strings. Regenerate stored canonical data from Mermaid source if an earlier
export erased a nonfinite value as `null`.

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

Kanban's inline header nodes follow the selected reserved-keyword boundary. Body identifiers
matching the standalone `kanban` token now reject, while `kanbanX` and `kanban_id` remain valid.
Numeric tickets, icons and assignees use JavaScript Number text; quote identifiers when their
exact authored spelling or integer precision must survive numeric conversion. Pie `showData`
legends retain their Number precision and use the same text for measurement and SVG output.

Venn accepts complete adjacent statements without requiring a physical newline. Numbers retain
their lexical endpoint, so `set A:1set B` declares both sets. Quoted and bracketed payloads remain
whole, indentation retains its selected text behavior, and recovered editor facts preserve
completed and following statements with their original source ranges.

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
