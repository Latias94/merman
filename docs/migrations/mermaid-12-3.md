# Mermaid 12.3 development migration

Current source development selects Mermaid `12.3.0` at
`2086904e76c692331e34a45d7cc3bc30860d040e`. Published `0.10.0-alpha.1`
artifacts retain their Mermaid `12.2.0` identity. Build wrappers and native/WASM
artifacts from the same source revision when using these development changes.

## Selected behavior graph

The [reference bundle](../../tools/upstreams/MERMAID_REFERENCE_BUNDLE.json),
[selection receipt](../../tools/upstreams/MERMAID_SELECTION_DECISION.json) and
[source lock](../../tools/upstreams/REPOS.lock.json) own the exact identities.
Both reference workspaces select `elkjs 0.12.0`, Cytoscape `3.34.4`,
`@braintree/sanitize-url 7.1.2`, DOMPurify `3.4.17`, and reference CLI `12.2.0`.
Parser `2.0.1`, Tidy Tree `1.0.1`, and ZenUML plugin `1.0.1` keep their
independent source identities. Selected ZenUML Core remains `3.50.1`; the
Mermaid source workspace's older Core resolution does not replace it.

ELK uses the exact target workspace oracle, `0.12.0`. The investigated
latest-compatible `0.12.1` has separate build and behavior changes and has not
completed admission. It is not substituted implicitly. The
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

Struct-literal consumers of `FlowEdge` add `step: None` when no step is authored.
`FlowchartModel` carries `dropped_subgraph_members` for shared-member provenance;
ordinary builders can use its default empty collection. These new data use
defaulted, omitted-when-empty JSON fields where applicable. Author metadata
cannot set internal `laneRole`, `laneIndex`, or `drawnExtent` values.

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

## Verified reference evidence (2026-11-04)

The four verification catalogs and seven semantic-label fixture contracts bind
actual Mermaid 12.3 SVGs and their selected-source manifests. The collection
rendered 3,748 reference SVGs. The 37-family comparison selects a different
inventory: 3,740 rendered inputs, 27 explicit source/host skips and three DOM modes.
All modes pass the blocking policy, including 145 exact browser-measurement
entries covering 381 fixture/mode pairs and separate parser-diagnostic residuals.
No comparison tolerance or broad normalization was added.

Controlled replay of the full archived 12.2 lock graph and selected 12.3 runtime,
under Node `24.21.0`, Chrome `156.0.8078.47`, `zh-CN` and equal fonts/config/seed,
establishes byte continuity for 21 newly admitted measurement cases: 20 Sequence
fixtures and one Timeline fixture. Old N-API and current Rust SVGs also match for
those cases. Their complete signatures bind only the observed modes; browser,
locale and font causes were not individually isolated.

C4's five refreshed upstream geometry snapshots use controlled collection
evidence while all 28 local geometry snapshots remain unchanged. Six ELK local
signatures follow the actual `0.9.3` to `0.12.0` layout observations. Ten parser
hashes come from newly rendered SVGs; changing only their displayed version back
reproduces the old hashes. The converged Statecross and Sequence
`stress_br_in_messages_notes_011` receipts were deleted. Raw controlled replay
and signature reports remain under ignored `target/mermaid-12.3-c4-controlled/`.

All 46 ELK browser-measurement entries were recollected: five fixtures account for
78 changed raw routes, and three historical label-key corruptions were corrected.
All 78 literal SVG probes were remeasured without numeric changes. Both matrices
identify the current capture and retain old metadata in `historical_collection`.

Step's strict/sandbox marker attributes reuse shared DOMPurify DOM-value
admission. Actual DOMPurify `3.4.17` rejects both `stroke:DarkGray` and
`stroke:#123456` while preserving ordinary colors. Loose mode retains the raw
values; marker identity is unchanged. Browser tests verify timing, update and
document isolation, and reduced motion in Classic, Neo and Hand-drawn looks.

Final `verify --strict` passes: 14,026 Rust tests, architecture/doctest owners,
all 37 three-mode family comparisons, Web build/contracts/smoke, Playground
tests/build/browsers and 316 VS Code tests. Independent size, installed Node and
real Typst consumers pass with the original budgets. The
[completion record](../knowledge/engineering/verification/2026-11/2026-11-04T000111Z-round2-completion-b98b1ed2141546f3aefb98ba51840f11.md)
names exact artifacts, source identities, review closure and explicit limits.
The upgrade playbook's
[current transition](../release/MERMAID_UPGRADE_PLAYBOOK.md#current-transition-mermaid-1230)
describes the selected development graph.
