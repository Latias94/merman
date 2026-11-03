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

## Reference-contract review (2026-11-04)

The four verification catalogs and seven semantic-label fixture contracts now bind
the generated 12.3 SVGs. All 148 distinct fixture inputs and their old SVG hashes
were checked against Git base `84ef69b30f128d8a4f955e3aafb50f6378a4de5c`;
the new hashes match the selected-source manifests. All five deterministic upstream
root signatures are unchanged. Initial rebinding preserved entry identities, modes,
precision, 126 complete local browser signatures, 28 upstream/local label-geometry
snapshots, and ten local parser SVG hashes. The subsequent C4 review below refreshes
five upstream geometry snapshots while retaining all local geometry.

Final integrated validation remains with the main worker's freshly built
three-mode comparison and strict verification.
Controlled C4 replay is complete: both reviewed C4Dynamic fixtures produce
byte-identical raw SVGs with Mermaid 12.2 and 12.3 under Node `24.21.0`, Chrome
`156.0.8078.47`, `zh-CN`, the same font probe, seed, clock, viewport and config.
Rebuilding the trusted 12.2 lock graph reproduces archived bundle
`29d255aa0a6d1d69e6d7c56a67b64c6c89318e2acf079587ec85743c70b44a2e`
and the current 12.3 SVGs. The verified old N-API addon and current Rust renderer
also produce byte-identical complete local SVGs with deterministic measurement
and white capture. C4 and shared-text source is unchanged. The five upstream
geometry snapshots now bind the measured reference, including the first message
anchor `(667.948, 396.913)`, while all local snapshots remain unchanged. This is a
bounded residual of the new reference collection; browser, locale and font effects
were not individually isolated.
The other 23 upstream label-geometry snapshots retain their numeric SVG evidence;
Class and Requirement SVG changes are confined to XHTML inline styles. Twelve
existing browser receipts have changed upstream artifacts, including ELK routes
across the `0.9.3` to `0.12.0` engine transition. The main worker rebound six ELK
local signatures using Halley's controlled old N-API and current Rust evidence.
The Statecross case fully converged, and its obsolete receipt was removed before
the 21-entry admission below. The ten parser upstream SVGs change only the
displayed Mermaid version. The main worker rebound all ten local hashes from
actual generated `target/compare` SVGs. Reversing only the displayed version text
in those newly rendered local SVGs reproduces all ten old recorded byte hashes
exactly. That reverse-version check establishes continuity; the admitted new
hashes come from the actual new SVG bytes.

The completed follow-up admits 21 exact browser-measurement receipts: 20 Sequence
fixtures and one Timeline fixture, each limited to its observed `structure`,
`parity` and `parity-root` failures. The catalog grows from 125 to 146 entries;
the existing 125 entries remain unchanged, preserving the main worker's six local
signature rebindings and removal of the converged Statecross receipt. Every new
input/upstream hash comes from the actual artifacts, and every complete local
signature comes from the existing Rust `canonical_local_svg_signature` owner.
No comparison rules, tolerances or normalization change.

All 21 fixtures produce byte-identical 12.2/12.3 raw SVGs in the same controlled
environment described above, including replay of the complete archived lock graph.
The verified old N-API addon and current Rust output are also byte-identical for
every fixture. Per-fixture DOM review confirms unchanged IDs, label content,
text ownership and path commands; the new differences are line grouping and its
derived geometry. Existing presentation differences were checked against the old
artifacts and existing comparator. The 63 exact mode receipts bound these visible
browser-measurement residuals; they do not establish an implementation fix or
pixel-perfect parity, nor isolate a particular browser, locale or font cause.
Raw replay, DOM review and owner-signature reports remain under
`target/mermaid-12.3-c4-controlled/`, with admission evidence in `admission-21/`.
The final unfiltered comparison also found that Sequence
`stress_br_in_messages_notes_011` now matches in all three modes. Its stale
historical receipt was deleted, leaving 145 current catalog entries. A Step
hand-drawn fixture exposed a separate marker paint defect: strict/sandbox marker
attributes now reuse the shared DOMPurify DOM-value admission instead of a local
hex-color exception. Actual DOMPurify `3.4.17` in Chrome `156.0.8078.47` rejects
both `stroke:DarkGray` and `stroke:#123456` while preserving ordinary colors.
Loose mode retains the upstream raw values and marker identity is unchanged.
Herschel separately recollected all 46 ELK browser-measurement entries against
the selected runtime: five fixtures account for 78 changed raw routes, and three
historically corrupted label keys were corrected. All 78 literal SVG probes were
also remeasured, retaining their numeric values. Both
`flowchart-elk-browser-measurements.json` and
`flowchart-svg-terminal-measurements.json` now identify the current Mermaid 12.3
capture at the top level; their old provenance is archived under
`historical_collection`. This unit preserves that separately owned collection
without editing either matrix.

The upgrade playbook's [current 12.3 transition](../release/MERMAID_UPGRADE_PLAYBOOK.md#current-transition-mermaid-1230)
describes the selected development graph. Final integrated strict verification
remains pending at this handoff; these artifact checks do not claim that gate has
passed.
