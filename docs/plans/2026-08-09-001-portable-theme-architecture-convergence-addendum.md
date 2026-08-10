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
| Product contract | The August 6 product contract and its R/A/F/AE identifiers remain unchanged. This addendum changes implementation order and ownership details only. |
| Authority | Mermaid `11.16.1@7ecca0cd` remains the semantic baseline. The compiled theme recipe describes visual intent; the render environment owns runtime ceilings; family and document evaluators prove what was actually applied. |
| Execution | Continue in `.worktrees/presentation-theme-model` on `refactor/presentation-theme-model`. C4a is closed for the deterministic native boundary and has been re-checked for bounded span traversal and cluster coverage; stop expanding the external prepared-text protocol and proceed through C5/C6 with the current Flowchart and State consumers. The native theme product surface may freeze after those gates; C4b must finish before any external-host assurance contract or its binding fields freeze. |
| Stop conditions | Do not expose a positive portability or capability conclusion from an unevaluated state, accept host-produced geometry without request/session evidence, or let theme/config inputs widen renderer-owned policy. |

## Why The Order Changes

The original direction remains valid, but implementation breadth has advanced faster than the
proof chain underneath it. Four conditions now require an architecture convergence gate:

- compile-time `required` capabilities are currently published as renderer `admitted` capabilities;
- an unadapted family with no collected residuals is currently reported as verified;
- a selected compiled theme can replace rather than restrict renderer-owned font policy;
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
- External backends return a public raw response DTO. Merman always validates the request binding,
  catalog identity, source/visible ranges, finite bounded geometry, and permitted font sources
  before constructing the internal immutable `PreparedText`. This basic admission is required even
  while the external result is reported as `HostDependent`.
- The built-in native catalog path is the first C4a producer that may claim `Portable`: its
  rustybuzz shaping, retained catalog faces, cluster/run evidence, and export font source are all
  owned by Merman. An external host result is `HostDependent` by default and cannot become
  `Portable` merely by echoing self-declared identity, capability, face, or source fields. C4b may
  define an explicit host trust authority, but without an independent trust root or Merman-owned
  replay the result remains `HostDependent`.
- A prepared result carries source-to-visible cluster ranges so transforms, entities, bidi runs,
  and fallback runs do not depend on matching transformed strings back to original words.
- Shape each run once. Wrapping uses retained cluster advances or prefix widths; it must not reshape
  every growing prefix.
- Runtime fallback is session-owned and ordered for the two real implementations that exist today:
  the configured host adapter and the built-in native catalog adapter. Host rejection, timeout,
  invalidation, and missing glyph may advance to the next allowed candidate. A custom catalog can
  never fall back to vendored default metrics. Do not expose a general multi-backend graph until a
  second non-native production adapter exists.
- The C4a external seam includes the safety needed by any renderable host result: a Merman-issued
  per-session generation binding, request/effective-policy binding, operation-level call/work and
  retained-response budgets, bounded candidate attempts, and session-local invalidation after
  protocol or budget violations. A same-process Rust callback remains trusted blocking host code;
  forced preemption belongs to an actual process/worker/async adapter boundary and must not be
  simulated by an ineffective logical timeout.
- Keep the external DTO/trait provisional, feature-gated, or crate-private until C7b. Do not expose
  it through stable JSON/FFI bindings, and document that a host adapter receives diagram label text
  plus bounded catalog metadata/resources needed for shaping.
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

- **After C4a closure:** do not expand the prepared-text DTO, backend graph, or assurance vocabulary
  before C5/C6 produce another real consumer. C4a closure permits crate-private Sequence work; it
  does not permit presets, public `Resolved*` evaluator APIs, stable binding schema fields, CLI theme
  flags, or public family adapters to freeze early. The external seam retains its bounded replay,
  budget, invalidation, and circuit-breaker safety; stronger trust/promotion remains deferred.
- **Before C5/C6 close:** State, `{Flowchart, Swimlane}`, and Sequence cutover work remains
  crate-private. C5 may prepare and internally cut over Sequence; only C6 may claim positive
  Sequence output evidence or a settled shared family interface.
- **Before C4b closes:** do not call an external host result `Portable` or freeze external-host
  assurance/attestation fields in any Rust, JSON, FFI, or language binding contract. Self-attested
  external results remain `HostDependent`; this does not block the native theme recipe/token product
  surface after C4a/C5/C6.
- **Until a real consumer runs:** do not claim Canvas/Effects capabilities are admitted merely
  because their data model validates, and do not promote modern theme showcases from “describable”
  to “rendered” evidence.
- Only renderer-owned, closed, versioned postprocessors may carry a preservation contract. Raw CSS
  and public custom postprocessors can never self-certify typed evidence; portability downgrade does
  not replace the independent SVG/CSS mount-safety decision.
- Keep the official `mmdc` compatibility surface unchanged except for fixes required by removed
  provisional alpha APIs.

## Implementation Checkpoint - 2026-08-11

| Gate | Status | Current evidence and remaining boundary |
| --- | --- | --- |
| C0 | Complete for the bounded Flowchart tranche | Prepared SVG labels retain one admitted text result through layout and emission; unsupported label modes fail closed. |
| C1 | Complete for the internal recipe domain | Recipe, font catalog, and SVG resource identities are separate and canonically encoded. Hidden legacy recipe state has been removed; public evaluator exposure remains a C7 decision. |
| C2 | In progress | Root/family reports, trusted-lane residuals, sealed document reports, consuming SVG admission, and target-specific prepared PNG/JPEG/PDF exports exist. State cascade evidence now attributes only final property winners. The terminal SVG consumer proves solid root base/layers (including explicit transparent base, opacity, placement, and blend declarations); native PNG/JPEG/PDF admission uses an explicit root-capability whitelist rather than assuming exporter support. Gradients, patterns, bleed/viewport expansion, effects, and complete emission-owned family evidence remain residual. Any untrusted postprocessor or raw theme CSS invalidates positive root evidence before document/target admission. |
| C3 | Complete for the current render session | `RenderEnvironment` owns runtime ceilings, sessions freeze the effective intersection, and themes cannot widen host policy. |
| C4a | Complete for the deterministic native boundary | Native rustybuzz shaping rejects `.notdef`, cluster fallback is used consistently by legacy and structured paths, and wrapping plus final metrics use separate monotonic span cursors with linear-visit regression coverage. Flowchart/State sidecars freeze only labels consumed by real SVG emission. Public SVG remains token-free while the sealed private SVG carries exact base-or-contiguous-line label tokens. PNG/JPEG/PDF verify the final token-associated face/source set against the retained per-label ledger and can produce `Portable` only from the owned catalog path. Exact final fallback byte ranges remain an explicit `usvg` integration hook, not inferred evidence. |
| C4b | Partial infrastructure retained; optional assurance deferred | Session-private fallback candidates, basic response admission, and usage reporting may remain, but no further general host-protocol expansion should precede C5/C6 proof. External backend output remains `HostDependent` unless an explicit independent host trust authority is later designed and admitted. |
| C5 | In progress; stages 1-2 converged, stage 3 partial | Explicit Mermaid compatibility now comes only from `spec.mermaid()`. The transitional bridge is family-local, runs after detection, preserves explicit site/source/detector ownership, and records compatibility residuals that strict portability rejects. `FamilyThemeProgram` owns the recipe, premerges static rules, retains source-order winners, meters only ordinal candidates, and now carries a private facet-level `TypedAdapter` / `LegacyCompatibility` / `Unsupported` route matrix. The bridge can read only route-approved legacy winners and never turns the matrix into positive evidence. State consumes the metered program directly; `{Flowchart, Swimlane}` and Sequence still use the bridge and remain the next cutovers. Final surviving compatibility provenance and parse/session theme binding remain convergence gates. |
| C6 | Not yet proven | The representative themes do not yet have a Flowchart/State/Sequence by SVG/PNG/PDF positive-output matrix. |
| C7a | Not eligible; intentionally blocked | Do not freeze the native theme recipe/token Rust, Options, binding, CLI, or preset surface before C1-C3, C4a, C5, and C6 close. Existing preset/CLI/binding selectors remain alpha inputs, not a completed compatibility promise. |
| C4b/C7b | Deferred by design | External-host assurance fields and remaining family/preset breadth freeze only after C4b closes. |

## Implementation Units

### C0. Preserve And Bound The Current Prepared Flowchart Tranche

- **Covers:** R1, R12-R16a, R43, R45.
- **Approach:** Retain the catalog/session split, structured typography, native mixed-script
  shaping, and family-owned SVG-label sidecar. Keep the default vendored/host callback path
  unchanged. Record SVG-only support explicitly; HTML/Markdown paths remain typed failures until
  C6. Do not copy the current provisional result contract into State or Sequence.
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
- **Approach:** Add all runtime ceilings to `RenderEnvironment`; intersect them with recipe
  requirements, source availability, and any prior compiler restriction when beginning a session.
  Remove theme getters that imply a recipe owns host policy. Thread the effective immutable policy
  through family preparation, sealed SVG, document report, and exporter admission.
- **Files:** `crates/merman-render/src/diagram_theme/admission.rs`,
  `crates/merman-render/src/diagram_theme/compiler.rs`,
  `crates/merman-render/src/diagram_theme/mod.rs`, `crates/merman-render/src/environment.rs`,
  `crates/merman-render/src/family.rs`, `crates/merman-render/src/svg/pipeline/mod.rs`,
  `crates/merman/src/svg/mod.rs`, `crates/merman/src/svg/operation.rs`.
- **Test scenarios:** embedded-only environment cannot be widened by a theme; strict portability
  cannot be weakened; trusted lanes are monotonic; empty intersections are typed failures before
  layout; selected-theme resources override environment resources while policies intersect;
  renderer setter order does not change effective policy.
- **Done when:** Session creation is the only place that produces effective runtime admission and
  every downstream consumer reads the same frozen result.

### C4a. Finish The Deterministic Prepared Text Core

- **Covers:** R12-R16a, R19-R19a, R43, R45, R49. R13 is satisfied by a prepared handshake plus a
  bounded `HostDependent` fallback path; it does not require an external backend to become
  `Portable`.
- **Approach:** Keep the public response DTO narrow and bounded, but make the native catalog path
  the first-class deterministic producer. Add request-derived aggregate response budgets, source/
  visible cluster ranges, native cluster-level fallback, actual face/source usage, and one-pass
  shaping with retained advances. Route layout, SVG, PNG, and PDF through the same native prepared
  result. Keep the configured host adapter behind a basic binding/range/geometry admission seam and
  report its accepted output as `HostDependent`. Bind every external call to a Merman-issued session
  generation and effective policy, charge aggregate backend work/response retention to the render
  operation, bound candidate attempts, and disable a candidate for the session after protocol or
  budget violations. Keep the provisional trait/DTO out of stable binding schemas. Treat the
  session-level prepared-layout report as candidate/backend summary only: family sidecars allocate
  operation-local deterministic label keys, mark evidence consumed only from real SVG emission, and
  freeze the consumed ledger after successful family rendering. Carry that read-only ledger on the
  sealed `ResvgCompatibleSvg` so the native exporter verifies the same artifact it parses instead of
  reconstructing family ownership from global session counters.
- **Files:** `crates/merman-render/src/text/prepared.rs`,
  `crates/merman-render/src/environment.rs`, Flowchart and State prepared-label sidecars,
  `crates/merman/src/svg/operation.rs`, `merman-export` font evidence, provisional host measurement
  adapters, and `fuzz/fuzz_targets/theme_text_layout.rs`.
- **Test scenarios:** native custom-font success; mixed Latin/CJK/combining/ZWJ coverage; transforms
  and explicit breaks; invalid ranges and metrics; request-derived byte/record/geometry budgets;
  native missing-glyph fallback; cmap-positive but shaped-`.notdef` rejection; variation selector,
  ZWJ, and complex-script positive/negative coverage; linear-work wrapping; the same per-label
  run/face/source ledger (including source/visible ranges) retained by layout and SVG, with the
  native exporter validating the token-associated final face/source set; an external response with
  basic valid evidence renders but remains `HostDependent`; replayed generation,
  operation-budget exhaustion, repeated protocol violation, and candidate circuit-breaker cases.
- **Done when:** No geometry reaches layout without basic request admission, the native path can
  produce `Portable` evidence from one retained catalog only after final shaping has rejected every
  `.notdef` cluster/run, native target admission verifies the actual token-associated final
  face/source set against the prepared per-label ledger, every renderable external result passes
  the C4a safety boundary, and at least a second crate-private family consumer can use the
  prepared result without widening its interface. Exact fallback byte-range evidence in the
  exporter remains an explicit follow-up requiring a maintained `usvg` hook; it is not inferred
  from resolver callback order or glyph text search. The interface remains reopenable until the
  Sequence witness in C6.

### C4b. Stabilize Optional External Host Assurance

- **Covers:** optional stronger host assurance, stable browser/FFI projection, and host-specific parts
  of R43/R45 beyond the required R13 `HostDependent` path. Family typography and long-label
  semantics remain owned by C5/C6.
- **Approach:** Preserve the bounded C4a raw DTO seam and any already-working session-private
  fallback/admission infrastructure, but defer further protocol breadth until the C5/C6 vertical
  slices expose real requirements. The public raw response DTO, request/session binding,
  capability/face/run evidence, invalidation, bounded diagnostics, and two-candidate fallback are
  provisional infrastructure to retain, not fields to redesign by default. Finish only the
  bounded fallback evidence and stabilize the corresponding FFI/binding projection. If a real
  product requires promotion beyond `HostDependent`, introduce a private unforgeable provenance
  state such as `NativeOwned`, `ExternalUnassured`, and `ExternalAssured`; `ExternalAssured` requires
  a configured independent host trust authority and never follows from the backend's own DTO alone.
  Otherwise omit promotion entirely. The coordinator must not become a public arbitrary-backend
  graph.
- **Files:** `crates/merman-render/src/text/prepared.rs`,
  `crates/merman-render/src/environment.rs`, binding/FFI adapters, exporter reports, and fuzz
  targets.
- **Test scenarios:** capability and face-evidence mismatch; stale session/request binding;
  invalidation; host rejection/timeout/missing glyph; policy ordering between native and accepted
  host-dependent results; strict portability rejection; actual per-label fallback usage; bounded
  diagnostics and fuzz smoke.
- **Done when:** The stable binding contract cannot confuse backend self-assertion with a trust root,
  every fallback outcome is represented by a bounded `(from, to, reason)` histogram plus candidate
  invalidation generation, and no public evaluator surface depends on provisional assurance fields.
  It is valid to close C4b with all external results permanently `HostDependent`.

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
     `LegacyCompatibility` mechanism until C7b migrates or explicitly drops that mechanism.
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
  semantic-rule projection or unmetered node-by-rule scans, the primary family tranche no longer
  depends on the legacy bridge, and every mechanism of every remaining family is explicitly
  classified as `TypedAdapter`, `LegacyCompatibility`, or `Unsupported`.

### C6. Complete Three Representative Vertical Slices

- **Covers:** R16a, R17-R24, R29-R32, R38-R41, R44-R49.
- **Approach:** Complete `{Flowchart, Swimlane}`, State, and Sequence before any other family. Extend
  the theme fixture schema to represent Sequence before using it as proof. Use three
  mechanism-distinct proof themes rather than broad preset count: Brutalist for ordinal/stroke/
  geometry, Spotless for typography/dash/repeating-gradient behavior, and Cyberpunk for layered
  canvas/blend/effect behavior. Add the smallest bounded effect consumer, resource closure, and
  paint-bounds/target admission needed by the Cyberpunk proof; broader Canvas/Effects remain C7b.
  Each slice must cover custom fonts, semantic paint/typography/spacing, source-style evaluation,
  SVG resources, and terminal document evidence. Add HTML/Markdown prepared-label support only
  through bounded family-owned projections; otherwise retain an explicit typed residual rather than
  building a general browser DOM/CSS engine.
- **Files:** Flowchart, State, and Sequence layout/render/style modules; sealed SVG/document modules;
  `merman-export`; representative fixtures under `fixtures/themes/`.
- **Test scenarios:** Define a fixture-backed acceptance table for Brutalist, Spotless, and
  Cyberpunk across Flowchart, State, and Sequence and the Standalone SVG, Browser SVG, PNG, JPEG,
  and PDF targets. Every named cell records expected mechanism state, residual IDs, font source,
  target admission, and an artifact assertion. Add focused Swimlane classification; light/dark
  State and Sequence readability; font clipping; long mixed-script labels; layout-changing
  typography; layered canvas; one bounded effect; `RequirePortable` and `BestEffort`; and default
  parity snapshots. HTML/Markdown are separate bounded label-mode witnesses or explicit residuals,
  not an implicit Cartesian multiplier.
- **Design-system witness:** Before the Rust API freezes, build light/dark `ThemeTokens` from one
  host-resolved token set, reuse each compiled theme across independent renderers, and prove State
  and Sequence SVG plus one native output without state leakage. Add mechanism-level witnesses for
  Ghibli, Hand Drawn, and Memphis so their unique needs can still reshape the typed model before API
  freeze without requiring their full cross-target matrix.
- **Done when:** Every mandatory acceptance-table cell has its declared positive or residual result,
  the design-system witness passes, and the three proof themes demonstrate actual renderer and
  target consumption rather than model-only expressiveness.

### C7a. Freeze The Native Theme Product Surface

- **Covers:** Native-theme Rust/API, Options, binding, CLI, Typst, and design-system portions of the
  remaining R/AE items.
- **Approach:** Only after C1-C3, C4a, C5, and C6 pass, freeze the native theme recipe/token API and
  update Options JSON, bindings, CLI/man pages/completions, Typst, examples, migration docs, and a
  deliberately selected first-party preset set. Do not expose provisional C4b attestation fields.
  Close or explicitly authorize the legacy `scoped_css` Options JSON lane before U9 is considered
  complete. Do not promote the complete 24-theme modern_mermaid reference catalog into supported
  presets without a separate product and licensing decision.
- **Done when:** Users can exercise the proven native theme surface ergonomically while external
  host results remain honestly `HostDependent` and strict portability rejects them.

### C7b. Finish Host Assurance And Remaining Family Breadth

- **Covers:** External-host assurance/binding clauses and the remaining family/preset/showcase work
  from the August 6 plan.
- **Approach:** After C4b closes, freeze only the host-assurance/attestation binding fields. Migrate
  remaining families according to the per-mechanism support matrix, keep or remove each behavior
  deliberately, and delete `LegacyFamilyThemeBridge` only when no `LegacyCompatibility` mechanism
  remains. Complete selected Modern/PR #28 showcase comparisons and the remaining original units.
- **Verification:** Run the original U8-U11 matrices, platform feature checks, generated legal
  material checks, Modern/PR #28 showcase comparison, simplification, and independent reviews.
- **Done when:** Public transports expose only settled native and host-assurance contracts, every
  remaining family mechanism has an intentional terminal classification, and no compatibility lane
  bypasses trust or portability evidence.

## Commit Sequence

1. `refactor(theme)!: separate recipe identity from runtime evidence`
2. `fix(theme): make capability and family evaluation explicit`
3. `refactor(theme)!: resolve host policy at render sessions`
4. `refactor(text)!: admit prepared text from bounded response evidence`
5. `perf(text): shape prepared labels once for bounded wrapping`
6. `refactor(theme): compile family-scoped semantic programs`
7. `feat(theme): complete flowchart state and sequence vertical slices`
8. `feat(theme)!: freeze the native theme product surface`
9. `refactor(text): finish external host assurance after vertical proof`
10. Resume remaining family breadth, selected presets, showcase, and final-review commits only after
    the corresponding C7b gate passes.

## Verification Gates

- Cargo commands run serially; use `cargo nextest` for Rust tests and `cargo fmt` for formatting.
- Each behavior-bearing unit starts from a failing or strengthened test where practical.
- Default no-theme Mermaid snapshots and host callback order are regression gates for every text
  or policy change.
- A positive capability, family verification, portability, or target-admission assertion must name
  the evidence stage that produced it.
- Main worktree changes remain untouched. Only this worktree is staged and committed.

## Product Contract Preservation

Product Contract unchanged. This addendum reorders implementation and narrows provisional public
claims; it does not remove any August 6 R/A/F/AE requirement.
