# Theme PR Addition Necessity Review — October 10, 2026

This review accounts for the entire frozen PR diff by module and distinguishes a necessary
responsibility from a minimal implementation. It finds additional debt beyond the first narrow
review. It does not certify every added line as necessary or exclude all undiscovered problems.
The [cleanup plan](../plans/2026-10-10-1535-refactor-theme-addition-necessity-plan.md) owns further
investigation and implementation; this report changes no production code.

## Counting and coverage

The comparison is main `2928265122d5a3d35e239e3ddbce885c7e342c59` against
branch `8664ff06ea234da73c59de99fb12a76034e301d3` using Git's three-dot diff,
rename detection and NUL-delimited numstat. Main is an ancestor of the frozen head.
Nine renamed paths are assigned to their destination module; a no-renames diagnostic may
show additional moved lines and must not replace the primary denominator.

- Added text: **421,112 lines**; deleted text: **40,358**; net: **380,754**.
- Changed paths: **1,683**, including **104 binary paths**. Binary content is assigned an owner
  but contributes zero to numstat text lines; image and gzip bytes are not production LOC.
- Primary partition: **107 modules**, every path assigned once. Diagram-family modules combine
  their top-level entry, semantic/preparation directory and corresponding SVG writer. For example,
  `render/flowchart` includes the shared Agentflow/Swimlane writers inside Flowchart's SVG directory;
  `render/swimlane` contains only its separate semantic/preparation paths.
- Counts include inline tests, independent tests, examples, generated code and legal projections.
  The six reviewers' overlapping scope totals are not added together.

An independent test-path view identifies **111,602 added lines (26.50%)**, including Rust tests,
Web specs and script tests, but excluding inline test bodies. A separate conservative Rust-only
pass recognizes 105,987 additions in readable independent `.rs` test paths plus 62,941 in final
top-level test modules: **at least 168,928 lines (40.11%)**. These views overlap the module table;
they are not extra additions or exact counts of all tests. Renamed-path syntax and nested/test-only
helpers make that Rust-only count a lower bound. Tests do not ordinarily enter release binaries.

**Assessment vocabulary:** Retain responsibility means a real consumer and distinct contract were
identified, not that every helper/test is minimal. Mixed means necessary responsibility and known
or plausible simplification coexist. Unresolved means the actual need or safe replacement remains
to be traced or tested. Retain history/data is an explicit archival/provenance decision rather than
a claim that the material is required by production rendering.

Each assigned module was checked against its changed-file inventory, entrances, owners and
representative diffs. Deep reads concentrate on the major additions and candidate call chains.
This is not a line-by-line proof of 421,112 additions. In particular, fallback cascade, all Flowchart
shape branches, graph-label details, all test cases, and every release-script branch remain outside
exhaustive review. No tests, builds or runtime measurements were run for this assessment.

## Disjoint module accounting

`render/` abbreviates `crates/merman-render/` with the ownership partition described above.
Each assessment refers to the current responsibility; details and uncertainties follow the table.

| Module | Added | Deleted | Added share | Paths / binary | Assessment and current responsibility |
| --- | ---: | ---: | ---: | ---: | --- |
| `render/integration-tests` | 70,351 | 3,011 | 16.71% | 71 / 3 | **Retain responsibility.** Real SVG/source/geometry/filter/native/support observations, not merely plan assertions; duplicate fixture helpers remain candidates. |
| `render/diagram_theme` | 32,932 | 0 | 7.82% | 33 / 0 | **Mixed.** Versioned compile/materialize/support and preset owners; obsolete route state and unavailable font success model remain. |
| `render/flowchart` | 31,742 | 3,691 | 7.54% | 99 / 0 | **Mixed.** Prepared source/typed paint, text/math geometry, markers and effects; demonstrated bookkeeping duplication; shape details not exhaustively reviewed. |
| `render/family` | 19,930 | 3,748 | 4.73% | 8 / 0 | **Mixed.** Operation preparation, current terminal evidence and diagnostics; generic historical cutover wiring remains; includes 12,025-line test module. |
| `render/sequence` | 14,911 | 1,515 | 3.54% | 49 / 0 | **Mixed.** Role-local styles, measured text/math and actual line/effect receipts; duplicate text/shape effect resolution; includes moved geometry code. |
| `crates/merman-core` | 14,251 | 2,515 | 3.38% | 36 / 0 | **Mixed.** Pinned Mermaid input derivation/provenance and bounded parsing; Treemap hand-written JSON size estimator needs a bounded replacement decision. |
| `docs/knowledge/engineering` | 14,216 | 0 | 3.38% | 132 / 0 | **Retain history; simplify candidates.** Dated architectural/verification evidence; remove only orphan completed journals with current conclusions elsewhere, per ARCHIVE policy. |
| `render/state` | 13,164 | 1,003 | 3.13% | 19 / 0 | **Mixed.** Unified style plan and real occurrence proof; separate final DOM/CSS observer is a bounded simplification trial. |
| `crates/merman` | 11,912 | 1,216 | 2.83% | 56 / 0 | **Mixed.** Rust authoring/preset/render/export consumers; two sealed-document construction paths repeat identity/admission work. |
| `crates/merman-theme-fixtures` | 10,623 | 0 | 2.52% | 24 / 0 | **Retirement candidate.** Rust catalog/parser has only self-test consumers; static scenes, references, licenses and R38/R39 mappings must survive. |
| `render/class` | 9,888 | 1,073 | 2.35% | 28 / 0 | **Retain responsibility.** Node/interface/member/relation/table/namespace terminals, typography and provenance; local minimality not certified. |
| `fixtures/themes` | 8,103 | 0 | 1.92% | 106 / 2 | **Retain data; retire historical execution.** Pinned upstream sources, current scenarios, provenance/license/reference translations; historical C6 lineage is not current admission. |
| `render/infrastructure` | 8,055 | 1,813 | 1.91% | 19 / 0 | **Mixed / unresolved.** Session, source CSS, resources, graph labels, fallback and private receipts; fallback/graph-label branches only partially deep-read. |
| `docs/performance` | 6,554 | 2 | 1.56% | 126 / 87 | **Retain evidence; minimality unresolved.** Matched-source cost reports and compressed receipts; old raw projections may consolidate after preserving identities/digests. |
| `crates/merman-bindings-core` | 6,494 | 1,162 | 1.54% | 25 / 0 | **Mixed.** Shared cross-language compiler/error/resource owner; same input repeats Value decode and selector validation. |
| `crates/merman-theme-acceptance` | 6,427 | 0 | 1.53% | 20 / 0 | **Mixed.** Current native SVG/PNG qualification and host-dependent scene proof are real owners; proof parser should remain bounded. |
| `render/text` | 6,195 | 26 | 1.47% | 9 / 0 | **Mixed / unresolved.** Projection, host admission and artifact identity; embedded catalog-dependent session path is unreachable, public host contract remains. |
| `render/gitgraph` | 5,970 | 460 | 1.42% | 9 / 0 | **Mixed.** Branch/commit/tag/palette and source ownership; active BestEffort static-fill fallback requires an explicit semantic decision. |
| `crates/merman-theme-contract` | 5,964 | 0 | 1.42% | 24 / 0 | **Retain responsibility.** Versioned portable definition/spec/recipe/support/errors and canonical sharing; wire/null/Clear contracts differ from runtime models. |
| `render/svg-common` | 5,911 | 3,528 | 1.40% | 23 / 0 | **Mixed.** Root/CSS/marker/output and resource-safe emission; common CSS/effect ownership and admission must retain output parity. |
| `crates/xtask` | 5,579 | 2,444 | 1.32% | 28 / 0 | **Mixed / unresolved.** Real Mermaid oracle/SVG comparator/Typst owners; family-specific normalization and budget implementations need explicit scope. |
| `render/block` | 5,283 | 543 | 1.25% | 13 / 0 | **Mixed.** Composite shells, classDef/source-owned channels, marker/label/background/font consumers; retired classification branches remain. |
| `render/er` | 5,084 | 855 | 1.21% | 7 / 0 | **Mixed.** Entity/row/relation/divider/text identity and source precedence; current per-occurrence proof is not aggregate typography. |
| `docs/plans` | 4,890 | 8 | 1.16% | 10 / 0 | **Retain decisions; correct claims.** Historical product/implementation decisions; current completion wording overstates generic proof retirement. |
| `render/svg-pipeline` | 4,729 | 358 | 1.12% | 18 / 0 | **Mixed / unresolved.** Final artifact/resource/text safety; same-byte validation replay needs identity-aware analysis; external mutations remain independent. |
| `render/xychart` | 4,286 | 273 | 1.02% | 6 / 0 | **Retain responsibility.** Bar/line series and axis/tick/title/legend geometry, identity and shadows; ordinal behavior is plot-specific. |
| `render/timeline` | 4,143 | 206 | 0.98% | 8 / 0 | **Mixed.** Event/section palette, shape stroke and typography with different class/layout/terminal owners. |
| `render/requirement` | 4,113 | 503 | 0.98% | 11 / 0 | **Mixed.** Node/divider/relation/title/text source owners; measured source size and browser cascade assertions are current contracts. |
| `crates/merman-export` | 3,939 | 207 | 0.94% | 5 / 0 | **Mixed / unresolved.** Actual PNG/JPEG/PDF filter/font admission; raw-SVG/resolved-filter comparison is necessary but simplification not proven. |
| `render/kanban` | 3,776 | 174 | 0.90% | 6 / 0 | **Mixed.** Card geometry and HTML visible-run inheritance; duplicate typography fact accumulation can reuse its existing owner. |
| `render/architecture` | 3,774 | 226 | 0.90% | 15 / 0 | **Mixed.** Service/group/edge/text geometry and receipts; keep independent Architecture Text integrity, remove retired classification consumers. |
| `render/treemap` | 3,773 | 410 | 0.90% | 5 / 0 | **Mixed.** Section/leaf/title font/paint/source owners; single-title bookkeeping overlaps Venn/ZenUML. |
| `fixtures/_verification` | 3,270 | 115 | 0.78% | 4 / 0 | **Retain responsibility.** Current core/xtask Mermaid theme-variable oracle; source-backed rather than a resurrected old baseline. |
| `playground` | 3,268 | 1,302 | 0.78% | 39 / 0 | **Mixed.** Preset/editor/recipe/share/viewport UI and browser tests; explicitly open-branch viewport share compatibility can retire. |
| `render/radar` | 2,778 | 137 | 0.66% | 7 / 0 | **Mixed.** Series/title/axis/text/font terminal owners; historical no-terminal CSS projection must remain explicitly non-Applied. |
| `platforms/web` | 2,697 | 574 | 0.64% | 38 / 0 | **Mixed.** Typed Web/runtime/export/package consumers; manual protocol projection is needed, but must not become another schema validator. |
| `render/gantt` | 2,672 | 119 | 0.63% | 5 / 0 | **Mixed.** State-dependent task bars, labels, grid/today markers, typography and actual CSS evidence. |
| `render/math` | 2,550 | 167 | 0.61% | 3 / 0 | **Retain responsibility.** Actual prepared math consumers and separate native/opaque projection evidence; not ordinary text shaping. |
| `render/journey` | 2,458 | 211 | 0.58% | 6 / 0 | **Mixed.** Tasks/actors/legend/text/effects have different real surfaces and applicability. |
| `render/c4` | 2,432 | 214 | 0.58% | 7 / 0 | **Mixed.** Boundary and element-type-specific paint/fonts; existing upstream layout vocabulary is not a retired bridge. |
| `render/mindmap` | 2,316 | 221 | 0.55% | 4 / 0 | **Mixed.** Branch palettes, source overrides, Neo/effect bounds and prepared typography; inverse default palette is Mermaid behavior. |
| `render/pie` | 2,301 | 190 | 0.55% | 5 / 0 | **Mixed.** Slice palette/stroke and title/legend/section text with independent source ownership. |
| `render/quadrantchart` | 2,120 | 138 | 0.50% | 5 / 0 | **Mixed.** Point/radius/stroke and axis/quadrant/title text have independent actual terminals. |
| `render/sankey` | 1,940 | 155 | 0.46% | 6 / 0 | **Mixed.** Node/link palette mapping and outlined/ordinary label inheritance; retained lookup has actual writer use. |
| `scripts` | 1,912 | 109 | 0.45% | 33 / 0 | **Mixed.** Real qualification CLI, external rustc consumer and release/security owners; some adjacent workflow updates need explicit PR scope. |
| `crates/merman-cli` | 1,693 | 338 | 0.40% | 29 / 0 | **Retain responsibility.** User preset/file/definition selection, bounded reads and shared binding compiler; includes tests and completion/man updates. |
| `render/packet` | 1,664 | 106 | 0.40% | 5 / 0 | **Mixed.** Bit/field/title roles and safe style admission; actual writer counts/signatures protect applicability. |
| `platforms/flutter` | 1,579 | 295 | 0.37% | 12 / 0 | **Retain responsibility; minimality unresolved.** Dart/FFI models and codec serve actual users; local language decoding cannot simply be removed. |
| `fixtures/block` | 1,573 | 1,573 | 0.37% | 98 / 0 | **Retain responsibility.** 1,573 added and removed snapshot lines: representation update, zero net growth. |
| `docs/rendering` | 1,548 | 239 | 0.37% | 8 / 0 | **Retain responsibility.** Current user API, preset support/limits, examples and migration guidance. |
| `render/venn` | 1,435 | 110 | 0.34% | 5 / 0 | **Mixed.** Set areas and actual text appearance; single-title rule bookkeeping is a local sharing candidate. |
| `render/tree_view` | 1,434 | 71 | 0.34% | 5 / 0 | **Mixed.** Node/icon paint, source overrides and paint outset/root containment. |
| `render/railroad` | 1,391 | 86 | 0.33% | 3 / 0 | **Mixed.** Grammar terminal/nonterminal/comment/rule-name text and viewport with actual traversal counts. |
| `render/ishikawa` | 1,290 | 163 | 0.31% | 6 / 0 | **Mixed.** Fishbone text-role font and source precedence; root layout config and stylesheet config differ intentionally. |
| `platforms/node` | 1,275 | 122 | 0.30% | 28 / 0 | **Retain responsibility.** Most growth is real smokes/examples/generated protocol; native/WASM transports share compilation. |
| `crates/merman-theme-authoring-fixtures` | 1,150 | 0 | 0.27% | 13 / 0 | **Retain responsibility.** Shared cross-language/facade/benchmark authoring golden vectors avoid duplicate expected protocols. |
| `distribution` | 1,082 | 330 | 0.26% | 30 / 0 | **Retain responsibility.** Typst package wrapper, actual compiler/smokes and generated license projections. |
| `platforms/python` | 957 | 92 | 0.23% | 11 / 0 | **Retain responsibility.** Python models/options/error mapping and actual authoring consumers. |
| `platforms/apple` | 943 | 65 | 0.22% | 10 / 0 | **Retain responsibility.** Swift API and fresh-process recipe examples/smokes/generated resource contract. |
| `render/wardley` | 878 | 145 | 0.21% | 3 / 0 | **Mixed.** Restores inherited fonts after upstream style clearing; role counts can share with Cynefin while CSS surfaces differ. |
| `docs/adr` | 788 | 43 | 0.19% | 10 / 0 | **Retain decisions.** Versioned authoring/ownership decisions and stable historical links. |
| `render/eventmodeling` | 787 | 44 | 0.19% | 5 / 0 | **Mixed.** Prepared source CSS and actual text roles; browser cascade helper is a real test consumer. |
| `render/cynefin` | 779 | 61 | 0.18% | 3 / 0 | **Mixed.** Root/inherited/CSS-variable font surfaces and visible-role counts; narrow sharing with Wardley is possible. |
| `platforms/android` | 634 | 86 | 0.15% | 13 / 0 | **Retain responsibility.** Kotlin/JNI public adapter and real consumers, with shared Rust compilation. |
| `render/zenuml` | 618 | 53 | 0.15% | 3 / 0 | **Mixed.** Bounded actual frame-title fill; unsupported siblings remain residual; single-title bookkeeping overlaps two families. |
| `render/usecase` | 567 | 176 | 0.13% | 8 / 0 | **Retain responsibility.** Prepared actor/usecase/boundary/note appearance and self-loop styling; real source/default consumers. |
| `render/error` | 564 | 61 | 0.13% | 2 / 0 | **Retain responsibility.** Actual fallback/error output, typography and CSS binding; not a user-selectable new theme engine. |
| `docs/alignment` | 546 | 14 | 0.13% | 10 / 0 | **Retain responsibility.** Current upstream/source/parity ownership and selected diagnostics. |
| `render/info` | 540 | 23 | 0.13% | 2 / 0 | **Retain responsibility.** Small current inherited-font/CSS/visible-text and viewport implementation. |
| `crates/merman-wasm` | 535 | 70 | 0.13% | 3 / 0 | **Retain responsibility.** Web compile/export/support and early raw admission; decode cost not assumed free. |
| `crates/merman-typst-plugin` | 524 | 25 | 0.12% | 4 / 0 | **Retain responsibility.** Real Typst JSON/preset/options and error/resource surface. |
| `docs/release` | 497 | 42 | 0.12% | 10 / 0 | **Retain responsibility.** Artifact budgets/package/native capability and release contract documentation. |
| `crates/merman-ffi` | 453 | 114 | 0.11% | 7 / 0 | **Retain responsibility.** Actual C ABI/header/descriptor/consumer alignment using shared bindings core. |
| `tools` | 451 | 110 | 0.11% | 7 / 0 | **Mixed.** Real Chromium cascade test helpers and showcase regeneration; PDF probe is historical reproduction, not a new release gate. |
| `docs/research` | 423 | 0 | 0.10% | 1 / 0 | **Retain history.** Dated interface reference analysis; not an executable gate. |
| `.github` | 405 | 60 | 0.10% | 6 / 0 | **Mixed.** Current cross-platform/private/optimized/public-boundary delivery owners; non-theme workflow growth is adjacent scope. |
| `crates/merman-node` | 249 | 41 | 0.06% | 4 / 0 | **Retain responsibility.** N-API/wire adapter; includes a package lockfile, not only executable Rust. |
| `crates/merman-uniffi` | 239 | 60 | 0.06% | 6 / 0 | **Retain responsibility.** Swift/Python transport, metadata and resource contracts. |
| `capabilities` | 222 | 42 | 0.05% | 6 / 0 | **Retain responsibility.** Actual artifact/API/feature descriptors consumed by packages; distinct from per-document theme evidence. |
| `crates/merman-analysis` | 219 | 15 | 0.05% | 4 / 0 | **Retain responsibility.** Resource-error diagnostics consumed by editor/LSP; no fabricated source positions. |
| `docs/bindings` | 208 | 108 | 0.05% | 7 / 0 | **Retain responsibility.** Actual language authoring/recipe/resource interfaces and examples. |
| `fixtures/other` | 201 | 183 | 0.05% | 20 / 0 | **Retain responsibility.** Selected current golden output updates; provenance and owner corpus remain required. |
| `crates/merman-rustdoc` | 190 | 17 | 0.05% | 5 / 0 | **Retain responsibility.** Real rustdoc rendering/options consumer and regression/docs updates. |
| `render/swimlane` | 171 | 69 | 0.04% | 3 / 0 | **Retain responsibility.** Reuses Flowchart preparation and source-bound label measurement rather than introducing another compiler. |
| `crates/merman-android-jni` | 107 | 29 | 0.03% | 2 / 0 | **Retain responsibility.** JNI transport and errors using shared operations; not another theme compiler. |
| `crates/merman-doc` | 107 | 25 | 0.03% | 2 / 0 | **Retain responsibility; adjacent scope.** Current Markdown input-complexity fix and tests, not core theme functionality. |
| `Cargo.lock` | 95 | 23 | 0.02% | 1 / 0 | **Retain generated closure.** Actual dependency graph/version changes; not executable source. |
| `docs/assets` | 71 | 2 | 0.02% | 22 / 12 | **Retain user examples.** Theme showcase images/SVGs; binary images do not contribute source-line counts. |
| `docs/development` | 71 | 13 | 0.02% | 1 / 0 | **Retain responsibility.** Current CI owners and retirement guide corrections. |
| `THIRD_PARTY_LICENSES` | 60 | 108 | 0.01% | 1 / 0 | **Retain generated attribution.** Legal projection of real dependency changes; net decrease. |
| `render/package` | 44 | 225 | 0.01% | 4 / 0 | **Retain responsibility.** Renderer dependencies/features/metadata; old visual reader deletion contributes net reduction. |
| `fuzz` | 41 | 6 | 0.01% | 2 / 0 | **Retain responsibility.** Mostly lockfile repair; current fuzz target alignment, not a new large validation framework. |
| `docs/workstreams` | 25 | 12 | 0.01% | 2 / 0 | **Retain bounded history.** Existing workstream summaries with current/historical scope. |
| `README.md` | 20 | 1 | 0.00% | 1 / 0 | **Retain user guidance.** Theme entry points and product discovery. |
| `crates/merman-ascii` | 19 | 5 | 0.00% | 2 / 0 | **Retain responsibility.** Small README/dev-test updates; does not bring the SVG theme compiler into ASCII runtime. |
| `docs/releasing` | 18 | 16 | 0.00% | 3 / 0 | **Retain responsibility.** Release operator contract corrections; no release performed. |
| `CHANGELOG.md` | 12 | 0 | 0.00% | 1 / 0 | **Retain user guidance.** User-facing change record. |
| `Cargo.toml` | 9 | 0 | 0.00% | 1 / 0 | **Mixed.** Runtime/dev/workspace dependencies; remove obsolete catalog member only with its owner. |
| `docs/security` | 9 | 3 | 0.00% | 3 / 0 | **Retain responsibility.** Current input/resource safety guidance. |
| `.agents` | 6 | 0 | 0.00% | 1 / 0 | **Retain contributor guidance.** Project skill ownership update; not a runtime dependency. |
| `docs/ARCHIVE.md` | 5 | 1 | 0.00% | 1 / 0 | **Retain responsibility.** Explicit policy for historical decisions and journal deletion. |
| `docs/FEATURES.md` | 5 | 5 | 0.00% | 1 / 0 | **Retain responsibility.** Feature/profile documentation with no net text growth. |
| `contracts` | 4 | 1 | 0.00% | 1 / 0 | **Retain responsibility.** ABI surface/version alignment for real consumers. |
| `crates/merman-lsp` | 3 | 9 | 0.00% | 3 / 0 | **Retain responsibility.** Diagnostic/API projection with net deletion. |
| `.gitattributes` | 1 | 0 | 0.00% | 1 / 0 | **Retain repository metadata.** Binary/generated diff handling metadata. |
| `crates/merman-fixture-render-context` | 1 | 0 | 0.00% | 1 / 0 | **Retain responsibility.** Existing development fixture integration; one added line, not a runtime subsystem. |
| `nix` | 1 | 0 | 0.00% | 1 / 0 | **Retain build metadata.** Existing package/build closure alignment. |
| **Total** | **421,112** | **40,358** | **100%** | **1,683 / 104** | Every frozen path assigned once; no unassigned module. |

## Confirmed debt and bounded candidates

### Generic historical cutover execution and retired runtime state

The current matrix test proves its LegacyCompatibility route inventory empty, yet
`diagram_theme/family_mechanism_matrix.rs` still maintains that enum, old paint/variant fallback
dictionaries and writer-missing exclusions. `resolved.rs` filters legacy routes and many family
consumers still ignore that state. The tail-family scan found 50 such references across 23 families;
they are not independent new user features.

The private audit still derives historical replacement projections from the current matrix.
`theme_route_cutover.rs` and `family.rs` collect facts and seal route receipts against both public
and native artifact hashes. The October 9 plan explicitly prohibited an executable historical oracle
and generating historical rows from the current matrix. Its broad U5 completion statement therefore
needs correction even though the old visual readers and separate authorization gates were removed.

Candidate windows at the frozen head are approximately 1,521 lines of generic cutover model/tests
after the separate Architecture Text section, 445 lines of historical audit projection derivation,
and 2,449 lines of matrix historical projection tests: **about 4,415 lines (1.05% of PR additions)**.
The related three full files total 8,359 additions (1.99%), but their retained current invariants and
Architecture Text proof must not be counted as removable. Another roughly 374-line matrix window
contains the old fallback/exclusion dictionaries; its whole 2,699-line file is not obsolete.

Remove the generic historical chain, migrate the still-useful cross-artifact non-replay assertion
to current evidence, and reduce production classification to typed/Unsupported only after domain
and support-descriptor equivalence. Preserve Mermaid input compatibility; it is a different owner.

### Self-consuming private fixture catalog

`crates/merman-theme-fixtures` contributes **10,623 lines (2.52%)**. No external Rust consumer of
its catalog API was found; it is a workspace member and its own tests exercise its Rust loaders.
Current qualification consumes source scenes, not the catalog API. Its C6 lineage parser/tests
are only a subset: **1,914 lines (0.45%)**, not another amount to add to 10,623.

The crate additionally interprets CSS/directives/frontmatter in `source_compatibility.rs`
(663 additions) and `theme_input.rs` (1,360). This is historical fixed-source evidence infrastructure,
not a supported TS/CSS import language. The original R38/R39 require independent theme-fixture
ownership and reference/mechanism classifications, not eternal retention of this parser.

Static content is widely consumed: renderer/facade tests and benches, qualification, Playground
browser specs, Node's authoring and installed smokes, Apple and Python examples, and a PDF debug
probe all read the public Cyberpunk scenes. Preserve exact bytes and migrate those callers first.
Retain the 24 reference themes, 18 mechanism translations, pinned source paths/revisions/hashes,
supported/residual mappings, font provenance and MIT/OFL/NOTICE records. Removing only the
self-consuming execution layer must not discard these records or current native qualification.

The generic projection windows plus this entire candidate crate cover about **15,038 lines
(3.57%)** before replacements, retained scene data and other live contracts are deducted. This
is a review envelope, not a promised deletion amount, a reduction quota or evidence of binary savings.

### Unreachable embedded-font success state

In `diagram_theme/assets.rs`, the sole successful catalog constructor is `system_fonts()`;
it creates empty asset/face/alias/named maps. Embedded catalog compilation ends with
`EmbeddedFontsUnavailable` after its current input/resource checks. `FontAsset` has no successful
production construction path. The normal session's prepared-text setup exits before initialization
when that catalog has no assets.

The private asset/catalog admission receipts and resolved-family maps therefore represent an
unavailable success mode, while related prepared-text code still treats it as a possible session
lane. The catalog file contributes 768 lines; its internal state/validation region is a roughly
273-line candidate window. `text/prepared.rs` contributes 4,979 lines, **but this is not a deletion
estimate**: readable wire types and public host/export contracts were deliberately retained by the
September 21 retirement. Trace those construction and consumer boundaries before simplifying.

Prepared-text terminal sealing also repeats some range-order/extent checks of its immutable
admitted ledger and collects two iterator sequences into vectors. About 45 lines are a local replay
candidate in the 290-line `terminal_receipt.rs`. Font source, actual faces, provenance and exact-artifact
binding are different contracts and remain required.

### Local algorithm duplication

- Flowchart has four identical incomplete-facet bookkeeping routines and repeated outcome
  accounting for scalar/padding/dasharray/typography. Roughly 205 lines are the initial duplicate
  algorithm window; final shared replacement size is not yet known. Keep first-residual ordering,
  source precedence, emission facts and differing capability/reason semantics.
- Sequence shape and text effect preparation both resolve None/Clear/Rule/Binding into the
  same shadow result. About 80 lines span the repeated state machines; text and shape receipts
  must still prove different actual terminals.
- Kanban reconstructs six scalar typography facts and repeats its existing fact accumulator.
  About 65 lines and one writer call are involved; expected and actual facts remain independent.
- Treemap/Venn/ZenUML single-title static-fill plans share 225/244 lines after family-name
  normalization; Cynefin/Wardley font-role accounting shares 236. These are similarity measures,
  not additional deletion counts. Share only their concrete rule/fact algorithms and retain family
  title presence, source ownership, CSS surfaces and writer sealing.
- Facade `render/document.rs` repeats report/evidence/portability/public-and-native digest/target
  admission in two finalization paths. A single internal finalization owner can remove duplication;
  public digest and native receipt contracts are real and cannot simply disappear.

### Boundary and heavy-observer candidates

- Bindings `theme.rs` and `common.rs` parse/validate a selector union more than once in the
  same path, including custom Deserialize through Value. Keep bounded raw preflight before
  allocation and strict schema/duplicate-field checks; consume an admitted input once within
  that boundary. The two files add 2,245 lines including tests, not 2,245 removable lines.
- Playground's explicitly open-branch-only `renderViewportMode` decoder compatibility is a
  direct retirement candidate. Preserve published s2, current s3 and default-to-auto semantics.
- State observes completed SVG through its own DOM/range/ancestor/CSS-signature layer.
  The 1,455-line file includes roughly 1,053 implementation lines. It proves actual occurrence
  values rather than plan intent. A replacement must preserve HTML overrides, special shapes,
  filtering, missing/duplicate writers and late artifact binding; no safe deletion or speedup is
  established yet.
- Private `svg_artifact_receipts.rs` adds 958 lines and maintains a bounded CSS observation
  parser. Current Sequence/C6 consumers need exact writer declarations, not computed-browser
  style proof. Reuse a tokenizer or narrow actual-emission ledger only with explicit unknown
  syntax policy; do not turn the observer into a CSS engine or fabricate evidence from a plan.
- Exporter's native filter receipt adds 1,811 lines, about 1,020 implementation and 790 tests.
  Raw SVG and usvg agree only after units/regions/stages/references are checked. Simplification
  is unresolved; another parsed representation is not automatically redundant.
- SVG pipeline checks after some passes may replay validation on unchanged borrowed bytes.
  Only identical validated artifact identity and metrics can justify reuse. Changed bytes,
  prepared-math admission and external postprocessors remain separate trust boundaries.
- Core Treemap's roughly 400-line JSON-size estimator manually models nesting, string escaping
  and output shape. It protects allocation amplification and cancellation. A counting serializer
  after creating the expanded Value would not replace its pre-allocation contract.
- xtask's family-specific CSS/paint normalization is bounded and has real negative cases, but
  its owner/evidence must remain family-local. Do not add semantic normalization just to make
  fixtures pass. No false-equivalence example was confirmed in this review.

GitGraph's historical static-fill BestEffort fallback still executes and affects output. Unlike
empty legacy classification, it cannot be deleted as dead code. Characterize strict/BestEffort,
ordinal/static and source-owned outcomes before deciding whether it is intentional current behavior.

## Why major additions remain justified at responsibility level

Core's Mermaid derivation preserves pinned `||`/`??`/unconditional assignment and explicit source
ownership. It is input compatibility, not the removed old SVG reader. The contract crate supplies
versioned recipe exchange independent of a renderer; concise definition and complete-spec are
different user workflows. Canonical identities, unknown/null/Clear semantics and closed schema
checks are needed by real language consumers.

Family preparation makes measurement, layout and emission use the same resolved style. Different
SVG/HTML/source-owned terminals need concrete consumers; static support discovery cannot prove
that a particular document actually applied a theme. Applied/residual/not-applicable and native
filter/text/math facts protect different output guarantees. Cross-language wrappers and consumer
smokes validate packaging and transport boundaries rather than implement separate Rust compilers.

Large test files observe real terminal effects, geometry, source overrides and unsupported portions.
For example, one applied fill does not prove stroke/font/effect, and equal values can have different
explicit owners. `family/tests/mod.rs` also protects operation, custom parser, cancellation, resource,
layout and output contracts outside theme appearance. Responsibility-based splitting and small
shared scene/assertion helpers are preferable to arbitrary assertion deletion.

Not every addition is theme runtime: adjacent Markdown complexity fixes, error propagation,
release-workflow corrections, generated ABI/legal projections, moved geometry and snapshot updates
must be described separately in the PR. New dependencies also have real cost: the SVG closure
adds contract/canonicalization/hash owners, while other dependencies remain native or test-only.
Removing one optional SHA edge does not mean the complete theme feature added no dependencies.

Docs add 29,874 lines (7.09%): knowledge history 14,216, performance/evidence 6,554 and plans
4,890 dominate. `docs/ARCHIVE.md` already permits removal of completed orphan journals when their
conclusion has a current owner, while preserving accepted decisions and useful dated evidence.
Current guides and showcase limitations serve users. Archive/consolidate only by that policy,
not by treating all dated material as production requirements or deleting provenance to shrink a diff.

## Focused test review and planned cleanup

A second read-only review of test standards and behavioral contracts found that broad coverage
is warranted but does not justify all current test implementation. The conservative frozen
Rust-test addition count remains 168,928 lines (40.11%); this overlaps the primary module table
and must not be added to it. No tests or workload measurements were run for this review.

Concrete findings, with source lines referring to frozen HEAD:

| Evidence | Finding and disposition |
| --- | --- |
| `crates/merman-render/src/diagram_theme/family_mechanism_matrix/tests.rs:2886,4710` | Approximately 1,813-line historical projection list and 482-line qualified-route test, within the already-counted roughly 2,449-line migration window; retire with U2, not a new savings estimate |
| `crates/merman-render/src/family/tests/mod.rs:11402,11455` | Required identity/order tests also pin a private index collection; remove storage-layout assertions while retaining deduplication, empty/many-rule coverage and unapplied behavior |
| `crates/merman-render/tests/sequence_svg_test.rs:2928,3506` | Approximately 602-line combined window repeats execution/evidence mechanics; share local helpers, preserve explicit-source and derived-source cases |
| `crates/merman-render/tests/sequence_svg_test.rs:3716` | Runtime-config-derived expected colors plus global SVG contains checks are not independent terminal verification; use explicit expected values and exact terminal properties |
| `crates/merman-render/tests/state_svg_test.rs:2097,448` | Already-parameterized special-terminal test is roughly 447 lines; improve responsibility boundaries and avoid discarded layout JSON round-trips in SVG-only helpers |
| `crates/merman-render/tests/gitgraph_svg_test.rs:3437,3748` | Unsupported-font tests repeat execution; retain fill-only/stroke/combined patches and separate Strict/BestEffort operations |
| `crates/merman-render/src/diagram_theme/canonical.rs:1205,1465,1972` | Field-sensitive recipe identity is necessary; repeated complete left/right builders can reuse explicit baselines without dropping fields or copying the encoder |

Current file size is not PR addition size: Sequence has 9,247 lines/137 test declarations,
State 6,448/72, family tests 12,025/202, and matrix tests 6,033/90. A declaration can loop over
many cases, so function count is not coverage count. Source windows are approximate static
spans, not measured redundant lines or expected deletions.

Keep route admission distinct from actual emission, SVG attributes distinct from native pixel
negative controls, and transport/browser behavior distinct from inner rendering. The plan's U9
records coverage ownership, independent expectations, retirement prerequisites and scoped checks.
It precedes U8 final reconciliation while leaving existing unit IDs stable. There is no arbitrary
reduction target, no evidence for promising a halving, and no claim that this audit is exhaustive.

## Review limitations and implementation handoff

The 107-module table provides complete accounting and responsibility-level assessment. It does
not provide a complete proof of minimal code, dynamic reachability in every feature combination,
or exhaustively unique tests. The deeper review found debt beyond the first five candidates, so
there is specifically no claim that those findings are exhaustive.

U1 of the linked plan deepens uncertain fallback, graph-label, shape, prepared-text, native-filter,
comparator and release-script surfaces and resolves their disposition. Later units remove only
demonstrated debt and preserve current boundaries. The plan requires actual final per-module
counts, dependency closure and canonical artifact measurements rather than source-size promises.
These planning documents are excluded from the frozen PR denominator and will themselves add
a small documentation diff; they do not retroactively alter the audited source identity.
