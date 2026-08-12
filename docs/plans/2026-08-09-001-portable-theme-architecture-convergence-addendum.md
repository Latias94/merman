---
title: "Portable Theme Architecture Convergence - Plan Addendum"
type: refactor
date: 2026-08-09
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
| Objective | Pause horizontal API and family expansion, then make theme identity, host policy ownership, capability evidence, prepared text, family evaluation, and target admission form one explicit and monotonic chain. |
| Product contract | The August 6 R/A/F/AE identifiers remain the traceability source. This addendum is authoritative for the current-release scope, ownership, evidence gates, and public-surface timing; any conflicting current-release statement in the August 6 plan is superseded by the matrix below. Long-term capabilities remain future requirements until their named gate is closed. |
| Authority | Mermaid `11.16.1@7ecca0cd` remains the semantic baseline. The compiled theme recipe describes visual intent; the render environment owns runtime ceilings; family and document evaluators prove what was actually applied. |
| Execution | Continue in `.worktrees/presentation-theme-model` on `refactor/presentation-theme-model`. Reopen C3 until every host profile constrains theme compilation before hostile bytes are decoded. Reopen the C4a terminal proof while preserving the completed native shaping, projection, wrapping, and bounded-work core: single-face native output may regain `Portable` only after final emitted text identity and ordered ranges are bound to the retained ledger; multi-face portability remains a C6b gate. Then finish C5/C6 across Flowchart, State, and Sequence. The native theme contract may freeze after C6a and real pre-freeze family consumers; stable cross-target preset claims wait for C6b. |
| Stop conditions | Do not expose a positive portability or capability conclusion from an unevaluated state, accept host-produced geometry without request/session evidence, or let theme/config inputs widen renderer-owned policy. |

## Why The Order Changes

The original direction remains valid, but implementation breadth has advanced faster than the
proof chain underneath it. Six conditions now require an architecture convergence gate:

- compile-time `required` capabilities are currently published as renderer `admitted` capabilities;
- an unadapted family with no collected residuals is currently reported as verified;
- a selected compiled theme can replace rather than restrict renderer-owned font policy;
- constrained Binding, CLI, and Typst requests still compile theme bytes, fonts, and effects with
  the default interactive compiler ceiling;
- native export currently reduces the retained prepared-text ledger to line count plus an unordered
  face/source set, so same-font text replacement or reordering can evade terminal verification;
- the initial prepared-text backend contract could not produce a result externally, while internal
  results were not bound to a request digest, source ranges, run evidence, or runtime fallback. The
  current WIP has a provisional raw DTO and basic admission seam; this addendum retains it without
  treating its host-assurance shape as frozen.

Canvas/effect types, bindings, presets, and additional public family surfaces must not expand until
these conditions are resolved. Existing source-backed corpus, resource validation, sealed-document
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
| Theme language | Stable `DiagramTheme` is a semantic visual recipe: typed targets, variants, paint/geometry, typography, bounded canvas/effects, and declared resources. It is not an arbitrary SVG/DOM/CSS programming language. Arbitrary selectors, element IDs, XML/HTML injection, and browser-only mechanisms remain `Unverified` or `SvgOnly` compatibility lanes. | A new primitive enters the stable language only after a real family consumer, resource limits, and target-specific evidence exist. |
| Theme authoring freedom | Users may create new themes by composing the supported `ThemeTokens` and full `DiagramThemeSpec` vocabulary, including family-scoped rules and bounded resources, without modifying renderer code. A new semantic target or drawing primitive requires renderer work and a new capability/evidence gate. | Additive target descriptors may be introduced through versioned discovery; they do not become silently accepted selector strings. |
| C5 versus C6 | C5 owns compilation, facet-level routing, direct family consumption, and family-local tests. C6a owns native terminal evidence; C6b extends the same proof to Browser SVG, JPEG, and PDF. | A bridge route is removed only after its matching native C6a evidence passes. Broader target claims wait for C6b. |
| Public surface timing | C7a-candidate may be declared only after C6a plus real pre-freeze family-writer/compiler round trips. C7a-rollout migrates bindings, Typst, examples, and native Merman CLI against that alpha candidate. C7a-contract freezes only after rollout verification. Fine-grained mechanism/evidence types and alpha preset IDs remain private or explicitly unstable. | Stable preset catalog and broader target/family claims require C6b. |
| Official CLI | New theme selection belongs only to the Merman native render/batch surface. The official `mmdc` compatibility surface remains unchanged except for removal of provisional alpha fields. A document, Options JSON, or project config cannot self-authorize a trusted lane. | Any mmdc expansion requires a separate compatibility decision. |

### Identifier Disposition

The following mapping makes the relationship to the August 6 identifiers executable rather than
implicit. Any August 6 identifier not listed here is retained unchanged and keeps its original
owner.

| August 6 identifiers | Current-release disposition | Owning gate |
| --- | --- | --- |
| R13, R43, R45; F2; AE15; KTD4, KTD8, KTD11; U3 external-host clauses | Native prepared text and external `HostDependent` safety are retained. External `Portable` promotion, fully attested host claims, stable external DTOs, and browser/FFI projection are deferred and do not participate in the current release. | C4a owns native proof, C4b owns crate-private external safety, C7c owns any future assurance contract. |
| R24, R35, R48; F5; AE7, AE18 | Retained with a stricter authority boundary: raw CSS and arbitrary SVG remain explicit trusted compatibility inputs, never self-authorize through document/config data, and cannot bypass the independent terminal safety decision. | C2 terminal safety and C7a-rollout public-surface checks. |
| R25, R47; F3, F7; AE17; KTD5; U4 | Retained and strengthened with the fixed terminal order: postprocess, active-content/CSS safety, resource closure, residual merge, then target admission. | C2. |
| R33-R37; F4, F8; AE18-AE21; KTD1, KTD9, KTD10; U8-U10 | Retained but reordered: define a candidate native contract, migrate first-party consumers against it, then freeze only after rollout verification. The official `mmdc` surface does not gain new theme flags. | C7a. |
| R38-R41; AE3-AE6, AE19-AE24; KTD2, KTD6, KTD7, KTD12; U1, U6, U7, U11 | Retained. Positive public claims require family-writer and target-artifact evidence, not model expressiveness or fixture declarations alone. | C5, C6a, C6b, and C7b as specified below. |
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
materializes a complete spec before compilation. Preset patch/merge semantics are not part of the
first stable binding or CLI contract. This avoids making merge precedence a second theme language;
a later typed preset-override layer requires a separate product decision and its own deterministic
materialization rules. The rollout must still provide a read-only, versioned way to materialize each
supported preset as a complete editable spec through the native CLI and binding metadata. That
export does not add patch semantics and records the preset/schema version from which it was derived.

Public maturity is explicit rather than inferred from where a symbol appears:

| Stage | Public promise |
| --- | --- |
| Internal proof | Recipe, receipt, and mechanism details may change without compatibility notice. |
| C7a candidate | Schema and selected presets are alpha discovery values; alpha preset IDs and completions are not stable enum or compatibility promises. |
| C7a rollout | First-party consumers exercise the exact alpha candidate and return a versioned, bounded execution-evidence summary. |
| C7a contract | Only the proven native recipe/compiler/token envelope and coarse report contract become stable. |
| C6b release | Individual presets may become stable only for their declared, runner-proven cells; an unqualified cross-target catalog claim requires the complete C6b gate. |

### C6 Contract Layers

C6 has four deliberately separate layers:

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

The current code is an early draft, not the final contract described above: the committed schema
does not yet retain every selector/facet/value or digest, and its public test constructors are not
the private target-runner boundary. The checkpoint remains incomplete until those constraints are
implemented.

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
- C4a's terminal exporter proof is not closed. The retained ledger already owns the request digest,
  projection spans, emitted line ranges, and ordered run evidence, but the exporter currently checks
  only label token, line count, and an unordered face/source set. Until the final emitted line-text
  digest and ordered source/visible runs are verified, even single-face native output remains
  `HostDependent` or rejected. Multi-face fallback additionally requires exact terminal per-run
  cluster/source byte ranges and remains a C6b gate.
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

## Implementation Checkpoint - 2026-08-12

| Gate | Status | Current evidence and remaining boundary |
| --- | --- | --- |
| C0 | Complete for the bounded Flowchart tranche | Prepared SVG labels retain one admitted text result through layout and emission; unsupported label modes fail closed. |
| C1 | Complete for the internal recipe domain | Recipe, font catalog, and SVG resource identities are separate and canonically encoded. Hidden legacy recipe state has been removed; public evaluator exposure remains a C7 decision. |
| C2 | In progress | Root/family reports, trusted-lane residuals, sealed document reports, consuming SVG admission, and target-specific prepared PNG/JPEG/PDF exports exist. State cascade evidence now attributes only final property winners. The terminal SVG consumer proves solid root base/layers (including explicit transparent base, opacity, placement, and blend declarations); native PNG/JPEG/PDF admission uses an explicit root-capability whitelist rather than assuming exporter support. Gradients, patterns, bleed/viewport expansion, effects, and complete emission-owned family evidence remain residual. Any untrusted postprocessor or raw theme CSS invalidates positive root evidence before document/target admission. |
| C3 | Reopened at host compiler boundaries | Runtime sessions correctly own effective ceilings, but Binding, CLI, and Typst still create the default interactive theme compiler before applying a constrained render profile. C3 closes only when every host derives one compiler ceiling before decoding and request themes can only restrict it. |
| C4a | Native core complete; terminal `Portable` proof reopened | Native rustybuzz shaping, cluster fallback, projection, bounded wrapping, consumed-label sidecars, and sealed label tokens are retained. PNG/JPEG/PDF currently reduce the ledger to line count plus an unordered face/source set, so same-font text drift can evade verification. Single-face `Portable` resumes only after final emitted line text, request identity, and ordered source/visible run evidence are bound; multi-face proof remains in C6b. |
| C4b | Conditional follow-up; assurance excluded | Session-private fallback candidates and basic response admission may remain, but no current product consumer requires the unfinished generation, budget, or circuit-breaker framework. Keep stable injection disabled. Implement those controls only before enabling a concrete crate-private external consumer. External output remains `HostDependent`; any later assurance or promotion requires C7c. |
| C5 | In progress; stages 1-2 converged, stage 3 partial | Explicit Mermaid compatibility now comes only from `spec.mermaid()`. The transitional bridge is family-local, runs after detection, preserves explicit site/source/detector ownership, and records compatibility residuals that strict portability rejects. `FamilyThemeProgram` owns the recipe, premerges static rules, retains source-order winners, meters only ordinal candidates, and now carries a private facet-level `TypedAdapter` / `LegacyCompatibility` / `Unsupported` route matrix. The bridge can read only route-approved legacy winners and never turns the matrix into positive evidence. State consumes the metered program directly; `{Flowchart, Swimlane}` and Sequence still use the bridge and remain the next cutovers. Final surviving compatibility provenance and parse/session theme binding remain convergence gates. |
| C6a | Partial infrastructure; native eligibility gate not closed | The acceptance specification and four-cell State tranche exist, but observations are caller-constructible and no exact 18-cell eligibility receipt exists. Flowchart/State/Sequence by Standalone SVG/PNG, source-style/spacing, repeating-gradient/effect shapes, design-system, cold-start, and real pre-freeze family consumers remain open. |
| C6b | Not started as a release gate | Browser SVG, JPEG/PDF completion, multi-face export, and the full 45-cell target-owned evidence matrix remain open. |
| C7a | Not eligible; intentionally blocked | Do not declare the alpha contract candidate before C1-C3, C4a, C5, and C6a close plus the pre-freeze family consumers. Do not freeze the contract until C7a-rollout verification passes. Stable cross-target preset claims wait for C6b. |
| C7b/C7c | Deferred independently | Remaining family/preset/showcase breadth proceeds under C7b without waiting for external assurance. External-host assurance fields remain a separately triggered C7c plan. |

The public alpha migration scaffold already spans Rust, Options JSON, Web/UniFFI, Typst, Playground,
and native/compatibility CLI entry points. That scaffold is not a frozen promise and must contract to
the staged C7a boundary: remove the provisional `mmdc` flags, keep only native CLI selection, repair
generated Python/Web/options drift, and allow breaking alpha corrections before C7a-contract.
The resource-policy implementation unit owns that immediate repair tranche rather than deferring
known stale generated packages or unsafe request lanes until final rollout.

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
  native exporter validating the token-associated emitted line-text digest, request digest, ordered
  source/visible runs, and final face/source evidence. Include negative cases where text or run order
  changes while the line count and face set remain unchanged.
- **Done when:** No geometry reaches layout without basic request admission, the native path can
  produce `Portable` evidence from one retained catalog only after final shaping has rejected every
  `.notdef` cluster/run, native target admission verifies the actual token-associated emitted text,
  request identity, ordered ranges, and face/source evidence against the prepared per-label ledger,
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
     families remain on the explicitly residual bridge.
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
  default no-theme Mermaid output remains unchanged.
- **Done when:** One selected family consumes one bounded compiled program without global
  semantic-rule projection or unmetered node-by-rule scans, the primary family tranche has direct
  family-local consumers and tests for its selected routes, and every mechanism of every remaining
  family is explicitly classified as `TypedAdapter`, `LegacyCompatibility`, or `Unsupported`.
  Terminal bridge-route deletion remains a C6a responsibility.

### C6a. Complete The Native Contract-Eligibility Slices

- **Covers:** The native portions of R16a, R17-R24, R29-R32, R38-R41, and R44-R49.
- **Approach:** Complete Flowchart, State, and Sequence before any other matrix family. Swimlane is a
  focused Flowchart classification witness, not a fourth Cartesian family. Use three
  mechanism-distinct proof themes: Brutalist for ordinal/stroke/geometry, Spotless for typography,
  dash, and repeating-gradient behavior, and Cyberpunk for layered canvas/blend/effects. Add an
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
  each compiled theme across independent renderers, and prove State and Sequence SVG plus PNG without
  state leakage. Add mechanism witnesses for Ghibli, Hand Drawn, and Memphis, but do not count them
  as matrix cells.
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
- **Browser evidence:** A dedicated non-public WASM operation returns the sealed SVG/report envelope.
  The Web runner mounts it in a scriptless sandboxed opaque-origin iframe with restrictive CSP, no
  top navigation, and no external network access. Its versioned JSON receipt binds source/input,
  recipe, document/resource, mount-policy, target, final artifact, and post-mount DOM digests. It
  records browser engine/version. A mechanism is `Applied` only with an engine-specific computed-
  style or tolerance-bounded pixel proof; an unqualified Browser SVG claim requires the supported
  Chromium, Firefox, and WebKit lanes to pass.
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
  Do not promote the complete 24-theme modern_mermaid reference catalog into supported
  presets without a separate product and licensing decision.
- **Verification:** The candidate gate validates the Rust envelope, compiler/policy boundary,
  version/discovery behavior, coarse reports, deletion audit, and independent contract review. The
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

### C7b. Finish Remaining Family Breadth And Showcases

- **Covers:** The remaining family/preset/showcase work from the August 6 plan. External-host
  assurance is a separate future plan and does not sequence this work.
- **Approach:** Migrate remaining families according to the per-mechanism support matrix, keep or
  remove each behavior deliberately, and delete `LegacyFamilyThemeBridge` only when no
  `LegacyCompatibility` mechanism remains. Complete selected Modern/PR #28 showcase comparisons and
  the remaining original family-breadth units. Ghibli, Hand Drawn, and Memphis are non-blocking C6
  mechanism witnesses; their complete showcases belong here. If this work exposes a missing public
  recipe shape, reopen C7a-contract before adding it.
- **Verification:** Run the original U8-U11 matrices, platform feature checks, generated legal
  material checks, Modern/PR #28 showcase comparison, simplification, and independent reviews.
- **Done when:** Every remaining family mechanism has an intentional terminal classification, no
  `LegacyCompatibility` route remains in the supported scope, and no compatibility lane bypasses
  trust or portability evidence.

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
10. Resume remaining family breadth, selected presets, showcase, and final-review commits only after
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
visibility, C5/C6 deletion ownership, public-surface timing, and official CLI scope. Other Product
Contract requirements remain in force or are explicitly deferred to C7b/C7c rather than silently
discarded.
