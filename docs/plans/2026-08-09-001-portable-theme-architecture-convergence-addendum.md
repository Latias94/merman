---
title: "Portable Theme Architecture Convergence - Plan Addendum"
type: refactor
date: 2026-08-09
updated: 2026-08-13
artifact_contract: ce-unified-plan/v1
artifact_readiness: implementation-ready
product_contract_source: docs/plans/2026-08-06-001-refactor-portable-diagram-theme-architecture-plan.md
origin: docs/plans/2026-08-06-001-refactor-portable-diagram-theme-architecture-plan.md
execution: code
---

# Portable Theme Architecture Convergence - Plan Addendum

## Goal Capsule

| Field | Contract |
| --- | --- |
| Objective | Converge Merman's visual customization layer inside the broader headless Mermaid engine. Merman continues to own parsing, analysis, layout, rendering, and editor-facing semantics with Mermaid parity as a quality target; the theme workstream adds typed custom themes, private family adapters, capability discovery, and target-specific portability evidence. Keep default Mermaid parity unchanged, retain a bounded opt-in built-in preset catalog, and prevent theme convenience features from becoming a second rendering architecture or an automated design product. |
| Product contract | The August 6 R/A/F/AE identifiers remain the traceability source. This addendum is authoritative for the current-release scope, ownership, evidence gates, product boundary, preset admission, milestone sequencing, and public-surface timing; any conflicting current-release statement in the August 6 plan is superseded by the decisions below. Long-term capabilities remain future requirements until their named gate is closed. |
| Authority | Mermaid `11.16.1@7ecca0cd` remains the semantic baseline. The compiled theme recipe describes visual intent; the render environment owns runtime ceilings; family and document evaluators prove what was actually applied. |
| Execution | Continue in `.worktrees/presentation-theme-model` on `refactor/presentation-theme-model`. Treat the three product milestones below as review and delivery themes, not three mandatory giant diffs. Keep making proof-first, independently reviewable Conventional Commits. Finish core convergence and C6a before declaring the authoring/discovery candidate, then contract and roll out the integration surface before C7a freezes. Stable cross-target preset claims wait for C6b. |
| Stop conditions | Do not expose a positive portability or capability conclusion from an unevaluated state, accept host-produced geometry without request/session evidence, let theme/config inputs widen renderer-owned policy, create preset-specific renderer branches, or add brand scanning, editorial rewriting, page-shell composition, bespoke routing, icon art direction, or motion playback to the core engine. |

## Why The Order Changes

The original direction remains valid, but implementation breadth has advanced faster than the
proof chain underneath it. Six conditions now require an architecture convergence gate:

- compile-time `required` capabilities are currently published as renderer `admitted` capabilities;
- an unadapted family with no collected residuals is currently reported as verified;
- a selected compiled theme can replace rather than restrict renderer-owned font policy;
- the resource-policy repair now applies host/request ceilings before theme bytes, fonts, and effects
  are decoded; cross-host verification remains a named gate;
- native export now seals the exact tokenized artifact with request, projection, line, run, and
  face/source identity, while the exporter verifies final emitted line text and face/source facts;
  exact terminal source/visible range proof for multi-face output remains a C6b gate;
- the initial prepared-text backend contract could not produce a result externally, while internal
  results were not bound to a request digest, source ranges, run evidence, or runtime fallback. The
  current WIP has a provisional raw DTO and basic admission seam; this addendum retains it without
  treating its host-assurance shape as frozen.

Stable canvas/effect types, bindings, preset IDs, and additional public family surfaces must not
expand until these conditions are resolved. Internal proof recipes and alpha preset candidates may
still be added when they use the existing typed recipe path, introduce no dedicated renderer branch,
and carry no stability claim. Existing source-backed corpus, resource validation, sealed-document
work, and the Flowchart SVG-label vertical slice remain useful inputs rather than discarded work.

The original C4 scope also combined two different assurances: deterministic native layout and
portable qualification of an arbitrary external host backend. They share a response seam, but they
do not have the same product urgency. Split them into C4a and C4b so the native text core can prove
real theme vertical slices without weakening the external-host trust boundary or prematurely
turning it into a stable cross-process protocol.

## Superseding Release Decisions

This section is normative for implementation and release decisions. The August 6 plan remains the
long-term requirement and traceability document, but it is not a second authority for the current
release when a statement below differs.

| Topic | Current-release decision | Later decision point |
| --- | --- | --- |
| Runtime policy ownership | `RenderEnvironment` and `RenderSession` own effective admission, source policy, measurement fallback, and runtime ceilings. The compiler may validate and record an earlier restriction, but it does not own the effective runtime policy. | Revisit only if a future host-policy architecture changes this ownership explicitly. |
| Compiler resource ownership | Every host derives one `ThemeResourcePolicy` ceiling from its selected resource profile before theme JSON, base64 assets, font catalogs, or effect graphs are decoded. A request may only intersect that host ceiling with stricter limits. | Revisit only if resource profiles and theme compilation are replaced by a different host-owned policy mechanism. |
| External prepared text | External results are crate-private and `HostDependent` by default. The current release does not expose an external DTO/trait through JSON, FFI, or stable bindings, and does not promote self-attested results to `Portable`. | A separate external-assurance plan may define an independent trust root and a new contract; it is not part of C7a. |
| Product identity | Merman is a headless Mermaid engine for parsing, analysis, layout, rendering, and editor-facing semantics, implemented with Mermaid parity as an explicit quality goal. Within that product, the visual customization layer owns typed theme authoring, capability discovery, and honest target admission. It may provide a bounded built-in visual preset catalog, but it does not scan brands, rewrite diagram content, choose a visual story, compose page chrome, or own animation and other application presentation policy. | A separate downstream application or adapter may compose those product decisions around Merman output without changing the core contract. |
| Theme language | Stable `DiagramTheme` is a semantic visual recipe: typed targets, variants, paint/geometry, typography, bounded canvas/effects, and declared resources. It is not an arbitrary SVG/DOM/CSS programming language. Arbitrary selectors, element IDs, XML/HTML injection, and browser-only mechanisms remain `Unverified` or `SvgOnly` compatibility lanes. | A new primitive enters the stable language only after a real family consumer, resource limits, and target-specific evidence exist. |
| Theme authoring freedom | Users may create new themes by composing the supported `ThemeTokens` and full `DiagramThemeSpec` vocabulary, including family-scoped rules and bounded resources, without modifying renderer code. A new semantic target or drawing primitive requires renderer work and a new capability/evidence gate. | Additive target descriptors may be introduced through versioned discovery; they do not become silently accepted selector strings. |
| Built-in presets | A preset is a convenient, versioned materialization of the same typed recipe accepted from Rust, JSON, and bindings. The catalog is open to additional generally useful themes; Brutalist, Spotless, Cyberpunk, Modern Slate, or any later theme name is illustrative rather than a fixed roadmap or product-brand hierarchy. A preset must not select layout, Mermaid look, output pipeline, trusted postprocessors, or family-private behavior implicitly. | Alpha candidates may remain discovery-only while coverage is incomplete. Stable status is decided per preset from C6-backed family/target cells, allowed residuals, resource burden, and licensing rather than from theme name or catalog size. |
| Preset cost boundary | Built-in presets should normally compile from static Merman-owned recipe data and reuse existing code and resources. They must not require a CSS/JavaScript interpreter, network access, brand crawler, application runtime, or substantial preset-only dependency/asset closure. Bundled fonts or other material assets require an explicit redistribution, size, resource-policy, and portability decision. | A heavier theme pack can live in an optional downstream package or host application instead of enlarging the core artifact. |
| Published binding epochs | `0.8.0-alpha.5` published the leaked family and renderer taxonomy under UniFFI API 3 and Web/WASM transport API 3. The subtractive family-catalog correction therefore supersedes the August 2 FFI-plan KTD9 only for those two epochs: UniFFI advances to API 4 and Web/WASM advances to transport API 4. Native C ABI 3, runtime-catalog schema 1, Android transport API 1, Typst plugin ABI 2, and the Node transport remain unchanged. The incompatible post-alpha.5 Options grammar advances independently to schema 3 under KTD16. The next prerelease must carry the corrected projections; an already published alpha.5 artifact is never republished in place. | A later epoch requires another demonstrably incompatible transport change; additive catalog rows and fields continue to evolve inside the current schema rules. |
| C5 versus C6 | C5 owns compilation, facet-level routing, direct family consumption, and family-local tests. C6a owns native terminal evidence; C6b extends the same proof to Browser SVG, JPEG, and PDF. | A bridge route is removed only after its matching native C6a evidence passes. Broader target claims wait for C6b. |
| Public surface timing | C7a-candidate may be declared only after C6a plus real pre-freeze family-writer/compiler round trips. C7a-rollout migrates bindings, Typst, examples, and native Merman CLI against that alpha candidate. C7a-contract freezes only after rollout verification. Fine-grained mechanism/evidence types and alpha preset IDs remain private or explicitly unstable. | Stable preset catalog and broader target/family claims require C6b. |
| Official CLI | New theme selection belongs only to the Merman native render/batch surface. The official `mmdc` compatibility surface remains unchanged except for removal of provisional alpha fields. A document, Options JSON, or project config cannot self-authorize a trusted lane. | Any mmdc expansion requires a separate compatibility decision. |

### Identifier Disposition

The following mapping makes the relationship to the August 6 identifiers executable rather than
implicit. Any August 6 identifier not listed here is retained unchanged and keeps its original
owner.

| August 6 identifiers | Current-release disposition | Owning gate |
| --- | --- | --- |
| R3, R3a; A7; AE19 | Retained with an alpha contraction: `ThemeTokens` remains the design-system adapter but its candidate stable vocabulary is limited to cross-family roles. Family-specific actor, note, activation, cluster, and similar values materialize through deterministic token-to-rule defaults or the full `ThemeRuleSet`/`DiagramThemeSpec`. Reuse and light/dark state-isolation requirements remain unchanged. | C6a design-system and cold-start witnesses, then C7a-contract. |
| R8; AE21, AE24 | Retained for source traceability, contributor attribution, complete typed mechanism classification, and representative reference comparison. Superseded as a fixed named-product obligation: neither `pr28-modern-slate` nor the six named showcase themes is required to ship as a preset or complete C7b. Individual recipes may remain test witnesses or enter the open preset catalog through KTD14's maturity gate. | C6a/C7b evidence; C7a/C6b only for preset maturity claims. |
| R13, R43, R45; F2; AE15; KTD4, KTD8, KTD11; U3 external-host clauses | Native prepared text and external `HostDependent` safety are retained. External `Portable` promotion, fully attested host claims, stable external DTOs, and browser/FFI projection are deferred and do not participate in the current release. | C4a owns native proof, C4b owns crate-private external safety, C7c owns any future assurance contract. |
| R24, R35, R48; F5; AE7, AE18 | Retained with a stricter authority boundary: raw CSS and arbitrary SVG remain explicit trusted compatibility inputs, never self-authorize through document/config data, and cannot bypass the independent terminal safety decision. | C2 terminal safety and C7a-rollout public-surface checks. |
| R25, R47; F3, F7; AE17; KTD5; U4 | Retained and strengthened with the fixed terminal order: postprocess, active-content/CSS safety, resource closure, residual merge, then target admission. | C2. |
| R33-R37; F4, F8; AE18-AE20; KTD1, KTD9, KTD10; U8-U10 | Retained but reordered: define a candidate native contract, migrate first-party consumers against it, then freeze only after rollout verification. KTD15 narrowly supersedes KTD9 for the already-published UniFFI and Web/WASM family-capability epochs; KTD16 separately advances the incompatible post-alpha.5 Options grammar to schema 3. The official `mmdc` surface does not gain new theme flags. | C7a. |
| R38-R41; AE3-AE6, AE19, AE20, AE22, AE23; KTD2, KTD6, KTD7, KTD12; U1, U6, U7, U11 | Retained. Positive public claims require family-writer and target-artifact evidence, not model expressiveness or fixture declarations alone. The full reference corpus remains a mechanism-classification source; publishing every reference theme as a preset or named showcase is not required. | C5, C6a, C6b, and C7b as specified below. |
| R42, R49; AE11, AE13, AE18; KTD3, KTD13; U2 | Retained with effective runtime policy owned by `RenderEnvironment`/`RenderSession`; compiler policy is only an earlier resource/admission restriction and provenance. | C1-C3. |

### Theme Language Boundary

The product promise is **composable semantic theming**, not arbitrary drawing. A theme author can
choose values, rules, variants, typography, bounded paint/effect graphs, and embedded resources
already represented by the stable recipe. The renderer remains responsible for semantic model
objects, layout, DOM structure, resource IDs, and target lowering. A theme that needs a new model
role, selector semantics, arbitrary path generation, or browser-only behavior is a renderer
extension or an explicit compatibility lane, not a hidden escape hatch in the theme schema.

Every new stable mechanism therefore needs all three proofs: a model-owned semantic binding, a
real family writer that emits it and returns a crate-private receipt, and target-specific evidence
that the final artifact consumes it. A validated data field without those proofs is descriptive
capacity, not supported rendering capability.

For authoring surfaces, the first stable contract remains a closed preset-or-complete-spec union.
`ThemeTokens` is the bounded convenience layer for applications that want a smaller vocabulary; it
materializes a complete spec before compilation. Its stable vocabulary should remain a small set of
cross-family roles such as canvas, surface, text, subtle text, border, line, accent, status colors,
series, and typography. Family-specific roles such as actor, note, activation, or cluster styling
belong in deterministic token-to-rule defaults or in the full `ThemeRuleSet`; they should not keep
expanding the stable convenience object. This contraction is an alpha migration and must retain a
design-system witness proving that common light/dark authoring remains ergonomic.

The exact token field set and expansion table are a C7a-candidate deliverable rather than an implied
implementation choice. It must enumerate each retained cross-family field, each removed alpha field,
its Rust and wire identifier, the deterministic `ThemeTarget`/variant/facet rules emitted from the
retained fields, and any built-in preset value that must move to an explicit `ThemeRuleSet`. The
current family-specific token fields remain alpha until that table and its binding migration tests
land; they are not silently renamed or mapped by separate hosts.

Preset patch/merge semantics are not part of the first stable binding or CLI contract. This avoids
making merge precedence a second theme language; a later typed preset-override layer requires a
separate product decision and its own deterministic materialization rules. The rollout must still
provide a read-only, versioned way to materialize each supported preset as a complete editable spec
through the native CLI and binding metadata. That export does not add patch semantics and records
the preset/schema version from which it was derived.

The built-in preset catalog is a convenience layer and reference implementation of the same
authoring contract, not Merman's primary brand or roadmap taxonomy. The catalog is not limited to
the current proof-theme names, and adding a high-quality low-cost preset is allowed when its recipe
is reusable and its maturity is reported honestly. Conversely, no milestone is complete merely
because a named theme exists or looks attractive; completion is measured by typed mechanisms,
family/target coverage, resource closure, and final admission.

Public maturity is explicit rather than inferred from where a symbol appears:

| Stage | Public promise |
| --- | --- |
| Internal proof | Recipe, receipt, and mechanism details may change without compatibility notice. |
| C7a candidate | Schema and selected presets are alpha discovery values; alpha preset IDs and completions are not stable enum or compatibility promises. |
| C7a rollout | First-party consumers exercise the exact alpha candidate and return a versioned, bounded execution-evidence summary. |
| C7a contract | Only the proven native recipe/compiler/token envelope and coarse report contract become stable. |
| C6b release | Individual presets may become stable only for their declared, runner-proven cells; an unqualified cross-target catalog claim requires the complete C6b gate. |

The fixed C6 matrices qualify the engine's shared mechanisms; they do not transfer a proof-theme
recipe fingerprint to an unrelated preset. A preset that seeks stable status therefore has a separate
`PresetQualificationSpec` and opaque `PresetQualificationReceipt`. That receipt runs the preset's own
recipe against its declared family/target cells and binds its recipe, document/resource, admission,
artifact, residual, font-source, and mechanism evidence. Preset qualification reuses C6 runners and
receipt types but does not add cells to the fixed 18/45 engine matrices.

A preset descriptor also binds an immutable recipe revision and recipe/resource fingerprint. Any
recipe or bundled-resource change creates a new revision, invalidates the prior qualification for
that revision, and returns the changed entry to alpha until its declared cells are requalified. The
catalog may retain an older revision only under an explicit compatibility/deprecation policy; a
stable ID never silently points at an unqualified recipe.

### C6 Contract Layers

C6 and preset qualification have five deliberately separate layers:

1. **`C6AcceptanceSpec`** is the final contract inventory. It contains the complete 45-cell
   theme/family/target expectation, including facet/value expectations, residual IDs, font source,
   admission, and artifact assertions.
2. **`C6EnforcedTranche`** is the subset currently executable in CI. It is a progress measure, not
   a release claim and cannot by itself unlock C7a. The initial tranche may grow in bounded steps as
   family and target runners land.
3. **Target proof receipts** are opaque inputs produced from sealed document and writer evidence,
   target admission, and final artifact bytes. The native receipt is Rust-owned. Browser SVG uses a
   dedicated non-public WASM operation and a versioned JSON receipt owned by `platforms/web`.
4. **`C6ObservedReport`** is constructed and evaluated only inside a non-published
   `merman-theme-acceptance` harness crate that depends on `merman`, `merman-export`, and
   `merman-theme-fixtures`. This avoids a dependency cycle while preventing ordinary callers from
   authoring conclusions. Every cell binds the source/input, recipe, document/resource, target
   receipt, proof-predicate, and final artifact digests. A caller cannot construct a passing
   observation by copying expectation fields.
5. **`PresetQualificationSpec`** is a per-preset declaration outside the fixed matrix. The private
   harness issues `PresetQualificationReceipt` only from observations of that preset's own recipe
   fingerprint and exact declared cells. Core C6 eligibility proves engine capabilities; preset
   qualification proves that a particular catalog recipe may make those bounded claims.

The current code is still an early matrix draft, not the final 45-cell contract described above:
the committed schema does not yet retain every selector/facet/value witness, and the four-cell
runner does not yet provide the 18-cell `C6aEligibilityReceipt`. The observation boundary itself
is now private to the non-published harness: fixture crates retain only catalog/specification
types, target adapters seal artifact digests and mechanism proofs, and unsupported enforced cells
return an explicit runner error instead of being silently skipped. The checkpoint remains
incomplete until the full C6a/C6b constraints are implemented.

`C6a` is the native contract-eligibility gate: the three proof themes must pass Flowchart, State,
and Sequence on Standalone SVG plus PNG as the named representative native target, and a cold-start
custom theme must prove that the public recipe is generative rather than only capable of reproducing
the reference corpus. The harness emits a dedicated `C6aEligibilityReceipt` only for the exact 18
positive cells plus the design-system, cold-start, and pre-freeze family-consumer witnesses; a
successful arbitrary tranche report is never equivalent. `C6b` is the cross-target release gate:
all 45 cells, including Browser SVG, must be enforced and observed by their target-owned runners
before stable cross-target preset/catalog claims are made. The 45 cells are exactly three proof
themes by three matrix families (Flowchart, State, Sequence) by five targets. Swimlane remains a
focused Flowchart-classification witness outside that Cartesian matrix. Residual cells remain useful
evidence, but residual-only progress never satisfies a positive C6 gate.

Each Browser SVG matrix cell aggregates a `BrowserEngineReceiptSet` rather than introducing an
engine dimension into the 45-cell matrix. The set contains exactly one versioned receipt for every
supported Chromium, Firefox, and WebKit lane. A missing, duplicate, unsupported, or failed lane
rejects that Browser SVG cell; the aggregate digest binds the sorted engine/version/receipt tuples.

### External Text and Safety Closure

The current release closes C4b only at the external-safety boundary: generation and effective-policy
binding, operation work/retained-response budgets, bounded attempts, and session invalidation are
required if external injection is enabled; external results remain `HostDependent`, and stable
external injection may remain unavailable. External `Portable` promotion, independent trust
authority, and browser/FFI assurance fields are a separate future product decision.

After every compatibility CSS or postprocessor stage, the terminal order is fixed: canonical
active-content and CSS mount-safety validation, then final resource-closure reconstruction, then
document residual merge and target admission. Safety failure rejects SVG and every native target;
`BestEffort` can preserve visual residuals but cannot bypass safety rejection. Resource deltas never
authorize scripts, event attributes, external loads, dangerous URLs, or unbounded `foreignObject`.

### Evidence And Admission Vocabulary

These stages use different vocabularies and must not be collapsed into one public `portability`
enum:

| Stage | Vocabulary | Meaning and next-stage mapping |
| --- | --- | --- |
| Family mechanism | `Applied`, `NotApplicable`, `Residual`, `Unadapted` | `Applied` is positive only with a real writer receipt. `Residual` and applicable `Unadapted` become document residuals. |
| Document proof | verified lanes plus named residuals | Any portability-relevant residual prevents a final `Portable` target result. Safety and unresolved resource closure reject independently. |
| Fixture expectation | `Portable`, `HostDependent`, `SvgOnly`, `Unverified` | This is corpus intent, not a runtime status. `SvgOnly` expects positive standalone/browser SVG consumption and native rejection. `Unverified` expects a document residual and strict rejection. |
| Final target admission | `Portable`, `HostDependent`, `Rejected` plus reasons | `SvgOnly` and `Unverified` never appear as final runtime statuses. They map to `Rejected` for unsupported or strict targets; `BestEffort` may still return an artifact only with a non-portable status and explicit residual/reason. |

Public presets declare per-target admission expectations and allowed residual IDs, not an unscoped
`portability grade`.

### Product Boundary Decision

- KTD14. **Keep built-in presets subordinate to the headless engine and on the same typed recipe
  path.** (session-settled: user-directed - chosen over either removing named presets entirely or
  turning named themes into Merman's main product taxonomy.) Merman may ship a bounded, open catalog
  of generally useful visual presets, including current or future candidates such as Brutalist,
  Spotless, Cyberpunk, or Modern Slate, but every preset is an ordinary `DiagramThemeSpec` recipe
  with the same capability, resource, evidence, and admission rules as user-authored themes. A
  preset cannot select layout, Mermaid look, trusted postprocessors, or family-private behavior.
  Headless Mermaid parsing, analysis, layout, and rendering remains the project identity; parity is
  the compatibility target and typed customization is the visual layer within it. Covers
  R2-R4, R33-R41, and the current C5-C7 preset/discovery sequencing.
- KTD15. **Advance only the published transports whose family-capability shape became
  incompatible.** (implementation-evidence-settled - chosen over assigning two incompatible
  required record shapes to epoch 3.) The alpha.5 UniFFI record and Web/WASM metadata payload
  exposed `logical_family_kind` and renderer-owned `render_model_kind`; the corrected contract
  exposes the core-owned `family_id` and removes renderer taxonomy. UniFFI and Web/WASM therefore
  advance to epoch 4. Native C ABI 3 remains valid because its generic metadata transport and
  function table did not change; runtime-catalog schema 1, Android transport 1, Typst ABI 2, and
  Node transport 1 also remain unchanged. Options schema 2 was unchanged by this family-catalog
  decision but is subsequently superseded by KTD16. This narrowly supersedes the August 2
  FFI-plan KTD9 after alpha.5 publication and governs the family-catalog contraction in C5 and its
  first-party binding rollout.
- KTD16. **Advance the post-alpha.5 Options grammar to schema 3 instead of reusing published
  schema 2.** (implementation-evidence-settled - chosen over a partial compatibility decoder that
  would advertise support without preserving the complete released grammar.) Alpha.4 and alpha.5
  schema 2 used `presentation`, `raster.background`, `pdf.background`, and general-binding raw CSS.
  The current contract replaces those with top-level typed `theme`, `raster.matte`,
  `pdf.page_paint`, and a trusted-host-only raw-CSS lane, so it is not wire-compatible. Runtime
  catalogs advertise only schema 3 and explicit schema-2 requests fail closed; omitted versions
  materialize to schema 3 for convenience callers. UniFFI/Web transport epoch 4, Native C ABI 3,
  runtime-catalog schema 1, Android transport 1, Typst ABI 2, and Node transport 1 remain unchanged
  because this is an Options epoch, not a transport-record change. This narrowly supersedes only
  KTD15's Options-schema-2 conclusion; KTD15's family-capability transport decision remains in
  force.

### Product Milestones

The remaining work is organized into three product milestones. These are delivery and review themes,
not a requirement to combine all listed work into three giant pull requests. Each milestone should
continue to land through small proof-first changes with local tests and Conventional Commits.

1. **Core theme-engine convergence.** Keep `DiagramThemeSpec` as the sole visual recipe authority;
   make the core Diagram Family catalog the sole family-ID and capability authority; complete direct
   State, Flowchart/Swimlane, and Sequence consumption for the routes selected by C6a; classify and
   retain every other bridge route until matching evidence or C7b removes it; repair
   residual/admission accounting; and preserve default Mermaid parity. Do not add brand onboarding,
   page-shell, animation, or other application product concerns while this milestone is open.
2. **Custom authoring and capability discovery.** Define a small alpha `ThemeTokens` convenience
   candidate plus the expressive `ThemeRuleSet`/`DiagramThemeSpec` path. Build a versioned internal
   descriptor for family, output target, semantic target, selector, facet, bounded support
   conditions, disposition, and relevant residual reasons. Publish `C7a-candidate` only after C1-C3, C4a, C5, C6a, and the pre-freeze
   family consumers can populate it truthfully; the contract becomes stable only after C7a-rollout
   verification. Built-in presets use this same interface and remain optional convenience entries.
3. **Integration seam and contract contraction.** Stabilize the unified `RenderedDocument`, recipe
   fingerprint, family/target admission, residual reasons, font/portability summary, and trusted
   `SvgPipeline` escape hatch. Contract the Rust facade, bindings, CLI, and Typst surfaces around
   compile, render, describe-support, and coarse reports. Family programs, adapters, evidence
   recorders, Prepared Text ledgers, and fine-grained mechanism DTOs remain implementation details.

Semantic SVG annotations are useful for downstream hover, links, tooltips, and host-owned motion,
but they are not a completion requirement for these three milestones. Any annotation experiment
requires a separate ADR and an opt-in, versioned schema. It must not promise whole-DOM stability;
the future ADR owns field selection, synthetic/one-to-many identity, source-element correlation,
and privacy policy rather than pre-freezing them here.

## Settled Architecture

### Identity

- Rename the current broad `ThemeFingerprint` concept to `ThemeRecipeFingerprint`.
- A recipe fingerprint identifies only a versioned canonical representation of visual intent and
  the exact retained resource identities. It does not prove host admission, family application, or
  target portability.
- Keep `FontCatalogFingerprint` as the content identity shared by prepared layout, sealed SVG, and
  native export.
- Do not publish a second opaque all-purpose runtime fingerprint. Runtime correlation is a
  structured report containing recipe, effective policies, family evaluation, prepared-layout
  evidence, document residuals, and target admission.
- Remove `Debug` text from fingerprint inputs. Use a versioned canonical encoder with explicit
  field tags, lengths, enum discriminants, normalized finite numbers, and deterministic map order.
- `DiagramTheme` equality must not silently mean both recipe equality and runtime-admission
  equality. Prefer explicit recipe fingerprint comparison; remove broad `PartialEq` if necessary.

### Monotonic Evaluation Chain

The positive proof path is explicit and cannot be inferred from an empty collection:

1. `Required`: mechanisms declared or inferred from the recipe.
2. `HostAllowed`: required mechanisms permitted by the compiling/rendering host ceiling.
3. `FamilyEvaluated`: applicable mechanisms are `Applied`, `NotApplicable`, `Residual`, or
   `Unadapted` for the selected family.
4. `DocumentResidual`: family, source-style, font/layout, trusted lane, postprocessor, and resource
   closure evidence is merged after the terminal SVG is known.
5. `TargetAdmission`: SVG, PNG, JPEG, and PDF independently decide `Portable`, `HostDependent`, or
   `Rejected` from the completed evidence.

Only an evaluated applicable mechanism with no residual can be verified. `Unadapted` is never
verified. A recipe with no mechanisms applicable to the selected family may be `NotApplicable`
without penalizing the unchanged default path.

### Policy Ownership

- `DiagramThemeSpec` and `DiagramTheme` own recipe content, exact resources, and requirements.
- `ThemeResourcePolicy` remains a compiler ceiling because hostile bytes must be bounded while
  decoding and canonicalizing.
- `RenderEnvironment` owns runtime `ThemeAdmissionPolicy`, `FontSourcePolicy`,
  `HostMeasurementFallbackPolicy`, `ThemePortabilityRequirement`, and trusted-lane ceilings.
- Session creation intersects renderer ceilings with recipe requirements and resource/source
  availability. A theme or request can only preserve or reduce a ceiling, never widen it.
- Compiler-side restrictions may be retained only as provenance of an earlier restriction and must
  be intersected again at session creation; they are not a replacement for environment policy.

### Prepared Text Boundary

- Catalog preparation and per-label preparation remain separate, fallible operations.
- External backends return a raw untrusted response DTO through a crate-private seam. Merman always validates the request binding,
  catalog identity, source/visible ranges, finite bounded geometry, and permitted font sources
  before constructing the internal immutable `PreparedText`. This basic admission is required even
  while the external result is reported as `HostDependent`.
- The built-in native catalog path is the first C4a producer that may claim `Portable`: its
  rustybuzz shaping, retained catalog faces, cluster/run evidence, and export font source are all
  owned by Merman. An external host result is `HostDependent` by default and cannot become
  `Portable` merely by echoing self-declared identity, capability, face, or source fields. The
  current release defines no external trust authority or `Portable` promotion; any such contract
  belongs to C7c.
- C4a's current native terminal binding seals the exact tokenized artifact with request, projection,
  line, run, and face/source identity; the exporter independently verifies the artifact, final
  emitted line text, and observed face/source facts. The ordered source/visible range identity is
  renderer-sealed rather than reconstructed from glyph callbacks. External results remain
  `HostDependent`, and exact terminal per-run range proof for multi-face fallback remains a C6b
  gate.
- A prepared result carries source-to-visible cluster ranges so transforms, entities, bidi runs,
  and fallback runs do not depend on matching transformed strings back to original words.
- Shape each source run once for initial advances. Wrapping uses retained cluster advances or prefix
  widths and must not reshape every growing prefix; each emitted line may perform one bounded final
  constrained reshape when line context changes shaping.
- Runtime fallback is session-owned and ordered for the two real implementations that exist today:
  the configured host adapter and the built-in native catalog adapter. Host rejection, timeout,
  invalidation, and missing glyph may advance to the next allowed candidate. A custom catalog can
  never fall back to vendored default metrics. Do not expose a general multi-backend graph until a
  second non-native production adapter exists.
- The C4b external seam includes the safety needed by any renderable host result: a Merman-issued
  per-session generation binding, request/effective-policy binding, operation-level call/work and
  retained-response budgets, bounded candidate attempts, and session-local invalidation after
  protocol or budget violations. A same-process Rust callback remains trusted blocking host code;
  forced preemption belongs to an actual process/worker/async adapter boundary and must not be
  simulated by an ineffective logical timeout.
- Keep the external DTO/trait crate-private throughout the current release. Do not expose it through
  stable JSON/FFI bindings. Document that an enabled adapter receives diagram label text plus
  bounded catalog metadata/resources needed for shaping; do not cache raw labels across operations
  or tenants, isolate session state, bound invalidation, and keep raw labels and host font paths out
  of diagnostics. Any remote or cross-process data contract belongs to C7c.
- Unmigrated custom-catalog label paths fail with a typed error. They never silently use a legacy
  profile that measures a different font.

### Family Scope And Rule Evaluation

- The final global Mermaid compatibility config consumes only explicit `spec.mermaid()` values.
  Semantic rules, palettes, canvas, effects, and typography must not infer global
  `themeVariables`.
- Family-scoped semantic rules are consumed only by the selected family adapter after detection;
  they cannot synthesize global `themeVariables` for unrelated families.
- Preserve current rendered behavior during migration through one crate-private
  `LegacyFamilyThemeBridge`. It runs only after family detection, sits below explicit Mermaid/site/
  source config in precedence, and records `LegacyCompatibility` evidence whenever it contributes.
  A bridge result is never typed `Applied` evidence and cannot satisfy `RequirePortable`.
- Migrate the bridge in dependency order: disable it for State first, then Flowchart, then Sequence.
  Remove the temporary default-typography bridge only after Flowchart and Sequence both consume
  resolved typography directly in layout and emission. Do not replace the bridge with per-theme
  adapters.
- Maintain an internal support matrix with exactly three states per family mechanism:
  `TypedAdapter`, `LegacyCompatibility`, or `Unsupported`. Existing broad projection tests prove
  compatibility behavior only; they are not positive typed-adapter or portability evidence.
- Map the static matrix to runtime evidence monotonically: `TypedAdapter` may produce `Applied`,
  `NotApplicable`, or `Residual` only from its real consumer; `LegacyCompatibility` always produces
  a compatibility residual and never `Applied`; `Unsupported` becomes `Unadapted` when applicable
  and `NotApplicable` only when the document contains no matching target.
- Compile exactly one `FamilyThemeProgram` per selected family. Typed adapters and the temporary
  bridge may read its already-resolved property winners and provenance; neither may rescan the spec,
  rematch selectors, or implement another fallback/cascade interpreter.
- Limit compatibility preservation to a named, versioned fixture corpus. Freeze bridge mappings:
  no new theme mechanism may enter the bridge, and each retained mapping has an owning cutover gate.
- Theme rules, selectors, ordinal palettes, and dynamic matches receive explicit resource limits.
- Compilation builds a target/family/variant index and premerges static declarations. Dynamic
  ordinal matching is bounded and charged to the operation work meter.

## Scope Freeze

The freeze is stage-specific rather than one condition that expires too early:

- **While C4a terminal proof is reopened:** preserve the completed prepared-text core, but do not
  expand its DTO, backend graph, or assurance vocabulary. Crate-private Sequence work may continue;
  presets, public evaluator internals, stable binding schema fields, CLI theme flags, and public
  family adapters cannot freeze early. External backend injection remains disabled in stable paths.
- **Before C5/C6a close:** State, `{Flowchart, Swimlane}`, and Sequence cutover work remains
  crate-private. C5 may prepare and internally cut over Sequence; only C6a may claim positive
  Sequence output evidence or a settled shared family interface.
- **Before any external consumer exists:** keep external injection disabled in stable product
  surfaces and do not expand C4b beyond existing basic admission. If a concrete crate-private
  consumer is later enabled, it must first add generation/policy replay binding, operation budgets,
  bounded attempts, and invalidation. No external result may be called `Portable` in this release.
- **Until a real consumer runs:** do not claim Canvas/Effects capabilities are admitted merely
  because their data model validates, and do not promote modern theme showcases from “describable”
  to “rendered” evidence.
- Only renderer-owned, closed, versioned postprocessors may carry a preservation contract. Raw CSS
  and public custom postprocessors can never self-certify typed evidence; portability downgrade does
  not replace the independent SVG/CSS mount-safety decision.
- Keep the official `mmdc` compatibility surface unchanged except for fixes required by removed
  provisional alpha APIs.

## Product Scope Boundaries

### In Core

- Headless Mermaid parsing, analysis, semantic/editor models, layout, SVG generation, and native
  export.
- Typed theme compilation from Rust, JSON, and supported bindings through one recipe authority.
- Private family adapters, capability discovery, evidence, residuals, resource closure, and
  target-specific admission.
- A bounded built-in preset catalog that materializes ordinary typed recipes and adds no parallel
  renderer, hidden behavior selection, or material dependency burden without explicit review.
- A trusted Rust/native host escape hatch through `SvgPipeline` and postprocessors; using it
  downgrades or revalidates evidence according to the terminal safety and admission rules.

### Outside Core Product Identity

- Website or brand scanning, automatic palette/font extraction, and brand onboarding workflows.
- Editorial content deletion, audience rewriting, diagram-type selection, or automatic story design.
- Titles, eyebrow text, summary cards, screenshots, captions, and page/figure shell composition.
- Bespoke manual layout, orthogonal routing as an aesthetic service, icon art direction, and an
  animation or presentation playback state machine.
- A promise to ship a particular number or fixed list of named themes. The catalog remains bounded
  by usefulness, evidence, licensing, and cost rather than by a branding quota.

Downstream products may build all of these capabilities around Merman's typed inputs, sealed output,
coarse reports, and optional future semantic annotations. They do not require the core renderer to
own those product decisions.

## Implementation Checkpoint - 2026-08-13

| Gate | Status | Current evidence and remaining boundary |
| --- | --- | --- |
| C0 | Complete for the bounded Flowchart tranche | Prepared SVG labels retain one admitted text result through layout and emission; unsupported label modes fail closed. |
| C1 | Complete for the internal recipe domain | Recipe, font catalog, and SVG resource identities are separate and canonically encoded. Hidden legacy recipe state has been removed; public evaluator exposure remains a C7 decision. |
| C2 | In progress | Root/family reports, trusted-lane residuals, sealed document reports, consuming SVG admission, and target-specific prepared PNG/JPEG/PDF exports exist. State cascade evidence now attributes only final property winners. The terminal SVG consumer proves solid root base/layers (including explicit transparent base, opacity, placement, and blend declarations); native PNG/JPEG/PDF admission uses an explicit root-capability whitelist rather than assuming exporter support. Gradients, patterns, bleed/viewport expansion, effects, and complete emission-owned family evidence remain residual. Any untrusted postprocessor or raw theme CSS invalidates positive root evidence before document/target admission. |
| C3 | Implementation landed; verification retained | Binding, CLI, and Typst now derive host resource ceilings before theme decoding/compilation, and request themes can only restrict them. Keep the gate open until the named cross-host verification commands are signed off. |
| C4a | Native core and single-face terminal binding landed; broader gate remains open | Native rustybuzz shaping, cluster fallback, projection, bounded wrapping, consumed-label sidecars, and sealed label tokens are retained. The exact tokenized artifact is bound to request/projection/line/run identity, while PNG/JPEG/PDF exporters verify final emitted line text and face/source observations. External results remain `HostDependent`; exact multi-face source/visible range proof remains in C6b. |
| C4b | Conditional follow-up; assurance excluded | Session-private fallback candidates and basic response admission may remain, but no current product consumer requires the unfinished generation, budget, or circuit-breaker framework. Keep stable injection disabled. Implement those controls only before enabling a concrete crate-private external consumer. External output remains `HostDependent`; any later assurance or promotion requires C7c. |
| C5 | In progress; program stages 1-2 converged, stage 3 partial; catalog authority open | Explicit Mermaid compatibility now comes only from `spec.mermaid()`. The transitional bridge is family-local, runs after detection, preserves explicit site/source/detector ownership, and records compatibility residuals that strict portability rejects. `FamilyThemeProgram` owns the recipe, premerges static rules, retains source-order winners, meters only ordinal candidates, and now carries a private facet-level `TypedAdapter` / `LegacyCompatibility` / `Unsupported` route matrix. The bridge can read only route-approved legacy winners and never turns the matrix into positive evidence. State consumes the metered program directly; Flowchart and Swimlane already consume a narrow typed Node/NodeLabel tranche while retaining bridge routes for uncovered mechanisms; Sequence now directly consumes selected Actor paint routes with post-emission evidence, while signal, note, activation, loop, message, typography, and other label routes remain on the bridge. The core family catalog still must become the sole family-ID/alias/detection authority before facet discovery; final surviving compatibility provenance and parse/session theme binding remain convergence gates. |
| C6a | Partial infrastructure; native eligibility gate not closed | The acceptance specification and current four-cell State tranche run through a private harness with render-group identities, per-cell target receipts, artifact/proof digests, exact enforced-cell coverage, and explicit feature gating. Fixture admission now binds the enforced cell to its reference theme and exact typed mechanism inventory. The runner remains one real Brutalist/State adapter rather than the final registry, and no exact 18-cell eligibility receipt exists: Flowchart/State/Sequence by Standalone SVG/PNG, selector/facet/value writer witnesses, source-style/spacing, repeating-gradient/effect shapes, design-system, cold-start, and real pre-freeze family consumers remain open. |
| C6b | Not started as a release gate | Browser SVG, JPEG/PDF completion, multi-face export, and the full 45-cell target-owned evidence matrix remain open. |
| C7a | Not eligible; intentionally blocked | Do not declare the alpha contract candidate before C1-C3, C4a, C5, and C6a close plus the pre-freeze family consumers. Do not freeze the contract until C7a-rollout verification passes. Stable cross-target preset claims wait for C6b. |
| C7b/C7c | Deferred independently | Remaining family/preset/showcase breadth proceeds under C7b without waiting for external assurance. External-host assurance fields remain a separately triggered C7c plan. |

The public alpha migration scaffold already spans Rust, Options JSON, Web/UniFFI, Typst, Playground,
and native/compatibility CLI entry points. That scaffold is not a frozen promise and must contract to
the staged C7a boundary. The provisional `mmdc` flags and ordinary Typst raw-CSS inputs are removed;
native CLI selection remains the trusted lane, generated Python/Web/options resource contracts have
been refreshed, and further breaking alpha corrections remain allowed before C7a-contract. The next
contraction tranche makes the core family catalog the only family-ID authority and removes bindings
and public theme scope from the independent renderer taxonomy before adding facet discovery.

## Implementation Units

### C0. Preserve And Bound The Current Prepared Flowchart Tranche

- **Covers:** R1, R12-R16a, R43, R45.
- **Approach:** Retain the catalog/session split, structured typography, native mixed-script
  shaping, and family-owned SVG-label sidecar. Keep the default vendored/host callback path
  unchanged. Record SVG-only support explicitly; HTML/Markdown paths remain typed failures until
  C6a. Do not copy the current provisional result contract into State or Sequence.
- **Files:** `crates/merman-render/src/text/prepared.rs`,
  `crates/merman-render/src/flowchart/label.rs`,
  `crates/merman-render/src/flowchart/svg_label_artifact.rs`,
  `crates/merman-render/src/environment.rs`, `crates/merman-render/src/family.rs`.
- **Test scenarios:** mixed Latin/CJK catalog; layout and SVG reuse one result; no legacy callback
  on the prepared path; host callback order unchanged on the default path; entities, explicit
  breaks, long words, unsupported tags, missing glyphs, and mismatched catalog evidence.
- **Done when:** The current vertical slice is test-backed and all known unsupported paths fail
  closed without creating a false family-wide support claim.

### C1. Separate Recipe Identity From Runtime Evidence

- **Covers:** R2, R24-R28, R44, R47-R49.
- **Approach:** Introduce `ThemeRecipeFingerprint` and a canonical encoder. Remove `Debug` hashing,
  duplicate derived-config inputs, and fingerprint-only whole-object equality. Keep catalog and SVG
  resource fingerprints separate. Replace compile-time “resolution” naming with recipe/requirement
  naming; add structured operation identity only where a real consumer needs it.
- **Files:** `crates/merman-render/src/diagram_theme/compiler.rs`,
  `crates/merman-render/src/diagram_theme/mod.rs`, diagram-theme model modules,
  `crates/merman-render/src/environment.rs`, `crates/merman-render/src/family.rs`,
  `crates/merman/src/svg/operation.rs`.
- **Test scenarios:** semantically identical recipes produce the same fingerprint; field/map input
  order is irrelevant; every semantic field changes the fingerprint; resource bytes change only
  the appropriate identities; differing runtime ceilings do not alter recipe identity but do alter
  structured session evidence; no API treats the recipe ID as target admission.
- **Done when:** Every fingerprint has one documented content domain and no positive runtime claim
  is encoded only as hash equality.

### C2. Make Capability And Family Evaluation Explicit

- **Covers:** R17-R24, R26-R28, R41, R46-R49.
- **Approach:** Replace compile-time `admitted` with `required` and `host_allowed`. Introduce explicit
  family evaluation states and separate semantic-theme evaluation from source-style residuals.
  Evaluate root canvas/effects independently from family styles. Freeze positive root evidence only
  in the common terminal SVG consumer, then propagate the report into the completed document and
  target admission. Treat raw theme CSS and postprocessors as mutation boundaries unless they carry
  an explicit preservation contract; a mutation must monotonically downgrade root evidence.
- **Files:** `crates/merman-render/src/diagram_theme/compiler.rs`,
  `crates/merman-render/src/family.rs`, `crates/merman-render/src/svg/pipeline/mod.rs`,
  `crates/merman/src/svg/operation.rs`.
- **Test scenarios:** LayeredCanvas/SvgFilter/Noise cannot be reported applied without consumers;
  solid and explicit transparent root base/layers are marked applied only for capabilities actually
  emitted; native PNG/JPEG/PDF admission rejects unproved root capabilities and admits only the
  explicit solid/transparent/layer/placement/blend/opacity set; untrusted CSS or postprocessors
  invalidate prior root evidence; unadapted Sequence/Flowchart styles are not verified; empty/default
  recipes are NotApplicable; State valid styles are evaluated; State invalid styles retain residuals
  through terminal SVG; RequirePortable rejects every applicable unadapted or residual mechanism
  after every output stage.
- **Done when:** Empty residuals cannot manufacture positive evidence and document reports expose
  every stage needed for target admission.

### C3. Move Effective Admission To Render Sessions

- **Covers:** R4, R9-R15, R24, R27-R32, R42-R49.
- **Approach:** First derive one `ThemeResourcePolicy` compiler ceiling from every host-selected
  resource profile before parsing theme JSON or decoding assets. Binding constructors retain that
  host ceiling; request replacement/overlay compilation can only call `restrict_with`. CLI derives
  the same ceiling from its resolved profile, and Typst applies the constrained theme-input ceiling
  before full deserialization. Then add all runtime ceilings to `RenderEnvironment`; intersect them with recipe
  requirements, source availability, and any prior compiler restriction when beginning a session.
  Remove theme getters that imply a recipe owns host policy. Thread the effective immutable policy
  through family preparation, sealed SVG, document report, and exporter admission.
- **Files:** `crates/merman-render/src/diagram_theme/admission.rs`,
  `crates/merman-render/src/diagram_theme/compiler.rs`,
  `crates/merman-render/src/diagram_theme/mod.rs`, `crates/merman-render/src/environment.rs`,
  `crates/merman-render/src/family.rs`, `crates/merman-render/src/svg/pipeline/mod.rs`,
  `crates/merman/src/svg/mod.rs`, `crates/merman/src/svg/operation.rs`, host Binding/CLI/Typst
  constructors, generated Python package inputs/outputs, Web public theme types, and Options JSON
  resource documentation.
- **Test scenarios:** constrained Binding/CLI/Typst inputs reject oversized encoded themes,
  base64/font assets, and effect graphs before interactive limits allocate or compile them;
  embedded-only environment cannot be widened by a theme; strict portability
  cannot be weakened; trusted lanes are monotonic; empty intersections are typed failures before
  layout; selected-theme resources override environment resources while policies intersect;
  renderer setter order does not change effective policy.
- **Done when:** No host parses hostile theme bytes with a compiler ceiling broader than its selected
  profile. Session creation is the only place that produces effective runtime admission and every
  downstream consumer reads the same frozen result. The complete Python package is regenerated,
  Web reuses the shared 11-theme name type, Options JSON documents every resource ceiling, and
  freshness tests reject future authority/generated drift.

### C4a. Finish The Deterministic Native Prepared Text Core And Terminal Binding

- **Covers:** R12-R16a, R19-R19a, R43, R45, R49 for the Merman-owned native catalog path.
- **Approach:** Make the native catalog path the first-class deterministic producer. Add
  request-derived native limits, source/visible cluster ranges, native cluster-level fallback,
  actual face/source usage, and retained-advance wrapping followed by a bounded final-line reshape
  where line context can change shaping. Route layout, SVG, PNG, and PDF through the same native
  prepared result. Keep every external trait/DTO and backend injection point crate-private until
  C4b. Treat the session-level prepared-layout report as backend summary only: family sidecars
  allocate operation-local deterministic label keys, mark evidence consumed only from real SVG
  emission, and freeze the consumed ledger after successful family rendering. Carry that read-only
  ledger on the sealed `ResvgCompatibleSvg` so the native exporter verifies the same artifact it
  parses instead of reconstructing family ownership from global session counters.
- **Files:** `crates/merman-render/src/text/prepared.rs`,
  `crates/merman-render/src/environment.rs`, Flowchart and State prepared-label sidecars,
  `crates/merman/src/svg/operation.rs`, `merman-export` font and final-text evidence, provisional
  host measurement adapters, and `fuzz/fuzz_targets/theme_text_layout.rs`.
- **Test scenarios:** native custom-font success; mixed Latin/CJK/combining/ZWJ coverage; transforms
  and explicit breaks; invalid ranges and metrics; request-derived byte/record/geometry budgets;
  native missing-glyph fallback; cmap-positive but shaped-`.notdef` rejection; variation selector,
  ZWJ, and complex-script positive/negative coverage; linear-work wrapping; the same per-label
  run/face/source ledger (including source/visible ranges) retained by layout and SVG, with the
  native exporter validating the exact token-associated artifact, emitted line-text digest, and
  final face/source evidence while the renderer seals request and ordered-range identity. Include
  negative cases where text or run order changes while the line count and face set remain unchanged;
  exact multi-face terminal range observation remains a C6b responsibility.
- **Done when:** No geometry reaches layout without basic request admission, the native path can
  produce single-face `Portable` evidence from one retained catalog only after final shaping has
  rejected every `.notdef` cluster/run, native target admission verifies the exact token-associated
  artifact, emitted text, renderer-sealed request/ordered-range identity, and face/source evidence,
  and at least a second crate-private family consumer can use the result without widening its
  interface. Exact multi-face fallback byte-range evidence remains a C6b follow-up requiring a
  maintained `usvg` hook; it is not inferred from resolver callback order or glyph text search.

### C4b. Conditionally Close A Crate-Private External Host Safety Boundary

- **Covers:** R13's crate-private renderable external `HostDependent` path and the safety portions of
  R43/R45. Family typography and long-label semantics remain owned by C5/C6a. Stronger host
  assurance, stable browser/FFI projection, remote transport, and `Portable` promotion are excluded
  and belong only to C7c.
- **Approach:** Preserve only already-working session-private fallback/admission infrastructure and
  do not implement the remaining framework until a concrete crate-private consumer needs to execute
  an external result. Before enabling any external backend as a product path, add a Merman-issued
  generation and effective-policy
  binding, operation-level call/work/retained-response budgets, bounded candidate attempts, and
  session-local invalidation after protocol or budget violations. The raw response DTO,
  capability/face/run evidence, bounded diagnostics, and two-candidate fallback remain private
  implementation details. The coordinator must not become a public arbitrary-backend graph, and
  C4b does not define `ExternalAssured`, a trust authority, or a stable transport.
- **Files:** `crates/merman-render/src/text/prepared.rs`,
  `crates/merman-render/src/environment.rs`, binding/FFI adapters, exporter reports, and fuzz
  targets.
- **Test scenarios:** replayed generation; operation-budget exhaustion; repeated protocol or budget
  violations and candidate circuit breaking; capability and face-evidence mismatch; stale
  session/request binding; host rejection/timeout/missing glyph; policy ordering between native and
  accepted host-dependent results; strict portability rejection; actual per-label fallback usage;
  bounded diagnostics and fuzz smoke.
- **Done when:** With injection disabled, the current release is complete without further C4b work.
  If a concrete consumer enables injection, every enabled external call is operation-bounded and
  generation-bound, every fallback outcome is represented by a bounded `(from, to, reason)`
  histogram plus candidate
  invalidation generation, protocol/budget violations trip session-local invalidation, and no
  public surface exposes the external seam. All accepted external results remain `HostDependent`;
  stable product surfaces may keep injection unavailable.

### C5. Compile Family-Scoped Theme Programs

- **Covers:** R3-R8, R17-R24, R33, R38-R41, R46.
- **Catalog authority tranche:** Make the descriptor in `merman-core` the sole declaration of built-in
  family IDs, aliases, detection, semantic ownership, and public capability identity. Keep any
  renderer taxonomy private and derive or validate its adapter mapping against that descriptor;
  bindings and public theme scope parse catalog IDs rather than the independent render enum. Delete
  duplicated ID/alias lists instead of adding a third synchronization layer. The primary files are
  `crates/merman-core/src/family.rs`, `crates/merman-render/src/render_family.rs`,
  `crates/merman-bindings-core/src/theme.rs`, and the family metadata/discovery projection.
- **Approach:** Converge in four internal stages rather than deleting the current projection in one
  step:
  1. **Complete.** Split the compiled theme into a thin explicit Mermaid compatibility config
     derived only from `spec.mermaid()` and a clearly named transitional
     `LegacyFamilyThemeBridge`.
  2. **Complete.** Add a post-detection/pre-family-parse config hook and preserve configuration
     provenance through
     `merman-core` parsing metadata. The facade supplies a read-only `FamilyCompatibilityOverlay`;
     `merman-core` remains generic and does not depend on render-theme types. Apply layers in the
     tested Mermaid order without letting bridge defaults override explicit `spec.mermaid()`,
     renderer/site, frontmatter, or init directives; emit `LegacyCompatibility` residual evidence
     directly from overlay provenance rather than inferring it from final values. Strict portability
     rejects that evidence.
  3. **Partial.** Compile semantic rules into the single bounded `FamilyThemeProgram`, premerge
     static matches, meter the remaining ordinal work, and cut over State, `{Flowchart, Swimlane}`,
     and Sequence in that order. The program now owns the exact recipe and compiles each applicable
     base-typography property, rule facet, ordinal palette, and effect binding into one private
     `TypedAdapter`, `LegacyCompatibility`, or `Unsupported` route. The bridge may consume only
     route-approved legacy winners. This matrix assigns responsibility; it never manufactures
     runtime `Applied` evidence. The program and State cutover are complete; the other primary
     families have partial direct slices while their uncovered mechanisms remain on explicitly
     residual bridge routes.
  4. Remove the default-typography path from the bridge only after direct Flowchart/Swimlane and
     Sequence layout/emission consumption is proven. Keep the bridge for every remaining
     `LegacyCompatibility` mechanism until its C6a terminal evidence authorizes route deletion or a
     later family-breadth plan explicitly drops that mechanism.
- **Files:** `crates/merman-render/src/diagram_theme/mermaid_compatibility.rs`,
  `crates/merman-render/src/diagram_theme/semantic.rs`,
  `crates/merman-render/src/diagram_theme/resolved.rs`, compiled-theme/session internals,
  `crates/merman-core/src/lib.rs`, `crates/merman-core/src/parse_pipeline.rs`, parsed metadata,
  `crates/merman/src/svg/mod.rs`, family style adapters, `crates/merman-render/src/resources.rs`.
- **Test scenarios:** family-scoped rules do not leak into unrelated Mermaid config; rule order and
  precedence remain deterministic; rule/selector/palette limits reject amplification; large
  node/rule products consume bounded work units; explicit Mermaid compatibility retains official
  mmdc layering; bridge output never counts as typed `Applied` or portable evidence; State,
  Flowchart, Swimlane, and Sequence each pass with their relevant bridge mechanisms disabled;
  default no-theme Mermaid output remains unchanged. Add cross-crate catalog tests proving every
  public ID and alias resolves to one core descriptor, detection selects that descriptor, every
  render adapter maps to a declared descriptor, bindings emit the same IDs, and no independent
  render/binding list can drift.
- **Done when:** The core catalog is the only family-ID/alias/detection authority; renderer adapters
  and binding discovery project from or are exhaustively checked against it. One selected family consumes one bounded compiled program without global
  semantic-rule projection or unmetered node-by-rule scans, the primary family tranche has direct
  family-local consumers and tests for its selected routes, and every mechanism of every remaining
  family is explicitly classified as `TypedAdapter`, `LegacyCompatibility`, or `Unsupported`.
  Terminal bridge-route deletion remains a C6a responsibility.

### C6a. Complete The Native Contract-Eligibility Slices

- **Covers:** The native portions of R16a, R17-R24, R29-R32, R38-R41, and R44-R49.
- **Approach:** Complete Flowchart, State, and Sequence before any other matrix family. Swimlane is a
  focused Flowchart classification witness, not a fourth Cartesian family. Use three current
  mechanism-distinct proof recipes: Brutalist for ordinal/stroke/geometry, Spotless for typography,
  dash, and repeating-gradient behavior, and Cyberpunk for layered canvas/blend/effects. These names
  select useful coverage diversity; they are neither the Merman product taxonomy nor a closed preset
  roadmap. Any of them may later become a built-in preset through the same maturity gate, and later
  generally useful low-cost presets may be admitted without changing the C6 matrix shape. Add an
  explicit bounded gradient repetition/tile geometry shape with canonical encoding, binding shape,
  SVG lowering, native admission, and writer receipt; the current gradient stop model and fixed
  pattern primitives cannot express the Spotless reference. Add the smallest bounded effect
  consumer and paint-bounds/resource closure needed by Cyberpunk. Broader Canvas/Effects stay C7b.
- **Evidence ownership:** Add non-published `merman-theme-acceptance`. It exclusively constructs
  observed cells from opaque native receipts and evaluates the fixture inventory. Replace public
  conclusion constructors with crate-private receipt inputs. Each predicate binds source/input,
  recipe, target/family/selector/facet/value, document/resource, admission, and artifact digests.
  The runner dispatches from the enforced catalog rather than a hard-coded State list. The C6 Rust
  test declares explicit PNG/JPEG/PDF required features, and CI invokes the named C6 command instead
  of relying on workspace feature unification.
- **Test scenarios:** Enforce the exact 18 cells for Brutalist, Spotless, and Cyberpunk by Flowchart,
  State, and Sequence by Standalone SVG and PNG. Every slice covers custom fonts, semantic paint,
  layout-changing typography and spacing, source-style evaluation, SVG resources, and terminal
  document evidence. Add focused Swimlane classification; light/dark readability; font clipping;
  long mixed-script labels; repeating/tiled versus ordinary gradients; blend values; one bounded
  effect graph; `RequirePortable` and `BestEffort`; and default parity snapshots. HTML/Markdown are
  separate bounded label-mode witnesses or explicit residuals, not a Cartesian multiplier.
- **Design-system witness:** Build light/dark `ThemeTokens` from one host-resolved token set, reuse
  each compiled theme across independent renderers, and prove Flowchart, State, and Sequence SVG plus
  PNG without state leakage. This witness uses only `ThemeTokens` as caller input and verifies the
  deterministically materialized spec; it cannot inject family-specific rules to compensate for a
  missing convenience token. The full-spec cold-start witness remains separate.
- **Non-blocking mechanism witnesses:** Ghibli, Hand Drawn, Memphis, and later reference recipes may
  exercise distinct mechanisms without counting as matrix cells or participating in
  `C6aEligibilityReceipt`. They remain C7b inputs unless explicitly promoted into the canonical
  18-cell tranche by a later plan revision.
- **Cold-start authoring witness:** Build one theme that is not derived from `modern_mermaid` or PR
  #28 using only the candidate `ThemeTokens` and `DiagramThemeSpec`. It combines common and
  family-scoped semantics without raw CSS or a new renderer primitive and passes Standalone SVG plus
  PNG across Flowchart, State, and Sequence.
- **Done when:** The private harness emits `C6aEligibilityReceipt` only after all 18 cells plus the
  design-system, cold-start, and real pre-freeze family-consumer witnesses pass. A smaller successful
  tranche cannot generate this receipt or unlock C7a.

### C6b. Complete The Cross-Target Release Matrix

- **Covers:** Browser SVG, JPEG/PDF, multi-face export, and stable cross-target claims from the same
  R/AE set. C6b does not block C7a's native contract candidate.
- **Approach:** Extend the same acceptance inventory to all 45 cells: three proof themes by
  Flowchart/State/Sequence by Standalone SVG, Browser SVG, PNG, JPEG, and PDF. Complete the maintained
  `usvg` hook that binds terminal per-run face and exact cluster/source ranges so mixed-script
  multi-face `FullFont` output can become `Portable`; until then public claims remain single-face.
  Before implementing that gate, record the workspace-pinned `usvg` fork/revision, the exact
  per-glyph/cluster byte-range and face API it exposes, `[patch.crates-io]` integration, license
  compatibility, and the owner responsible for upstreaming or rebasing the patch. An unowned local
  hook is not an acceptable release dependency.
- **Browser evidence:** A dedicated non-public WASM operation returns the sealed SVG/report envelope.
  The Web runner mounts it in a scriptless sandboxed opaque-origin iframe with restrictive CSP, no
  top navigation, and no external network access. Its versioned JSON receipt binds source/input,
  recipe, document/resource, mount-policy, target, final artifact, and post-mount DOM digests. It
  records browser engine/version. A mechanism is `Applied` only with an engine-specific computed-
  style or tolerance-bounded pixel proof. Each Browser SVG cell produces the exact
  `BrowserEngineReceiptSet` defined above; an unqualified claim requires its Chromium, Firefox, and
  WebKit receipts to pass and aggregate without missing or duplicate lanes.
- **Done when:** The final 45-cell specification has no deferred cell, every observation was produced
  by its target-owned runner and private harness, multi-face export has exact terminal range proof,
  and the stable preset/catalog claims state their supported engines and cells. Residual results stay
  recorded but cannot close the positive gate.

### C7a. Freeze And Roll Out The Native Theme Product Surface

- **Covers:** Native-theme Rust/API, Options, binding, CLI, Typst, and design-system portions of the
  remaining R/AE items.
- **Pre-freeze family consumers:** Before declaring a contract candidate, run one source-backed
  representative from each remaining semantic shape category: relation-centric (`Class`),
  entity/card (`ER`), interval/lane (`Gantt`), quantitative chart (`Pie`), and spatial/container
  (`Architecture` or `C4`). For each, record model-owned targets, variants, clear/none semantics,
  unique geometry/typography/effect facets, and whether the versioned envelope can express them
  without a new public sum-type shape. For each shape category, at least one minimal direct typed
  family writer must consume the candidate shape through compiler and Standalone SVG terminal
  evidence. A model-only or runtime-residual probe cannot freeze that category. Findings and owner
  decisions live in the internal family mechanism matrix and committed fixtures/reports. Any missing
  public shape returns the plan to C1/C5 before rollout.
- **Approach:** Use three ordered gates rather than freezing before consumers can exercise the
  candidate. `C7a-candidate` follows C1-C3, C4a, C5, C6a, and the pre-freeze family consumers; it
  declares the proposed versioned recipe envelope, compiler entry point, `ThemeTokens`, host-policy
  boundary, and coarse reports, but remains alpha. `C7a-rollout` then migrates Options JSON, bindings, Typst,
  examples, migration docs, and Merman native render/batch CLI pages/completions against that exact
  candidate. `C7a-contract` freezes only after rollout verification passes. Semantic target/variant
  discovery remains additive and versioned, while per-mechanism ledgers stay private or alpha. The
  official `mmdc` compatibility surface remains unchanged except for removed provisional alpha
  fields. Do not expose C4b external fields. Remove and reject `svg.scoped_css` from ordinary Options
  JSON, aliases, site/source config, and general bindings. Raw CSS remains available only through a
  host-constructed trusted Rust/native CLI capability and still cannot bypass terminal safety.
  Do not bulk-import the complete 24-theme modern_mermaid reference catalog into supported presets.
  Individual Merman-owned recipes may join the open preset catalog when they satisfy the same typed
  recipe, licensing, resource-cost, maturity, and evidence rules as every other candidate.
- **Capability-discovery candidate:** Compose one versioned descriptor from the core family catalog
  and the private C5 mechanism matrix. The core catalog owns family IDs and aliases; family writers
  and terminal evidence own selector/facet disposition and residual facts; the facade only projects
  the bounded result. Cover family, output target, semantic target, selector, facet, disposition,
  and relevant residual reason IDs. A facet support entry is `Unconditional`, `Conditional`,
  `Unsupported`, or `Unverified`; a `Conditional` entry carries only bounded, versioned value
  classes, label modes, resource requirements, or host-policy predicates proven by C6. Conditions
  that cannot be represented in that closed shape remain `Unverified`. The final `RenderReport`,
  not discovery, remains authoritative for a concrete request. Rust, JSON metadata, Web/Node, UniFFI, C FFI, and Typst-facing
  metadata must be generated or verified from the same authority. Older response consumers ignore
  unknown additive descriptor rows/fields; execution requests reject unknown family, target,
  selector, or facet IDs. Keep writer receipts and per-element ledgers private.
- **Existing preset migration:** Treat `editor-light`, `editor-dark`, `one-dark`, `gruvbox-light`,
  `gruvbox-dark`, `ayu-light`, and `ayu-dark` as the initial alpha inventory rather than inherited
  stable IDs. Before C7a-contract, each receives an explicit retain/remove decision and an alpha
  descriptor with immutable recipe revision/fingerprint, schema version, and currently proven cells.
  All first-party bindings expose the same inventory and maturity. A preset ID does not become a
  stable compatibility promise until the stable catalog gate after C6b, even when it remains usable
  during alpha; any later recipe/resource change invalidates that revision's qualification as defined
  above.
- **Preset materialization contract:** `merman-bindings-core` owns one versioned
  `DiagramThemeSpecWire` serializer used by every binding metadata projection and by the native CLI.
  The CLI exposes a fixed read-only `theme materialize <preset> --format json` operation; metadata
  returns the preset revision, schema version, maturity, and the same complete editable spec. Unknown,
  removed, or alpha-only preset handling is identical across hosts. Materialization never introduces
  merge/patch semantics and encodes bounded assets through the existing typed resource wire shape.
- **Verification:** The candidate gate validates the Rust envelope, compiler/policy boundary,
  version/discovery behavior, facet descriptor population from real C5/C6 evidence, unknown-ID and
  additive-field behavior, cross-binding descriptor round trips, existing-preset migration, coarse
  reports, deletion audit, and independent contract review. The
  rollout gate then runs the native portions of the original U8-U11 matrices: Options/binding/Typst
  parity, native CLI scope, generated contracts, migration examples, and public API compilation.
  Only the combined receipt freezes `C7a-contract`. The rollout also regenerates all SDK/package
  copies from their authorities, checks Web types against the shared theme catalog, documents every
  resource field, and returns a versioned bounded execution-evidence envelope through Node, UniFFI,
  and C FFI. An alpha preset is discoverable only with an explicit maturity marker. A stable preset
  is eligible only when it declares supported family/target cells, allowed residual IDs, font source,
  and per-target admission expectations backed by C6 runner evidence.
- **Done when:** Users can exercise the proven native theme surface ergonomically while external
  host results remain honestly `HostDependent` and strict portability rejects them. Any later family
  work that requires a new public recipe shape, capability, or report contract reopens
  `C7a-contract` instead of silently extending the frozen API.

### C7b. Finish Remaining Family Breadth And Preset Evidence

- **Covers:** The remaining family/preset/reference-comparison work from the August 6 plan. External-host
  assurance is a separate future plan and does not sequence this work.
- **Approach:** Migrate remaining families according to the per-mechanism support matrix, keep or
  remove each behavior deliberately, and delete `LegacyFamilyThemeBridge` only when no
  `LegacyCompatibility` mechanism remains. Complete the source-traceable mechanism classification of
  all 24 pinned modern_mermaid themes and retained PR #28 contributions, while running detailed
  visual comparisons only for a selected representative set. Complete the remaining original
  family-breadth units. Ghibli, Hand Drawn, and Memphis are non-blocking C6
  mechanism witnesses. A witness may remain test-only or become an alpha preset candidate according
  to its general usefulness and cost; no theme name is required to complete this gate. If this work
  exposes a missing public recipe shape, reopen C7a-contract before adding it.
- **Verification:** Run the original U8-U11 matrices, platform feature checks, generated legal
  material checks, the complete mechanism-classification ledger, selected Modern/PR #28 visual
  reference comparisons, source/contributor attribution checks, simplification, and independent
  reviews. Preset candidates additionally verify descriptor maturity, materialized-spec export,
  resource closure, and their declared C6-backed cells.
- **Done when:** Every remaining family mechanism has an intentional terminal classification, no
  `LegacyCompatibility` route remains in the supported scope, the complete reference corpus has a
  typed/unsupported/residual classification with provenance, and no compatibility lane bypasses
  trust or portability evidence. No fixed theme name is required to become a shipped preset.

### C7c. Optional External Host Assurance

- **Trigger:** A separately approved product requirement needs an external result to become more
  than `HostDependent` or needs a stable browser/FFI external-layout transport.
- **Approach:** Define the independent trust authority, unforgeable provenance, data-disclosure and
  retention policy, cancellable process/worker/async boundary, and stable transport only in that
  follow-up plan. Backend self-assertion is never the trust root.
- **Done when:** The external contract has its own security, privacy, replay, budget, isolation, and
  cross-target verification gates. This is not a prerequisite for C7b family migration.

## Commit Sequence

1. Keep completed C1/C2/native-text-core commits as historical checkpoints; do not rewrite them.
2. `fix(theme): constrain compilation and refresh alpha resource contracts`
3. `fix(text): bind terminal prepared text to retained evidence`
4. `refactor(theme): complete family-scoped semantic programs`
5. `feat(theme): complete the native C6a eligibility slices`
6. `feat(theme): declare the alpha native theme candidate`
7. `feat(theme): roll out the candidate native theme surface`
8. `feat(theme)!: freeze the verified native theme contract`
9. `test(theme): close the cross-target C6b release matrix` may proceed independently once its target
   runners exist; it is required before stable cross-target preset/catalog claims, not before C7a.
10. Resume remaining family breadth, selected preset candidates, reference comparisons, and
    final-review commits only after
    the corresponding C7b gates pass. External-host assurance proceeds only under a separate C7c
    product decision.

## Verification Gates

- Cargo commands run serially; use `cargo nextest` for Rust tests and `cargo fmt` for formatting.
- Each behavior-bearing unit starts from a failing or strengthened test where practical.
- C6 tests declare their required features and have an explicit CI command; feature unification is
  never accepted as evidence that the gate ran.
- Generated Python package files, Web theme types, Options JSON resource fields, and all binding
  evidence envelopes have authority-versus-generated freshness checks before rollout can pass.
- Public `#[non_exhaustive]` theme enumerations expose `ALL` as a slice or iterator rather than a
  fixed-length array; workspace-only evidence types are moved behind an explicit opaque receipt
  module before the stable contract freezes.
- Default no-theme Mermaid snapshots and host callback order are regression gates for every text
  or policy change.
- A positive capability, family verification, portability, or target-admission assertion must name
  the evidence stage that produced it.
- Main worktree changes remain untouched. Only this worktree is staged and committed.

## Product Contract Preservation

The August 6 R/A/F/AE identifiers remain the traceability source. The Superseding Release Decisions
section changes the current-release contract for external-host portability, policy ownership, DTO
visibility, C5/C6 deletion ownership, product positioning, ThemeTokens contraction, preset admission,
public-surface timing, milestone sequencing, and official CLI scope. KTD14 records the session-settled
product-boundary and preset decision. KTD15 records the narrow post-alpha.5 UniFFI and Web/WASM
epoch correction. KTD16 records the independent Options schema 3 correction without changing the
C ABI, runtime-catalog schema, Android, Typst, Node, or the already-selected UniFFI/Web transport
epochs. Other Product Contract requirements remain in force or are explicitly
deferred to C7b/C7c rather than silently discarded.
