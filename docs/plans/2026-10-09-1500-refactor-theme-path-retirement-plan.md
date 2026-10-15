---
title: Fearless Refactor of Unified Theme Lowering and Historical Path Retirement
type: refactor
date: 2026-10-09
artifact_contract: ce-unified-plan/v1
product_contract_source: ce-plan-bootstrap
execution: code
---

# Fearless Refactor of Unified Theme Lowering and Historical Path Retirement

## Execution Status

**U1-U8 are complete.** The final deletion closure, local contract matrix and
affected remote platform CI have been verified. CI run `38023696805` at
`3a2a2de46476365b2d45270f35ce036da62fbb4d` completes successfully with 55 passing
jobs and the successful `pr-gate`; the unselected `typst-package-compat` job is
intentionally skipped. Performance run `38023696796` at the same commit passes.
These workflow results do not turn the separately recorded inconclusive native
latency row or unavailable Sequence allocator lane into successful measurements.
No merge or release was performed.

The initial implementation was frozen at `3a11f46ba`;
`62c59113a` contains only documentation changes, and `17da680e9` repairs facade
documentation comments. The initial local measurements use that clean revision
and record comment-only repairs separately from runtime evidence. The follow-up
shared implementation at `dd79ca584` has its own verification and measurements.
The initial verification update was pushed as `cb3958a43` to the existing feature branch and
PR #178. Run `38012289765` found stale fuzz lockfile inputs and a Typst size-budget
regression. The local fuzz fix passes its pinned nightly locked check; Typst
shared implementation improvements are committed at `dd79ca584`. Its 4,864
renderer tests and strict CLI clippy check passed. Runtime confirmation completed:
eight rows confirm non-regression, while label reuse remains A/A-inconclusive;
the overall result is inconclusive, with no confirmed material regression.
The authorized six-artifact size rebaseline follows the existing measured +3%,
round-up-to-1,000-byte rule. One-source package verification is complete and all
24 replacement size checks pass; twelve limits decrease and twelve increase.
Remote verification of the shared implementation and replacement limits passed
in the final CI run. Shared-source facade (303), native-export (98), full SVG comparison
and fresh browser (77) checks pass; follow-up raw evidence is archived separately.
All seven remote fuzz jobs now pass
in run `38016218098`; that historical run failed only Typst size and its aggregate
gate. Both pass in the final run after the authorized package rebaseline.
The earlier sandbox Git-write restriction has been resolved. The final
measurement report and archives are included with this local verification
update. After loopback socket access was restored, all 704 CI script unit tests
passed; the earlier five environment errors remain archived alongside the recheck.

- U1: Inventory covers all 35 renderer families, including Error; there are
  34 selectable diagram families. The
  [final deletion closure and replacement owners](../knowledge/engineering/theme-path-retirement-inventory-2026-10-09.md)
  preserve the initial inventory separately from the final family partition.
- U2-U4: Family terminal bindings and final Flowchart node/cluster/edge source
  cutovers are committed. The frozen-tree static audit found no remaining production
  terminal interpretation outside prepared family artifacts. Edge typography is
  occurrence-owned; matching bases share layout projection, distinct HTML bases
  remain explicit, and unlabelled edges retain no font payload. Block class and
  hand-drawn wrapper declarations are prepared before emission.
- U5: Historical authorization gates, orphaned raster acceptance machinery and
  their callers are removed; the executable-call audit found no surviving invocation.
- U6: Unused compatibility providers, fallback evidence and private conveniences
  are removed. Public Mermaid input compatibility remains in the core.
- U7: Current guides, real examples and resource-helper schema descriptions are
  corrected. Historical migration records remain explicitly separated. The current
  documentation scan checked 46 files and confirmed all 201 relative file links
  exist; this is not remote-URL or exhaustive anchor verification.
- U8: Local contract, SVG corpus/root viewport and optimized-release checks have
  passed. Equal-feature, adjacent-revision latency, allocation and CLI/WASM size evidence is recorded
  in the [final measurement report](../performance/theme_path_retirement_2026-10-10.md).
  Eight latency rows satisfy the registered non-regression thresholds; one A/A
  row is inconclusive. Flowchart allocator smoke checks pass; the supplementary
  Sequence lane is unavailable at both revisions. CLI size increases about 1%;
  the initial unchanged web-full budget passed at `17da680e9`. The final
  shared source has a verified, authorized six-artifact baseline with measured
  +3% ceilings. No speed or allocation improvement is claimed. Remote
  affected-platform CI and its aggregate gate pass at `3a2a2de46`.

### Final frozen-implementation local evidence

These are final local receipts for `3a11f46ba`, distinct from the earlier migration
checks below. Comment-only repairs are identified separately. They do not attest
the later remote platform CI matrix or unavailable measurement lanes.

| Owner | Result |
| --- | --- |
| Curated feature matrix | All 45 builds and isolated-consumer checks passed |
| Workspace defaults | 12,241 passed, 13 skipped |
| Public facade | 303 passed, two skipped |
| Independent PNG/JPEG/PDF exporter | 98 passed |
| Private four-package library closure | 2,798 passed, two skipped |
| Retained projection integration | Nine passed; actual source-paint behavior retained |
| Support discovery | 52 passed |
| Public API/package boundary | Passed |
| No-family core / analysis / editor | Five / one / three passed |
| Ambient binding/FFI feature contract | Two passed |
| Rustdoc examples and compile-fail boundaries | Theme contract six; renderer nine; facade two passed |
| Public Rust examples / committed diagram fragments | Passed |
| API documentation | Broken intra-doc links denied; renderer/theme contract/SVG facade, no-feature facade and all-feature facade builds passed after comment-only link repairs |
| Dependency and legal owners | cargo-deny, representative closures, 13 license reports, 382 release projections and third-party verification passed |
| Web full WASM and TypeScript | Rebuilt full artifact; 42 exports, 53 bindings and five entries verified; input freshness passed |
| Playground | Fresh build and test typecheck passed; all 77 Chromium tests passed |
| Optimized evidence / qualification | 13 passed, one ignored / 11 passed |
| Full SVG corpus | Structure, parity and parity-root comparisons passed |
| Chromium root containment | 3,712 fixtures, zero blocking failures; 54 browser diagnostics, 2,344 inherited cases, one existing exact residual, zero unused residuals |
| Native latency | Nine output identities match; eight rows have 14 AB/BA pairs and confirmed non-regression; one A/A-unstable row leaves overall result inconclusive |
| Native allocator | Flowchart six scales x five repetitions pass existing smoke caps; no allocation reduction or candidate admission; Sequence preflight fails at both revisions |
| Equal-feature CLI size | Raw +1.01%, stripped +1.03%, gzip +0.91%; same 441-row normal package/version closure |
| Web-full size budget | Unchanged official assembled-package budget passed; independent clean-clone comparison retained in measurement report |
| Current documentation | 46 files, 201 existing relative file links; historical references classified separately |
| CI script unit tests | 704 passed after restoring local loopback access; original sandbox errors retained |

The first browser artifact build stopped at a wasm-opt 133 assertion. Repeating
with the existing CI-pinned wasm-opt 131 selected through local PATH, after verifying
its hash, passed. No production code was changed for this tool mismatch. Browser
receipts apply to the rebuilt artifact, not the earlier stale Playground dist.

The initial full root oracle stopped on an external-image decode failure. The
unchanged full recheck passed after focused local/upstream decoding succeeded.
Both receipts are retained; no comparator or residual was relaxed. Performance
archives preserve the unstable A/A row and both failed Sequence preflights.
Those are evidence limitations, not green checks. The complete release-profile
size matrix remains a publication requirement; this local comparison selects
web-full and the equal-feature CLI.

### Completed migration receipts

These are historical incremental checks at their named commits, not additional
final frozen-tree receipts. Previously pending edge/cluster work is complete in
`3a11f46ba`; old status wording does not describe current outstanding work.

| Commit | Completed scope and recorded checks |
| --- | --- |
| `6ff15c63d`, `01cb9ca2d` | First family bindings; Class relations, ER, Requirement, Treemap and EventModeling; scoped owner suites passed |
| `f495a7542`, `1deaec16a` | Compatibility preparation and retirement of the complete `SvgTheme`/`MermaidThemeAdapter` closure |
| `2ea621d8b`, `b997dc160` | Block/ER/Sequence/GitGraph terminal choices, Flowchart artifact node records and Class/Sequence/Requirement/GitGraph output |
| `d88469f1f` | Class/C4/Venn/Mindmap/Sankey/Treemap/Flowchart/Wardley/Usecase final styles; 4,845 renderer tests passed, six skipped |
| `7594b3272`, `bb3f1ae36` | Sequence Neo, Ishikawa look/seed, Requirement shadows and current resource-helper documentation; 4,848 renderer tests passed, six skipped |
| `803ef3512` | Flowchart node source typography and prepared look/seed/shadow/gradient; 4,854 renderer tests passed, six skipped; Agentflow-only cfg check and nine renderer compile-fail rustdoc contracts passed |
| `afedd43e5` | Artifact-owned Flowchart render configuration; 203 affected integration tests passed; measurement harness's 121 contract tests passed, without a renderer performance claim |
| `5e5f5cd0a` | Actual Dagre/ELK/Swimlane cluster candidates and title styles; 4,856 renderer tests passed, six skipped; independent candidate/writer/resource review completed |
| `3a11f46ba` | Final edge typography, Block declarations and unused parser-wrapper retirement; 4,864 renderer tests passed, six skipped; strict all-feature CLI Clippy, format/diff checks and independent review passed |

Review-driven resource fixes charged actual folded Treemap retained fields,
restored fallback font accounting, and removed duplicate render-ID accounting.
Reservations follow actual Arc/sidecar ownership; retained-byte accounting is not
whole-heap temporary allocation or peak-memory measurement. A Flowchart fixture
was corrected to assert its actual canonical first-match source model rather than
ER duplicate-declaration behavior.

Other preserved behavior includes GitGraph boolean coercion, Sequence numeric-string
activation widths, ER duplicate-subgraph first-match handling, and Engine-established
explicit input ownership. Source-authored `var(...)` remains filtered while trusted
host CSS can preserve it. Shared SVG escaping has one owner.

Earlier Web input checks (151 tests), license-report regeneration, third-party and
release-material verification, and six theme-contract rustdoc examples passed at
their recorded revisions. `1fb614180` owns the Web/license refresh; `9f3e5d42d` and
`c8dcdd721` clarify historical/current documentation. Those scoped receipts do not
substitute for final frozen-tree verification. The final workspace, documentation,
dependency, SVG, optimized-release and measurement receipts are listed above;
the affected remote platform CI matrix additionally passes at `3a2a2de46`.

## Goal Capsule

**Objective:** Make every render operation consume one operation-scoped, family-specific terminal theme binding while preserving Mermaid-compatible input behavior, then remove obsolete runtime adapters, migration gates, stale documentation, and test-only compatibility machinery that no longer owns a current contract.

**Means:** Keep Mermaid's public input and upstream theme derivation in `merman-core`; lower the resulting values, structured Merman recipes, and supported source declarations once into concrete renderer-owned terminal bindings. Reuse existing rule/provenance machinery without equating the terminal binding with the public recipe or `ResolvedThemeStyle`. Do not force arbitrary CSS or Mermaid's non-numeric sentinels into typed paint tokens. Retire historical bridge authorization and obsolete presentation APIs only after their current consumers and evidence owners are removed together.

**Authority hierarchy:** Public Mermaid configuration and source semantics outrank Merman defaults; explicit source declarations retain property-level ownership; typed recipe rules supply missing semantic values; family geometry, layout, `look`, font measurement, and native output constraints remain with their current owners. A value's origin is never inferred from equality.

**Stop conditions:** Investigate before deleting a symbol reachable from a published binding, CLI/Web input, current release workflow, Mermaid compatibility behavior, layout/measurement, or native receipt contract. A live caller requires migrating or deliberately retiring its contract in the same unit; it does not justify retaining a historical path indefinitely. If an externally observable contract cannot be preserved, report the concrete blocker before proceeding.

---

## Product Contract

### Requirements

- **R1. Mermaid compatibility remains complete.** `theme`, `themeVariables`, source directives/frontmatter, `classDef`, `style`, `linkStyle`, property-level precedence, unknown-theme behavior, and config serialization remain observable as before. This includes values that are not representable as finite typed colors.
- **R2. Merman presets remain a separate authoring product.** Presets and custom recipes compile through the same structured theme compiler and do not become a second global `themeVariables` cascade. `look` and layout remain explicit controls; a preset does not silently change them.
- **R3. One runtime interpretation exists.** A render operation/family resolves Mermaid compatibility, typed theme rules, supported source declarations, and family defaults into one terminal binding before measurement/layout consumers and writers use it. The old `SvgTheme`/`MermaidThemeAdapter` JSON-read path is removed per migrated family.
- **R4. Non-typed values remain safe and explainable.** The terminal binding may carry raw CSS, optional numeric measurement, action (`set`, `clear`, `inherit`, or residual), source provenance, declaration order, and importance. It is not a general CSS interpreter and does not broaden resource or security policy.
- **R5. Historical retirement is finite.** The executable KTD23 route/projection authorization, tombstone probes, empty-bridge gate, and acceptance-only constructors are removed when their current callers are removed. Durable evidence is retained as a short historical record and current structural invariants remain narrow and source-backed.
- **R6. User-facing documentation describes the current interface.** Current guides point to compiled presets/custom recipes and Mermaid compatibility configuration. Options 2/`HostTheme`/`PresentationProfile` examples and broken example links cannot remain in the current navigation.
- **R7. Existing visual and output contracts are preserved.** SVG structure, default Mermaid behavior, effects that are currently supported, font/resource limits, cancellation, native filters/receipts, and feature-gated family behavior remain unchanged except where a verified duplicate path is removed.

### Key Flows

- **Mermaid render:** Mermaid defaults and source/site configuration are resolved, source declarations are attached with provenance, typed recipe rules fill only unowned properties, one family binding is produced, then the same binding serves measurement, layout-facing style queries, SVG emission, and native finalization.
- **Preset render:** A preset compiles to typed rules plus only its explicit Mermaid compatibility lane; the operation uses the same lowering path as a Mermaid-only render.
- **Retirement:** An inventory proves a candidate has no production/public/release caller; callers and tests are migrated or deleted in the same unit; historical evidence is archived; structural checks assert absence of the retired path.

### Acceptance Examples

- A flowchart with `themeVariables.primaryColor`, `classDef`, inline `style`, and an explicit font declaration preserves the winning property, source path, and emitted CSS/value behavior.
- A sequence diagram with a typed preset and `look: neo` preserves Neo's built-in visual path and does not claim typed glow ownership where that look does not consume it.
- A Mermaid theme value such as `calculated`, a relative font size, or a browser-safe opaque CSS value remains renderable or is reported as a residual without being silently converted to a different typed value.
- A built-in preset and a custom recipe render through the same terminal binding path; no family reads Mermaid theme JSON after the cutover.
- Removing the historical bridge gate does not make a non-empty runtime legacy route pass; current source-level absence and support claims remain independently checked.

---

## Planning Contract

### Key Technical Decisions

- **KTD1. Lower to a renderer terminal binding, not the public recipe schema.** `CanvasPaint` and `ResolvedThemeStyle` are too strict for all Mermaid CSS values. The internal binding must preserve raw emission and optional measurement alongside typed semantic values.
- **KTD2. Preserve core derivation.** `merman-core/src/theme.rs` remains the source of Mermaid catalog, defaults, derived variables, and unknown-input behavior. The renderer consumes its resolved output instead of reimplementing theme-variable derivation.
- **KTD3. Keep CSS interpretation bounded.** Reuse existing safe CSS parsing and ownership code in `merman_style.rs`; do not create a broad CSS cascade engine, browser emulator, or universal theme adapter.
- **KTD4. Migrate by family evidence.** Start with a family whose typed semantic bindings and geometry are stable, then migrate measurement-sensitive families such as Flowchart and Sequence. Delete the old adapter methods only when all callers for that family have moved.
- **KTD5. Retire historical proof with its wiring.** `legacy_family_theme_bridge.rs`, KTD23 projection ledgers, release-preflight authorization, and acceptance tests are one retirement unit. Keep a durable evidence summary and a small current-source invariant, not an executable historical migration oracle.
- **KTD6. Clean docs by ownership.** Current guides and indexes are updated to current APIs; completed plans and migration records remain only when they explain a durable decision, with stale current-status claims corrected. `HostTheme` aliases are not reintroduced.

### High-Level Technical Design

```mermaid
flowchart TB
  A[Mermaid defaults and source/site config] --> B[merman-core resolution]
  C[Typed preset or custom recipe] --> D[Recipe rules]
  B --> E[Operation-scoped family lowering]
  D --> E
  F[Supported source declarations] --> E
  G[Family defaults and look/layout] --> E
  E --> H[Terminal binding: value, raw CSS, measure, provenance, action]
  H --> I[Measurement and family plans]
  H --> J[SVG and native writers]
  H --> K[Diagnostics and receipts]
```

The binding is concrete per family/target and immutable after preparation. It carries only the fields that the consumer needs, with explicit residuals for unsupported or opaque values. `look` can select an existing built-in effect path, but arbitrary theme CSS never becomes a new effect engine. The compiler and family program remain the ownership points for semantic rules; SVG parity helpers become compatibility adapters only until each family cutover is complete.

### System-Wide Impact

- **Core/config:** Mermaid catalog and precedence remain public; only the handoff to renderer lowering changes.
- **Renderer:** family plans, SVG parity theme helpers, source style ownership, effects, diagnostics, and root paint bounds are affected.
- **Bindings/CLI/Web/Playground:** public recipe and compatibility inputs remain stable; examples and discovery metadata must point at the same current path.
- **Acceptance/release:** historical bridge authorization and private constructors are deleted together; current support and structural checks remain independent.
- **Documentation:** current rendering guides, examples index, migration guide, ADR status links, and historical workstream summaries need link/status cleanup.

---

## Implementation Units

### U1. Build the deletion and semantic inventory

**Goal:** Establish a checked, file-level map of active callers, feature closures, public exports, release wiring, docs links, and tests for every candidate path before changing ownership.

**Files:** `crates/merman-render/src/svg/parity/theme.rs`, `crates/merman-render/src/svg/parity/theme/families.rs`, `crates/merman-render/src/diagram_theme/source_styles.rs`, `crates/merman-render/src/mermaid_style.rs`, `crates/merman-render/src/diagram_theme/legacy_family_theme_bridge.rs`, `crates/merman-render/src/theme_route_cutover.rs`, release/acceptance scripts, current theme docs and example indexes.

**Approach:** Classify each symbol as public compatibility, active runtime, acceptance-only, historical evidence, or stale documentation. Record the exact replacement owner and a deletion proof. Include the lazy family compatibility builders and `FamilyPaintDefaultPaths`; do not assume names containing `legacy` are removable.

**Test scenarios:** Inventory detects a published caller, release caller, or source-compatibility path and blocks deletion; it also detects stale links to missing `HostTheme` examples and identifies historical-only files with no current inbound reference.

### U2. Introduce the operation-scoped terminal binding

**Goal:** Add the smallest renderer-internal value that can represent typed values and Mermaid-compatible raw values without losing ownership or measurement semantics.

**Files:** `crates/merman-render/src/diagram_theme/resolved.rs`, `crates/merman-render/src/diagram_theme/source_styles.rs`, `crates/merman-render/src/mermaid_style.rs`, operation/theme preparation modules, focused family binding module.

**Approach:** Reuse existing `ResolvedProperty`, source provenance, CSS safety policies, and diagnostics. Preserve the core operation's frozen path ownership in a compact cross-crate input view; authored versus derived values cannot be recovered from materialized JSON alone. Add explicit raw/measure/action fields only where a consumer needs them. Apply existing family-specific precedence independently for each property: source declarations, including unverified owning declarations, suppress typed winners; authored Mermaid/compatibility values retain their current priority; typed rules can replace derived/default values. This is not a universal numeric priority order. Keep clear and residual states distinguishable from absence. Do not route through global `MermaidConfig` after lowering.

**Test scenarios:** Verify same-value writes retain their source, later declarations obey order and `!important`, `clear` removes a prior owner, relative font sizes preserve measured output, opaque safe CSS is emitted without numeric fabrication, and unsafe/resource-bearing CSS remains rejected or residual according to existing policy.

### U3. Cut over stable families and delete their JSON adapter reads

**Goal:** Move Class and bounded families with equivalent semantic roles to the binding, then remove their `SvgTheme`/`MermaidThemeAdapter` reads and duplicate fallback helpers. U1 records the complete family list; U3 and U4 partition all currently supported families with no omitted family.

**Files:** `crates/merman-render/src/class`, selected family theme modules, `crates/merman-render/src/svg/parity/theme/families.rs`, `crates/merman-render/src/svg/parity/theme.rs`, family program/plan consumers.

**Approach:** Preserve family-specific target ownership and generated CSS behavior. Keep compatibility fallback in core/compatibility compilation, not in a second renderer cascade. Delete each adapter method only after repository-wide caller search and family tests show no use.

**Test scenarios:** Mermaid-only, typed-preset-only, combined, explicit source override, no-theme, and unknown-theme cases produce the same public SVG structure and winning property values for migrated families.

### U4. Cut over measurement-sensitive families and effects

**Goal:** Migrate Flowchart, Sequence, XYChart, and remaining supported families without changing geometry, Neo/classic behavior, or root paint containment.

**Files:** family theme/plan modules, `crates/merman-render/src/flowchart/theme_evidence.rs`, `crates/merman-render/src/sequence/theme_evidence.rs`, root paint/effect helpers, remaining adapter consumers.

**Approach:** Keep measurement and bounds ownership in the family. Lower only the values consumed by each family. Preserve built-in Neo filters and effect bindings as explicit internal mechanisms; do not treat preset selection as a `look` mutation. Remove duplicated effect decoding only where input and output contracts are identical.

**Test scenarios:** ELK and Dagre flowcharts, classic and Neo look, sequence actor/frame shadows, xychart palette fallback, typed effects, Mermaid `themeVariables`, and source styles retain output containment and diagnostics. Native exports continue to honor final-region and filter receipts.

### U5. Retire the historical bridge and compatibility authorization

**Goal:** Remove the executable historical migration contract after runtime routes are absent and current callers are migrated.

**Files:** `crates/merman-render/src/diagram_theme/legacy_family_theme_bridge.rs`, `legacy_projection_retirement.rs`, `legacy_tombstones.rs`, legacy portions of `theme_route_cutover.rs`, `crates/merman-theme-acceptance/src/cutover_manifest.rs`, release-preflight scripts, related exports/tests.

**Approach:** Delete release authorization, route/value probes, acceptance-only constructors, and empty-inventory gates together. Preserve a compact historical record with revision, scope, and digest where it is needed for audit; add only a current source-level invariant that production has no legacy provider/dispatch. Do not generate historical rows from the current matrix and do not replace the old gate with an always-empty pass.

**Test scenarios:** Production feature closures compile without acceptance bridge modules; private acceptance no longer exposes retired APIs; a deliberate reintroduction of a provider/dispatch is caught by the current structural check; public support discovery and runtime rendering remain independent.

### U6. Remove obsolete compatibility builders and duplicate validation

**Goal:** Delete lazy family compatibility providers, old default-path allowlists, duplicate effect/property readers, and tests that only construct retired objects, while retaining security, ownership, cancellation, resource, and receipt validation.

**Files:** core/render provider builders found by U1, `FamilyPaintDefaultPaths`, duplicated validators, obsolete fixture constructors, related module exports and tests.

**Approach:** For each candidate, require one live caller and one distinct contract before retaining it. Merge only identical validation at a shared boundary; keep independent checks where they protect different layers. Do not add setter journals, cross-family evidence frameworks, or a new universal CSS abstraction.

**Test scenarios:** Invalid input, unsafe CSS, cancellation, resource limits, native filter mismatch, error ordering, and provenance diagnostics remain covered after deletions; duplicate-only tests disappear with their implementation.

### U7. Clean current documentation and historical records

**Goal:** Make the repository describe the current theme interface accurately and remove dead navigation and examples.

**Files:** `docs/rendering/custom-diagram-themes.md`, `docs/rendering/presentation-themes.md`, `crates/merman/examples/README.md`, `crates/merman-ascii/README.md`, `docs/bindings/OPTIONS_JSON.md`, `docs/release/ALPHA7_TO_0_8_0_THEME_MIGRATION.md`, relevant ADRs/plans/workstreams.

**Approach:** Mark or remove the Options 2 guide from current indexes; replace nonexistent HostTheme/presentation examples with real compiled-theme examples; keep the 0.8 migration guide as historical compatibility documentation; compress closed workstream logs only when their decisions and evidence have a current owner. Remove obsolete gate instructions after U5, and retain raw performance evidence only under its existing archive/digest contract.

**Test scenarios:** Link and example checks find no current references to missing files or removed APIs; current docs show Mermaid compatibility, preset/custom recipe, look/layout, family support limits, and sharing behavior without claiming unsupported qualification.

### U8. Final contract and performance verification

**Goal:** Prove behavior and size changes are attributable to the deletion, not to hidden feature or evidence changes.

**Files:** affected tests, performance summary/evidence, support catalog and release notes only when required by measured results.

**Approach:** Run the repository's serial owner matrix for normal, all-diagram, native/export, private acceptance, browser, lint, dependency, and docs/link surfaces. Compare adjacent clean revisions with equal feature closures and calibrated A/A pairs before making latency, binary-size, allocation, or WASM claims. Record residual browser/font differences instead of widening comparators.

**Test scenarios:** All R1-R7 acceptance examples pass; no migrated family reads JSON theme state; no production legacy route or obsolete public export remains; representative SVG/native outputs and artifact budgets are unchanged or have a documented attributable improvement.

---

## Verification Contract

- **Static ownership:** repository-wide symbol/reference search confirms every deleted path has no published, runtime, release, or current-doc caller; a current source invariant catches reintroduction of a production bridge.
- **Behavior:** owner tests cover Mermaid-only, typed-only, combined precedence, source styles, unsupported CSS residuals, effects, `look`, geometry containment, cancellation, resources, and native receipts.
- **Feature closures:** validate normal production, all-diagram with layout features, export/native, and private acceptance closures separately; do not treat a successful default build as coverage of cfg-gated acceptance.
- **Documentation:** link checker, example inventory, and API-doc build pass with no stale current references to Options 2/HostTheme.
- **Performance:** use adjacent-revision, equal-feature, calibrated A/A methodology; retain raw receipts and a small summary with revision, workload, feature closure, toolchain, hashes, and measured deltas.

Concrete owner checks, run serially with the shared target directory and bounded build concurrency:

```bash
cargo fmt --all -- --check
cargo nextest run --locked -p merman-core --lib
cargo nextest run --locked -p merman-render --all-features --lib
cargo nextest run --locked -p merman --features all-diagrams,svg,png,pdf,layout-cytoscape,layout-elk,math
cargo nextest run --locked -p merman-export --features png,pdf --lib
python3 scripts/run_theme_acceptance.py nextest run --locked -p merman-theme-acceptance -p merman --features merman/all-diagrams,merman/layout-elk,merman/math --lib -E 'package(merman-theme-acceptance)'
python3 scripts/verify_theme_acceptance_boundary.py
cargo clippy --locked -p merman-cli --all-targets --all-features -- -D warnings
cargo deny check advisories bans licenses sources
npm --prefix playground/tests run typecheck
npm --prefix playground/tests exec -- playwright test theme-cyberpunk.spec.ts --project=chromium-desktop
```

U1 also records the current GitHub Ubuntu build/test and binding smoke lanes; U8 runs their applicable local steps plus the full affected feature/platform CI matrix. U5 removes obsolete retirement-specific commands from CI and acceptance wrappers together; surviving checks must execute nonzero relevant tests. Existing structure/parity comparison and browser root-containment checks remain authoritative. No check listed here was run during planning.

---

## Definition of Done

- Every supported Mermaid input path and public preset path reaches one family terminal binding before family consumers use theme values.
- No supported family reads visual theme JSON through `SvgTheme`/`MermaidThemeAdapter`; their old visual-reader implementation and exports are deleted. Parser/layout configuration reads remain with their original configuration owner.
- Historical bridge authorization, tombstones, acceptance constructors, and release wiring are removed together, with durable evidence and a narrow current invariant.
- Obsolete compatibility builders, duplicate readers, stale current docs, broken example links, and dead exports are removed; migration history that remains is clearly historical.
- Mermaid compatibility, preset/custom recipe behavior, visual/effect semantics, geometry, native/export, security, cancellation, and resource contracts remain verified.
- The final diff has a complete deletion inventory and no new universal CSS engine, global cache, allowlist bypass, or long-lived compatibility shim.

---

## Scope Boundaries

### Deferred to Follow-Up Work

- Expanding preset qualification metadata or promising pixel parity with Modern Mermaid fixtures.
- Replacing host font measurement or browser-dependent rendering with a new layout engine.

### Outside This Refactor

- Removing Mermaid's public compatibility inputs.
- Changing the pinned Mermaid release or default ELK/layout behavior.
- Adding a theme registry, remote theme loading, automatic font embedding, or a general CSS browser runtime.

---

## Appendix

Primary research artifacts used for this plan:

- `docs/performance/theme_retirement_and_deduplication_2026-10-08.md`
- `docs/performance/theme_root_paint_containment_2026-10-09.md`
- `docs/adr/0077-presentation-theme-and-output-ownership.md`
- `docs/adr/0082-versioned-theme-authoring-facade.md`
- `crates/merman-render/src/diagram_theme/mermaid_compatibility.rs`
- `crates/merman-render/src/diagram_theme/source_styles.rs`
- `crates/merman-render/src/mermaid_style.rs`
- `crates/merman-render/src/svg/parity/theme.rs`
