# Public Theme Workflows Before Contract Freeze

Date: 2026-09-17. Source baseline: `405593f27`.
This is a source-backed interface audit, not installed-consumer or visual qualification. It supplements the [portfolio analysis](2026-09-16-theme-portfolio-and-application-boundaries.md) and [product boundary plan](../../../plans/2026-09-16-1618-refactor-theme-product-boundaries-plan.md).

## Findings

| User task | Current evidence | Gap before freeze |
| --- | --- | --- |
| Choose a preset for the current diagram | `merman-theme-contract/src/preset_catalog.rs` exposes availability and qualification; `ToolbarControls.tsx` presents preset IDs as names | Curated family design scope and availability explanations are missing from the public selection flow |
| Keep a theme while changing diagram family | The compiler already has family-scoped rules; the shared preset builder contains Mindmap and other family exceptions | Explain dedicated/base-only/unreviewed styling without changing selection or pretending a generic palette completes a family design |
| Adapt a preset to a brand | Bindings accept exactly one of preset/spec; ADR 0082 explicitly excludes preset-plus-patch in v1 | Measure the actual export/edit/import workflow before deciding whether a small authoring convenience is necessary |
| Share an exported theme file | Simple definitions and materialization results carry versions; complete specs and `PresetExportV1::CompleteSpec` lack a serialized spec version | Choose one existing envelope as the canonical durable exchange representation and make import/version validation explicit |
| Use an exported file in another application | CLI `--theme-file` consumes preset/spec selection, while preset export returns a different envelope | Export-to-file-to-import must not require knowledge of internal envelope extraction and reconstruction |
| Redistribute a custom recipe | Built-in catalog metadata includes license/attribution; custom definition/spec/export has no equivalent identity or design metadata | State whether optional metadata belongs in the existing exchange envelope or accompanying README/LICENSE; do not imply a recipe guarantees font rights or self-contained rendering |

Source owners: `crates/merman-theme-contract/src/{preset_catalog,authoring,materialized,spec,preset_export}.rs`; `crates/merman-bindings-core/src/theme.rs`; `crates/merman-render/src/diagram_theme/{presets/catalog,resolved,semantic}.rs`; `crates/merman-cli/src/cli.rs`; `playground/src/components/ToolbarControls.tsx`; `docs/adr/0082-versioned-theme-authoring-facade.md`.
The missing serialized version is a durability decision and usability gap, not evidence that the current v1 parser misparses input.

## User Concepts and Application Policy

Keep three concepts visible: the selected recipe, its design scope for this diagram, and the actual output result. Availability means compilation, designed scope is curated intent, and qualification is evidence for a particular recipe/scenario/target. None substitutes for another.

Apply the shared base plus the current family's scoped rules. Outside a dedicated scope, retain the selected recipe and explain base-only or unreviewed behavior. An unsupported requested effect remains an explicit residual or error under the existing policy. It must not trigger an invisible switch to another theme or disappear from reporting.

Do not add `fallbackPreset` to the compiler at this point. The existing base composition addresses missing family specialization without defining cross-preset merges, recursion or dark/light substitutions. A host may offer an explicit alternative selection. If real user journeys demonstrate the need for an automatic alternative, define its trigger, effective recipe identity, override precedence and reporting before adding the API.

Distinguish this policy from property fallback: Unspecified, Clear and transparent Value already have different semantics. Clear restores the family base according to existing ownership rules; transparent paint must not be filled back in. An unknown discovery classification is preserved and displayed conservatively; it is not a portable or recommended claim. Unsupported input schemas and unknown preset IDs fail explicitly rather than being treated as discovery extensions.

## Authoring and Distribution Decisions

Keep a simple creation path and one complete typed representation. Simple token/style definitions remain useful. Complex preset compilation and export must share the complete recipe, including family rules, canvas and effects. Do not expand the authoring DSL solely to mirror every internal field.

Before fixing the public shape, try copying Cyberpunk, changing two brand colors and one Class rule, then saving and importing it in a fresh process. If this requires navigating compiler-derived fields or rebuilding envelopes by hand, simplify the owning public seam. An SDK convenience should delegate to the same materialization and admission semantics rather than create a second patch engine.

Select a canonical exchange envelope from the existing formats. Define schema identification, unknown-version behavior, direct import, lossless round-trip expectations, and how optional identity/design metadata survives. The schema identity must be serialized, not inferred only from a Rust type name or operation endpoint. Keep unpublished theme protocols at v1; this audit does not authorize a package version bump or a new theme registry.

A shareable recipe can still depend on host fonts or a chosen backdrop. Document resource requirements and redistribution obligations. JSON plus README/LICENSE is an acceptable initial distribution mechanism; remote loading, package installation, registry resolution and automatic font downloads are outside this tranche.

## Required User Journeys

1. Select a preset, switch Flowchart/Class/XY, and retain the explicit selection while the design explanation changes.
2. Change two brand colors and one family rule; verify source ownership, Clear and transparent paint without replacing unrelated facets.
3. Export to a file, start a fresh process or another SDK, import the same file directly, and preserve the effective recipe. Include a complete canvas/effect recipe, not only a palette.
4. Run offline with missing host fonts and with embedded-font capability disabled; explain the actual dependency or error without silently changing the recipe.
5. Exercise old/unknown schema versions, unknown preset IDs, and unknown discovery/admission values under their distinct input versus output contracts.
6. Move from a small showcase to dense Class, more-than-three-series XY, and PNG/PDF; do not retain an unqualified full-support claim.

Implement these through existing CLI/SDK/Playground and shared transport tests. A new metadata field, README or Rust round-trip alone does not close a user journey. Define the sharing and selection contracts before U4/U9 migration; preserve the completed core migration rather than restarting it.

## Direct Recipe Exchange Implementation

Implemented after source baseline `bd56f6dab`, on 2026-09-17. The Findings table above records
its original audit baseline; the exchange envelope and direct-import rows have since advanced.
The unpublished `PresetExportV1` is replaced by `ThemeRecipeV1`, with a required serialized
`schema_version: 1`. Its closed `definition`/`complete_spec` variants preserve compact authoring
or the complete canvas/effect specification. No package or schema version was incremented.

Binding `options.theme`, CLI `--theme-file`, and Web's public theme selection type accept the
exported document directly. Rust callers have `DiagramThemeCompiler::compile_recipe`.
Existing exact preset/spec selections remain supported; mixing them with a recipe is rejected.
Raw input admission checks the recipe byte ceiling before envelope validation, retains duplicate
root fields for rejection, and reuses the existing definition JSON structural/version preflight
before normalizing JSON. Materialization and semantic compilation retain their existing owners.

Verification on this implementation:

- `CARGO_BUILD_JOBS=1 cargo nextest run -p merman-theme-contract -p merman-bindings-core --features merman-bindings-core/svg`: **331/331 passed**. Covers stored recipe rendering through one-shot and reusable engines, new-compiler fingerprint equivalence for EditorDark and Cyberpunk, structured canvas/ordered-effect serialization, invalid/missing versions, duplicate/escaped keys, depth limits, mixed selections, and encoded-byte error precedence.
- `CARGO_BUILD_JOBS=1 cargo nextest run -p merman-cli --features layout-elk --test cli_contract -E 'test(native_theme_file) | test(native_theme_definition)'`: **2/2 passed**, 35 unrelated tests filtered out. A fresh CLI process consumes a file containing the public preset export without envelope reconstruction; the existing raw-definition workflow also passes.
- Renderer preset and definition-admission tests: **14/14 passed**, 2,562 unrelated tests filtered out. Includes all ten catalog recipes, their complete-spec round trips and existing authoring structural admission.
- Web `npm run build:ts` (including contract type checks) and `node --test scripts/theme-authoring.test.mjs`: **3/3 passed**. These verify public types and mock transport forwarding, not installed WASM behavior.
- An independent API-contract review found a duplicate `complete_spec` normalization hole; the raw envelope probe and paired direct/options negative cases were corrected, then included in the passing 331-test run. The follow-up source review reported no remaining finding in this slice.

Logs: `/tmp/merman-u13-rust-green.log`, `/tmp/merman-u13-cli.log`,
`/tmp/merman-u13-web-build.log`, `/tmp/merman-u13-web-tests.log`,
`/tmp/merman-u13-renderer.log`.
The first three binding import tests failed before the implementation. The wire and TypeScript
negative checks also exposed the missing version/import contract before their implementations.

The real-WASM smoke now contains direct import/output-equivalence and unknown-version/mixed-input
cases, but a rebuilt installed WASM package was **not** exercised in this slice. This does not close
U13: curated family design metadata/selection UI, two-brand-colors-plus-one-family-rule usability,
missing-font workflows, and cross-installed-consumer distribution remain to be verified. No
qualification cells or public contract freeze are promoted by these results.

## Family Design Disclosure and Browser Selection

Implemented after source baseline `53af16c4a`, on 2026-09-17. The existing preset descriptor now
projects `family_designs` as sorted, unique logical-family/treatment pairs. Class, Flowchart,
Sequence and XY are explicitly `base_only` for the current ten shared-palette recipes. This
allows necessary family color/semantic adaptations; it does not promise complete Modern Mermaid
styling or readability in every scene. Other families are absent/unreviewed. No dedicated design
or qualification cell was promoted to make the catalog appear complete.

Web and Flutter retain unknown IDs, default an absent design array to empty, and reject malformed
or ambiguous arrays. The Playground maps parser variants using the loaded runtime's logical-family
catalog. Its theme menu presents presets first, explains the selected preset's scope for the
visible diagram, and retains unknown/unavailable saved selections for explicit replacement. It
does not infer design from `available` or `qualified_cells`, change the selection on family
switching, or add cross-preset fallback. During an update, the description follows the still-visible
previous diagram rather than claiming the new editor input has already been detected.

Verification:

- Contract/bindings nextest with SVG: **331/331 passed**, including the shared preset catalog golden and open-string Rust projection.
- Renderer preset tests: **13/13 passed**, 2,563 unrelated tests filtered out; recipe fingerprints and all ten complete-spec round trips remain unchanged.
- Web TypeScript build/contract checks and authoring/catalog tests: **13/13 passed**. New discovery tests first failed before implementation; unknown treatment, missing array, invalid/duplicate/unsorted entries, and defensive copying are covered.
- Rebuilt and assembled the **full** browser WASM package with one Cargo build job; input manifest fingerprint `77a4ee44e987`. Its real package smoke passed across 35 diagram entries, including shared catalog equality and the prior slice's saved-recipe import/output-equivalence cases. This closes that slice's rebuilt full-WASM gap; other package profiles were not rebuilt here.
- Production Playground build passed TypeScript, existing license checks, opaque-realm verification and the final artifact-graph check. The initial license failure was caused by installed `js-yaml 4.3.1` against lockfile `4.3.2`; a lockfile-based install fixed it without changing dependencies or rewriting the license report. The final build used the repository-selected npm 12.0.2.
- Playground coordinator/design unit tests: **34/34 passed**. Chromium against the production build: **2/2 passed**. The user flow retains Cyberpunk across Flowchart, Class, XY and Packet, shows base-only versus unreviewed appropriately, and retains an unknown saved preset as selected/unavailable. This verifies selection/disclosure, not reference visual reproduction or native export quality.
- Flutter `dart run tool/abi3_contract_test.dart` passed; focused Dart analysis reported no issues. These are projection/contract checks, not a rebuilt installed native package.
- Independent source review found no remaining API-contract issue in this slice. Screenshot inspection confirmed that the preset-first menu exposes the current scope and compact preset rows without hiding them below the Mermaid configuration choices.

The compact preset-array JSON grows from **2,534 to 4,654 bytes (+2,120 bytes)** relative to
`53af16c4a`. This measures only the serialized metadata array; it is not a binary-size or runtime
performance conclusion and does not change any artifact budget.

Logs are `/tmp/merman-u13-design-{rust,renderer,web-green,web-smoke,wasm,playground-build,playground-unit,browser}.log`.
The inspected screenshot is `target/bench/experiments/theme-design-53af16c4a/theme-design-menu.png`.
U13 remains open for actual small-brand-edit usability, resource/font failure journeys, and the
remaining installed-consumer distribution matrix. U4/U6–U8 must still deliver the dedicated
recipes/effect consumers before this catalog may advertise those designs.

## Scoped Customization and Fresh-Consumer Import

Checked after source baseline `9b99e1aa3`, on 2026-09-17. The executable Web example exports
Cyberpunk, copies its complete recipe, changes the canvas base and Node border paint, and appends
a Class-only Node fill rule. Existing layers, effects and unrelated authored facets are retained;
no new patch engine, registry, token-reference grammar or runtime dependency is introduced.
The [user guide](../../../rendering/custom-diagram-themes.md) covers simple definitions, complete
recipes, rule ordering, Clear versus transparent, and JSON plus accompanying README/license files.
Custom exports do not inherit catalog design or qualification claims.

Verification on this slice:

- Real full-WASM package smoke passed across 35 diagram entries. The added example is exercised
  through the public Web entry point, JSON save/load, and raw WASM output equivalence. Its color
  substring checks are smoke checks, not a visible-terminal oracle. The existing resource contract
  still rejects an edited-recipe render with `max_source_bytes: 1` as `MERMAN_RESOURCE_LIMIT_EXCEEDED`.
- CLI nextest: **3/3 passed**, 35 unrelated tests filtered out. The new test saves an edited complete
  recipe and starts fresh CLI processes. XML assertions identify the canvas and Class node paths;
  they verify the requested paints, source-owned fill/stroke/width, Clear restoring family fill,
  transparent remaining transparent, and the Class-only rule not changing Flowchart fill. Existing
  direct recipe import and encoded theme-file budget tests also pass.
- A separate local probe exported the recipe from the actual WASM module, applied the shared Web
  example, wrote `shared.json`, and passed that exact file to fresh native CLI processes for Class
  and Flowchart. XML terminal assertions passed. This is a local built-artifact cross-consumer
  witness, not a freshly published npm/wheel or full installed-platform matrix.
- An independent source review reported no correctness finding in the example, focused tests or
  guide. It correctly limited the Web substring checks to smoke coverage.

Logs: `/tmp/merman-u13-edit-web-smoke.log`, `/tmp/merman-u13-edit-cli.log`.
Probe artifacts: `target/bench/experiments/theme-authoring-9b99e1aa3/`.
The first CLI test run exposed a stale guessed Flowchart node ID; the assertion now uses the
actual `data-id="A"` terminal identity rather than a historical generated-ID convention.

### Remaining public-interface decisions

1. **Complex preset brand parameters remain unresolved.** Current Cyberpunk exports 68 expanded
   rules (6,631 compact JSON bytes before editing). The tested workflow changes explicit roles;
   it does not establish a convenient global palette parameter for an effect-bearing recipe.
   Equal paint strings in Node, Actor, Edge, Marker and series rules are not proof of common token
   ownership. Decide the semantic customization seam with U4's complete recipe builder before
   freeze; do not implement global JSON string replacement as a recoloring feature.
2. **Web actual-outcome access is incomplete.** `renderSvg()` returns a string; `svgPlanJson()`
   reports artifact capabilities, not the consumed/residual theme facets. A real-WASM probe appended
   an unqualified Class Node `stroke.width: 2` rule: rendering succeeded, the node outline remained
   `1.3`, and the capability plan returned `ready: true`. A separate `describeThemeSupport()` query
   correctly returned `unsupported` with `theme-support.no-supported-route`. This is a user workflow
   gap, not a false `Portable` claim by the core. The binding operation owner already projects
   `theme_execution_evidence`, but WASM success adapters discard metadata via `into_data`.
   That projection contains coarse theme/target status and reasons, not per-rule/facet details.
   Expose a useful result through the existing execution owners; do not promise detailed
   explanations merely by forwarding the current metadata or create a second evidence engine.
   The probe and SVG are retained as
   `unsupported-width.json` and `unsupported-width.svg` in the artifact directory above.
3. **Strict policy is not exposed in binding options.** Web and binding environment JSON expose
   text measurement and math rendering, not the Rust facade's `RequirePortable` requirement.
   `resvg-safe` selects a rendering pipeline; it is not an equivalent strict-theme switch.
   The request owner builds the default BestEffort environment. Settle the explicit request policy
   and error behavior alongside actual-outcome access before freeze. Independent source review
   confirmed both projection gaps as existing P2 delivery issues, not regressions from this slice.
4. **Missing-resource and broader distribution journeys remain open.** No claim is made here about
   offline font behavior, disabled embedded-font capability, complete canvas/effect rendering in
   every consumer, dense reference scenes, PNG/PDF equivalence, or all artifact profiles.

U13 is still open. These decisions precede public contract freeze; the successful palette-edit
journey does not close U4/U6–U9 or promote current presets to full Modern Mermaid reproductions.

## Actual SVG Outcomes and Explicit Strict Policy

Implemented after source baseline `76e89f69e`, on 2026-09-17. Web now exposes `renderSvgResult()`
and `renderSvgResultWithTextMeasurer()`. Both return `{ svg, metadata }` from one existing binding
operation. The string-returning functions remain available. The result retains the canonical
versioned metadata and its `theme_execution_evidence`, with no second render, independent receipt,
or new certification schema. `byte_length` remains UTF-8 bytes. Unknown evidence versions and
status IDs remain opaque/open and must not grant portability.

The shared binding request now accepts `environment.theme_portability`, with `best-effort` as the
default and `require-portable` mapped to the existing renderer policy. Unknown values are rejected
as `MERMAN_INVALID_ARGUMENT`; a failed strict render retains `MERMAN_RENDER_ERROR`. Web and Node
option declarations expose the new choice. This is an execution policy, not a pipeline alias or
an instruction to substitute another preset. BestEffort may return SVG while metadata says the
native target was rejected; only an explicit strict request makes that admission failure fatal.

### Production output-policy defect found by the positive test

The first strict success test failed on the public `SvgOutputPolicy::pipeline()` path even though
the equivalent core `SvgPipeline::resvg_safe()` request passed. The shared output policy registered
a GitGraph label-baseline pass for every family. Its implementation returned the original SVG for
other families, but the pipeline invalidated prepared-text and typed-theme evidence merely because
the pass was registered. This affected public binding/CLI output and was not a font-decoding error.

The pipeline now stores an internal family scope with each configured pass. Execution and
prepared-text/typed-theme/math preservation use the same applicability predicate, evaluated using
the renderer-owned family before selecting prepared text and its ledger. Only the internal
GitGraph pass receives a GitGraph scope. Actual GitGraph changes and unknown family identity remain
conservative; public custom passes remain global and untrusted. Root-background modifications keep
their prior invalidation behavior. The finalization report still records configured pass names.
No public preservation assertion, new proof engine, dependency, or production font asset was added.

### Verification

- Binding core and native-host WASM unit tests: **287/287 passed**. Includes default/explicit
  BestEffort equality, one-shot/reusable/base-options policy agreement, unknown-policy rejection,
  unsupported winning-facet rejection, and a real strict Portable native-text SVG using the
  existing test font. The positive test uses the public output policy; the temporary diagnostic
  bypass to a bare core pipeline was removed before the passing run.
- Renderer policy/GitGraph/prepared-text regression tests: **8/8 passed**, 2,570 unrelated tests
  filtered out. Covers all logical families, unknown family identity, public custom passes,
  root-background invalidation, actual GitGraph geometry changes and prepared-text reservations.
- Math-enabled renderer regressions: **2/2 passed**, 2,604 unrelated tests filtered out. Canonical
  output retains legitimate math evidence; untrusted mutation still fails.
- Web TypeScript and contract checks passed: **40 WASM exports, 50 runtime bindings, five package
  entries**. Focused Web authoring and package-surface tests: **19/19 passed**. Unknown evidence
  versions/statuses are retained conservatively, invalid numeric options never reach WASM, and
  result wrappers forward one call and the host callback.
- Rebuilt both SVG package profiles serially: full input fingerprint **`d74920d854d4`**, render
  **`a5188ab17fba`**. Real-WASM smoke passed for each, across 35 diagram entries per package. New
  checks cover same-execution SVG/metadata, UTF-8 byte counts, callback invocation and equivalent
  measurement work, unsupported Class width, a real strict success, and a mixed supported-stroke /
  unsupported-radius failure with the same embedded font still valid.
- Real Chromium initialized the assembled public browser entry points independently for full and
  render. Each produced `verified / portable` for the embedded-font Sequence recipe. Adding only
  the unsupported Lifeline radius produced `residual / rejected`, with the sole target reason
  `theme_evidence_incomplete`, under BestEffort. Strict execution returned `MERMAN_RENDER_ERROR`.
  SVG equality and UTF-8 byte-length checks passed. This is actual browser API execution, not
  visual qualification or a complete browser/platform matrix.
- Independent source review found no remaining issue in these implementation seams. It required
  real host callback evidence and retained the distinction between coarse results and detailed
  diagnostics; both are reflected above.

Logs: `/tmp/merman-u13-result-{rust-green,pipeline,math,web-build,web-unit,wasm,render-wasm,web-smoke,render-smoke,browser}.log`.
The reproducible local browser probe and results are retained under
`target/bench/experiments/theme-outcome-76e89f69e/`.
One renderer build was interrupted by a full disk before tests executed; removing only inactive
Rust incremental caches allowed the scoped rerun. Source and audit artifacts were retained.

### Remaining gates after the coarse-outcome slice

The coarse Web outcome-access gap and explicit shared-policy gap above are now implemented and
verified for the two browser SVG profiles. U13 is **not closed**. Meaningful rule/facet diagnostics
still need a deliberate projection: the current renderer summary drops residual detail, a family
residual identifies a compiled rule rather than each facet, and compiled rule indexes are not
original JSON `/styles/N` paths. Do not infer missing detail from static support matrices or expose
internal proof objects as an authoring explanation.

Complex preset semantic color parameters, missing-resource workflows, consumer/UI rollout and the
remaining installed-platform matrix remain open. Node's option declaration was updated, but its
installed package was not rebuilt here. No catalog qualification, reference visual parity,
performance budget or C7a contract-freeze claim is promoted by this slice.


## Source-Located Render Explanations

Implementation baseline: `19d35e65c`, 2026-09-17. This slice advances the diagnostic gap recorded
above; it does not close U13 or certify reference visuals.

The final `FamilyRenderReport` now projects user explanations while its render session still owns
the compiled theme. The facade moves that immutable list into `RenderEvidence`; binding metadata
and Web `renderSvgResult()` expose it as `theme_execution_evidence.diagnostics` in the existing
unpublished version-one envelope. There is no new admission authority or public compiled-rule ID.

The materializer and wire decoder retain a private source map at the actual expansion, palette
replacement and rule/palette split points. Complete-spec paths refer to the input's mixed `styles`
array; Definition paths refer to authored entries or explicitly supplied tokens. Defaults remain
marked as generated without fabricated locations. Typed Rust specifications and built-in selections
have no original JSON document. Exporting and reimporting gives locations in the new complete-spec
payload. Source maps do not participate in visual recipe fingerprints.

The explanations remain deliberately less precise than a facet ledger: a mixed rule may apply paint
while retaining unsupported geometry. Root effect bindings currently identify the `/effects`
container, and expanded Definition typography has no precise token location. Missing diagnostics
mean the producer did not provide them; an explicit empty list is not a Portable certificate.
Unknown diagnostic identifiers remain open. Strict operation failures still use the existing error
path rather than returning a successful result with diagnostics.

### Correctness finding from the public journey

The new real-binding override test exposed a pre-existing Sequence defect: a Lifeline rule containing
only radius was omitted from terminal observations and could be classified NotApplicable. Adding
stroke paint to the same rule activated the receipt path and correctly retained the radius residual.
The writer and recorder both had paint-only gates. Message shared both gates; Loop had the recorder
gate. The fix uses their existing terminal/winner observations for all matching static facets.
It adds no support for radius and does not change the source-ownership precedence.

Local tests cover a later radius overriding an earlier one, source paint ownership retaining the
unsupported geometry, and absent Message/Loop terminals staying NotApplicable. The public binding
regression checks only the winning input location is reported. A diagnostic for a supported sibling
facet is not synthesized from the rule-level key.

### Verification scope

- Renderer compiler/materializer/admission/theme and Sequence evidence plus mutation regressions:
  **354/354 passed**, with 2,231 unrelated unit tests filtered out.
- Binding core and WASM Rust suites with SVG/Cytoscape: **291/291 passed**.
- Web TypeScript/contract build and focused authoring/surface tests passed.
- Binding metadata generation was refreshed through the existing xtask owner.

The source-map and metadata getter tests first failed to compile before implementation. The actual
pure-geometry render test then failed with zero rule diagnostics before the Sequence fix. Two
initial test-fixture mistakes (`ordinal_palette` rather than `ordinal-palette`, and omitted required
`tokens`) were corrected separately; those input errors are not evidence of a production defect.

Logs: `/tmp/merman-u13-diagnostics-{red,binding-red,sources,focused,binding-green,renderer,web-types,web-tests,generate,contract}.log`.

Cross-platform source inspection found raw metadata forwarding in C ABI, Node, UniFFI/Python and
Flutter, so the new list uses the existing transport surface. This inspection is not an installed
package or host-build rerun. Per-facet explanations, complex preset semantic customization,
missing-resource journeys and the remaining consumer/UI rollout stay open.


### Actual Web artifacts and local footprint

Both SVG packages were rebuilt from this implementation, assembled, and passed their actual WASM
smoke suites (**35 diagrams per package**). The same embedded-font loop scene first passes strict
admission; adding only unsupported radius to Lifeline, Message or Loop then leaves exactly
`theme_evidence_incomplete`, reports only the later winning `/styles/1`, and fails strict admission.
This exercises the production writers rather than manufacturing receipts in a unit test.

Chromium also loaded both public package entries and verified the Portable control, a partly applied
stroke-plus-radius rule, the pure-radius override case, byte length and SVG equality with the string
API. Full input fingerprint: `38ab504fe0d9`; render: `43eaae4f20d1`. Both input manifests passed
freshness verification. These checks cover the two SVG packages, not every Web package/profile.

The following comparison is against the previously built local artifacts from the preceding
`19d35e65c` slice, **not** against released alpha.6 and not a measurement of the whole refactor.
Tool versions and feature presets match; raw hashes, input manifests and deterministic gzip-9
measurements are retained under `target/bench/experiments/theme-diagnostics-19d35e65c/`.

| WASM profile | Previous bytes | Current bytes | Raw delta | gzip-9 delta |
| --- | ---: | ---: | ---: | ---: |
| full | 16,078,220 | 16,090,654 | +12,434 (+0.077%) | +3,433 (+0.057%) |
| render | 14,177,036 | 14,189,450 | +12,414 (+0.088%) | +5,394 (+0.101%) |

No dependency or font asset was added. No size budget was changed. This comparison says nothing
about latency or peak allocation. The facade consumes the renderer's diagnostic list without a
second deep copy; metadata serialization still owns its host representation. Source capture during
materialization and final report projection have not received a dedicated timing/allocation study.

Changed-crate `cargo clippy --lib` passed with existing warnings; scoped `cargo fmt --check`,
`git diff --check` and generated binding-contract verification passed. Web TypeScript contracts and
focused authoring/package tests passed. No full workspace, release archive, installed Node/Python
or all-platform build rerun is claimed.

Additional logs: `/tmp/merman-u13-diagnostics-{wasm-full,wasm-render,assemble,full-smoke,render-smoke,browser,clippy}.log`.
The browser probe and its JSON results are retained alongside the footprint measurements.


The scoped code review completed with no remaining findings after adding the real Message/Loop
writer cases. It covered correctness, project standards, testing, maintainability, security,
performance, API contract and adversarial scenarios across independent Codex review contexts.
Receipt: `/tmp/compound-engineering-501/ce-code-review/u13-20260917T145449/review.json`
(`status: complete`). The simplification pass retained the source-owner separation and removed the
facade's avoidable diagnostic deep copy. These review conclusions apply to this slice only.
