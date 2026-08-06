---
title: "Portable Diagram Theme Architecture - Plan"
type: refactor
date: 2026-08-06
deepened: 2026-08-06
artifact_contract: ce-unified-plan/v1
artifact_readiness: implementation-ready
product_contract_source: ce-plan-bootstrap
execution: code
origin: docs/plans/2026-08-02-001-refactor-presentation-theme-architecture-plan.md
supersedes: docs/plans/2026-08-02-001-refactor-presentation-theme-architecture-plan.md
---

# Portable Diagram Theme Architecture - Plan

## Goal Capsule

| Field | Contract |
| --- | --- |
| Objective | Replace the provisional presentation/theme split with a compiled diagram-theme module whose typography, semantic styles, canvas, SVG effects, font assets, SVG output, and native exports share one resolved visual contract. |
| Authority | Mermaid `11.16.0@7c0cafcf` owns Mermaid semantics and default parity. The local `modern_mermaid@a021cbc` clone is a capability corpus for custom-theme expressiveness, not a new parity baseline. ADR-0063 and the existing resource/security contracts remain authoritative unless this plan replaces a named owner. |
| Execution profile | Execute in `.worktrees/presentation-theme-model` on `refactor/presentation-theme-model`. Preserve unrelated user changes. Split retained correctness fixes from superseded presentation work before the first implementation commit. Use Conventional Commits and run Cargo commands serially. |
| Stop conditions | Stop and report if a required font asset cannot be redistributed or embedded, if a proposed portable effect cannot survive SVG and native export without an explicit residual, or if implementation would require a general browser CSS cascade, arbitrary XML injection, or network/filesystem I/O inside `merman-render`. |
| Tail ownership | Complete implementation, focused fixtures, migration, deletion, simplification, review, and local commits. Do not open a PR, publish, tag, merge, or modify the occupied main worktree without a new explicit request. |

## Product Contract

### Summary

Merman needs a theme abstraction that describes a complete diagram visual recipe instead of only mapping host colors into Mermaid variables. The compiled recipe must affect layout before geometry is frozen, emit structured SVG resources after bounds are known, retain exact font assets through terminal SVG finalization, and drive PNG, JPEG, and PDF from the same document resources.

The public model will have four core concepts. `DiagramThemeSpec` is the declarative input. `DiagramTheme` is the validated immutable result. `ThemeTokens` is a convenience adapter from a host design system. `RenderProfile` owns non-theme renderer behavior such as the first-party Modern Flowchart policy. Supporting types such as `DiagramThemeCompiler`, host-owned policy axes, resource policy, prepared-layout sessions, and staged reports make those concepts enforceable without becoming additional visual owners. `MermaidConfig` and `SvgPipeline` remain compatibility escape hatches, but they no longer define the portable-theme contract.

### Problem Frame

The August 2 plan assumed exact CSS-heavy themes could be composed from `HostTheme`, raw `MermaidConfig`, and `SvgPipeline`. Repository and `modern_mermaid` research disproved that assumption:

- `themeCSS` is applied after family layout and SVG generation, so font size, font family, weight, spacing, transform, padding, wrapping, and bbox changes arrive too late.
- `TextStyle` and the measurement protocol do not carry the full typography required by the theme corpus.
- native export unconditionally builds a process-global system `fontdb`, so layout, browser SVG, and PNG/PDF can resolve different fonts.
- root background policy expresses only a solid color. It cannot express gradients, patterns, layered canvas paint, or paint bleed.
- arbitrary CSS selectors such as `:nth-child`, `:has()`, and complex attribute selectors do not have stable semantics across browser SVG and usvg/simplecss.
- typed SVG effects and resource IDs do not have a first-class owner. Rust postprocessors can inject strings, but bindings cannot safely declare or validate equivalent structure.
- the current `PresentationProfile` visual fallback couples product behavior to appearance and obscures precedence.

The branch already contains uncommitted work from the superseded plan. Some changes remain valid, including request-scoped site-config materialization and the scoped-CSS root-sibling isolation fix. The conditional Redux/slate profile fallback, provisional direct `HostTheme` surface, and documentation that claims raw composition is sufficient must not become the foundation of the new design.

### Superseded Requirement Mapping

The requirement IDs in this document form a new namespace. They do not preserve the meaning of similarly numbered requirements in the August 2 plan.

| Superseded requirements | Disposition | Replacement or owner |
| --- | --- | --- |
| old R1 | Retained and deepened | new R1, R7, R35-R36 |
| old R2-R5 | Replaced by the compiled-theme ownership model | new R2-R8, R24, R31, R33-R36 |
| old R6-R11 | Split into behavior-only profile and visual theme contracts | new R4, R7-R8, R34 plus U0's retained Flowchart correctness audit |
| old R12-R17 | Replaced by immutable compiled values, target-aware reports, and binding contracts | new R2-R4, R24-R37, R42-R49 |
| old R18 | Retained as migration work | new R34-R37 and U10 |
| old R19 | Retained as an existing correctness invariant, not reimplemented | U0 must preserve its source-backed Flowchart/ELK fixes |
| old R20 | Retained as a complexity boundary | new R5-R6, R17-R24 and the no-mini-browser stop condition |
| old R21 | Still deferred | remains outside this theme refactor |
| old R22 | Replaced by the final theme/profile/output migration | new R35-R37 and U9-U10 |

### Actors

- A1. A Rust host compiles a custom theme with local font bytes and requires deterministic SVG and native exports.
- A2. A binding or CLI user selects a built-in theme, supplies structured custom values, or references bounded embedded font data through Options JSON.
- A3. A Mermaid document author supplies frontmatter, directives, classes, and inline styles that must retain Mermaid-compatible precedence.
- A4. A renderer maintainer adds a semantic theme target or family adapter without implementing another CSS engine.
- A5. An export consumer needs evidence that the SVG, font catalog, portability result, and operation report came from the same completed render.
- A6. A product owner selects a behavior profile independently from visual theme selection.

### Requirements

#### Ownership and default behavior

- R1. A renderer with no selected `DiagramTheme` or `RenderProfile` must preserve the current Mermaid-parity config, layout, SVG DOM, and output behavior.
- R2. `DiagramTheme` must be the only public owner of a complete compiled visual recipe: Mermaid theme inputs, typography, semantic styles, canvas, effects, assets, and declared capability requirements. Enforceable host admission and resource ceilings are separate owners.
- R3. `ThemeTokens` must be a design-system adapter that can produce a `DiagramThemeSpec`; it must not claim to express every family-specific or structural theme feature.
- R4. `RenderProfile` must own renderer behavior only. Neo, ELK defaults, and Merman-private Flowchart policy are renderer behavior; Redux/slate colors, fonts, canvas paint, CSS, and SVG resources are theme concerns and must not come from a profile.
- R5. `MermaidConfig` must remain the exact upstream-compatibility owner for official Mermaid values, document frontmatter, directives, class definitions, and raw `themeCSS`.
- R6. `SvgPipeline` must remain the output transformation and terminal safety owner. It must not own typed theme canvas, typography, semantic styles, or effect graphs.
- R7. Configuration precedence must be independent of builder call order: base engine config, `RenderProfile` behavior defaults, compiled `DiagramTheme` Mermaid inputs, explicit renderer or request `site_config`, then source frontmatter and directives.
- R8. First-party editor themes and the Redux/slate visual half of `merman-modern` must become full `DiagramTheme` presets or token adapters. They must not absorb Neo, ELK, or private Flowchart behavior and must not select output policy implicitly.

#### Font assets and typography

- R9. A compiled theme must retain an immutable, content-addressed `FontCatalog` containing validated font binaries, face metadata, family aliases, generic-family mapping, the sources the theme can supply or accept, embedding requirements, and a stable fingerprint. Source priority belongs only to the host-owned `FontSourcePolicy`; a theme cannot introduce a competing priority order. Host resource ceilings must be charged during compilation but must not become request-controlled catalog data.
- R10. Theme compilation must accept decoded SFNT inputs and WOFF2 inputs. WOFF2 must be decoded once to canonical SFNT bytes with a pure-Rust, WASM-compatible adapter before measurement, SVG emission, or export.
- R11. Font compilation must validate face indexes, family names, style, weight, collection structure, OpenType embedding permissions, duplicate IDs, single-asset bytes, total bytes, face count, and decoded expansion before retaining data.
- R12. Layout, wrapping, SVG bbox, computed text length, and native export must use a prepared text-layout service bound to the exact `FontCatalogFingerprint`.
- R13. A host/browser text-layout adapter must complete a prepare handshake before Merman accepts measurements. The evidence must bind the canonical catalog, backend identity/version, supported style/shaping contract, loaded catalog faces, and reusable session token. Catalog identity alone is insufficient. A mismatched, unavailable, or under-attested host must follow the host-owned measurement fallback policy and report the result.
- R14. Resolved text style must support font stack, size, weight, style, line height, letter spacing, word spacing, text transform, decoration, and white-space/wrap behavior before layout is frozen.
- R15. Native text layout must segment fallback runs by Unicode cluster and glyph coverage so mixed-script stacks such as Excalifont plus Xiaolai use deterministic per-run faces.
- R16. Existing Mermaid-parity vendored metrics must remain the default for themes that do not opt into custom font assets. The new font-backed path must not silently change default snapshots.
- R16a. Each family label must prepare source tokens, visible text, fallback runs, wrapped lines, bbox height, and computed width once, then reuse that immutable preparation during layout and SVG emission without changing observable host-measurement request order.

#### Semantic styles, canvas, and SVG effects

- R17. Theme styling must compile from typed semantic targets and typed paint/geometry properties rather than browser CSS selectors or Mermaid DOM paths. Unknown targets/variants and statically invalid target-family combinations are compile errors.
- R18. The semantic rule model must support common targets, family-scoped targets, named semantic variants, interaction/state variants that exist in static output, and deterministic ordinal palettes for cases currently expressed with `:nth-child`. A rule scoped to another family is not applicable; a known target missing from a family adapter is an internal capability error, not a user-input fallback.
- R19. Layout-affecting style fields from both the compiled theme and Mermaid source-owned styles must resolve before family layout. Paint-only fields may resolve during structured SVG assembly but must reuse the same compiled rule result and source precedence outcome.
- R20. `CanvasSpec` must support transparency, solid paint, linear gradients, radial gradients, bounded patterns, layered paint, layer opacity, and the blend modes required by the admitted modern theme corpus.
- R21. Canvas and effect assembly must distinguish layout bounds from paint bounds. Insets, filter bleed, stroke bleed, and overflow policy must participate in final viewBox and output-size planning without feeding back into graph layout.
- R22. `SvgEffectSet` must provide typed, bounded filter primitives and semantic bindings for drop shadow, blur, color matrix, turbulence, and displacement. Unsupported primitive combinations must fail compilation or produce an explicit best-effort residual.
- R23. A shared `SvgResourceNamespace` and collision-checking ID allocator must own gradients, patterns, filters, markers, clip paths, and other defs across all families.
- R24. Raw `themeCSS`, custom `SvgPostprocessor`, untyped output CSS, and untracked resource mutations must remain available as advanced compatibility lanes, but their use must mark portability as `Unverified`. `SvgOnly` is reserved for recognized, modeled SVG features that lack an admitted native representation.

#### Rendered documents and native export

- R25. `ResvgCompatibleSvg` must retain the authorized export resources captured from the render session plus a terminal `SvgResourceClosure` rebuilt from the final SVG. Every export mode must independently reject an unresolved resource closure; portability admission is a separate gate applied only after resource closure succeeds. Converting it to `String` must be documented as a lossy projection that discards the resource-bearing export capability.
- R26. `RenderedDocument` must replace the public `(ResvgCompatibleSvg, RenderOperationReport)` tuple as the only facade-level completion type that carries a Mermaid operation report. Its fields and construction must remain private so callers cannot combine artifacts from different operations. `ResvgCompatibleSvg` remains the public low-level sealed-SVG export artifact required by F7, but it cannot carry or forge a Mermaid operation report.
- R27. `RenderedDocument` must expose the sealed SVG, SVG text, operation report, theme resolution report, resource fingerprints, and portability result without retaining live measurers or mutable runtime services.
- R28. PNG, JPEG, and PDF convenience APIs must render one `RenderedDocument` and project from it. They must not re-render, re-finalize, or choose a second font environment.
- R29. `merman-export` must construct an operation-owned font database from the document catalog. The `embedded_only` policy preset must never consult system fonts; `embedded_then_system` must prefer retained faces; `system_only` must preserve the legacy host-dependent path. Font-source policy is an allowed-source set plus host-owned priority, independent from portability admission and host-measurement fallback.
- R30. Portable browser SVG must emit trusted, typed `@font-face` resources when the embedding policy allows it. Native exporters must still consume the retained catalog because usvg cannot be assumed to build its font database from SVG CSS.
- R31. Raster `background` must become output `matte`, and PDF `background` must become `page_paint`. These options composite outside the themed SVG canvas and must not replace `CanvasSpec`.
- R32. Reports must state theme fingerprint, font catalog fingerprint, coarse font fallback decisions, applied approximations, and whether raw CSS or postprocessors invalidated proof. Compilation emits an output-independent capability report, finalization emits a document report, and each exporter emits a target-specific portability/admission result. General bindings must redact host font paths, fontdb source IDs, and complete installed-family inventories.

#### Public API, bindings, migration, and proof

- R33. The Rust facade must expose `DiagramTheme`, `DiagramThemeSpec`, `DiagramThemeCompiler`, `ThemeTokens`, `ThemePreset`, `RenderProfile`, `FontCatalog`, `RenderedDocument`, the host-owned `ThemeAdmissionPolicy`, `FontSourcePolicy`, `HostMeasurementFallbackPolicy`, and existing `RenderResourcePolicy`, plus compile-, document-, and export-stage capability/portability reports through small immutable interfaces with private fields.
- R34. `HostTheme`, `HostThemePreset`, `ThemeRole`, `PresentationProfile`, `PresentationAspect*`, `ResolvedPresentation`, the `presentation` Options JSON group, and the conditional profile visual fallback must be removed or renamed in one breaking migration. Do not retain deprecated forwarding wrappers for unreleased alpha.4 APIs.
- R35. Options JSON must move to a theme-first schema with separate `theme`, `render_profile`, `site_config`, and `svg` owners. `theme` is a closed tagged union of one preset or one complete spec; preset/token/spec patch composition is not a binding contract. Ordinary binding and untrusted Options JSON requests must reject raw `site_config.themeCSS`; only an explicitly enabled trusted Rust/CLI compatibility lane may accept it, and that lane always produces an `Unverified` portability result plus an independent CSS safety result. Unknown fields, invalid resources, raw XML/defs, file paths, network URLs, and unbounded font payloads must be rejected.
- R36. A complete Options JSON spec may carry bounded base64 font assets inside that spec. Omitted `theme` inherits the constructor value, explicit `null` clears the selected theme without weakening host policy, an empty object is invalid, and preset/spec are mutually exclusive. Core theme compilation must receive bytes and must not perform I/O; cross-operation reuse comes from reusing the compiled renderer/theme, not a new global asset registry.
- R37. Runtime discovery must report known theme presets, render profiles, required compiled capabilities, portability support, supported font containers, semantic target catalogs, and output-specific residuals from Rust-owned descriptors.
- R38. `merman-fixture-render-context` must remain a test-only owner of source-bound fixture config and DOM evidence. Theme assets and portability fixtures must use a separate theme fixture catalog.
- R39. The modern theme capability corpus must prove that the architecture can model the diagram and canvas mechanics of 23 local `modern_mermaid` themes. Aurora backdrop blur must remain an explicit residual unless a source-backed portable implementation is found.
- R40. Annotation colors and the interactive annotation overlay from `modern_mermaid` must remain application-owned and outside the core diagram-theme contract.
- R41. `Portable` means the same admitted visual semantics, resources, bounds, and fallback decisions across the enabled outputs. It must not be documented as platform-independent text raster pixels or browser `getBBox()` floats.
- R42. `ThemeAdmissionPolicy` and `RenderResourcePolicy` must be constructor/host-owned ceilings. Request themes may declare stricter requirements but cannot enable system fonts, `BestEffort` portability admission, host-measurement fallbacks, raw resources, or larger budgets that the host disabled.
- R43. Portability requirement, font-source policy, and host-measurement fallback must be separate typed axes with explicit capability-set resolution. No policy type may mix an acceptance decision such as `RequirePortable` with a font source such as system discovery.
- R44. Portability must be assessed by stage and output target. A compiled theme reports required capabilities; a finalized document reports source/pipeline residuals; SVG, PNG/JPEG, and PDF exporters make the final target-specific admission decision.
- R45. Catalog-backed host measurement is `HostDependent` when the backend is under-attested. A backend that attests the versioned shaping/style contract and the actual catalog face/run evidence required by the operation may be `Portable` for a target that admits that contract. Native catalog-backed layout remains the authoritative portable path.
- R46. Every Mermaid style origin and property used by an admitted family must be classified as layout-before-freeze, paint-only structured, non-visual, or `Unverified`, with one source-backed precedence matrix shared by all family adapters.
- R47. Custom postprocessors remain trusted compatibility extensions. They must either declare a bounded resource delta that terminal closure validates or produce an unresolved closure that every export rejects; a stale pre-postprocess catalog must never be described as complete. Even a declared, closed postprocessor result remains `Unverified` for portability because resource-closure safety does not prove visual semantics.
- R48. Typed theme specs and font bytes may cross untrusted binding boundaries only under host-owned budgets. Raw CSS, custom postprocessors, arbitrary SVG, and loose parity DOM insertion remain trusted-input lanes; portability grading is not a security sandbox.
- R49. Theme font/resource parsing must have fuzz coverage across base64, WOFF2 reconstruction, SFNT validation, catalog compilation, bounded text/features/variations shaping, and SVG font serialization. U2 owns byte/container/catalog fuzzing, U3 owns `rustybuzz` shaping fuzzing, and U6 owns SVG font serialization fuzzing and negative resource-reference coverage. Public diagnostics must be bounded and redacted so they do not become a system-font enumeration surface.

### Key Flows

#### F1. Compile and install a portable Rust theme

1. The host builds `ThemeTokens` or a `DiagramThemeSpec`.
2. The host constructs a compiler from its `ThemeAdmissionPolicy` and `RenderResourcePolicy`.
3. The compiler decodes and validates assets, charges budgets, compiles semantic rules, validates canvas/effects, and produces stable fingerprints plus an output-independent capability report.
4. `HeadlessRenderer::with_theme` stores the immutable compiled theme.
5. Each render resolves the theme against the detected family and effective Mermaid config before layout.
6. The completed operation returns `RenderedDocument`; all exports consume that document.

#### F2. Render with a host/browser measurer

1. The adapter receives the canonical catalog fingerprint, backend contract version, normalized typography capabilities, and bounded catalog-owned face descriptors.
2. `prepare_catalog` returns a reusable session token, backend identity/version, loaded catalog face evidence, and the supported shaping/style contract.
3. Merman routes layout, wrap, bbox, and computed-length requests through that prepared session and records actual face/run evidence when the backend can provide it.
4. An under-attested or mismatched host follows the host-owned ordered fallback set (`AcceptHostDependent`, `NativeCatalog`, conditionally `VendoredDefault`, or an empty set meaning fail). `AcceptHostDependent` can preserve a valid bounded host result only as `HostDependent` and can never satisfy `RequirePortable`. `VendoredDefault` is eligible only for the no-custom-font Mermaid-parity path; a custom catalog must use `NativeCatalog`, accept a host-dependent result, or fail. `ThemePortabilityRequirement` then accepts or rejects the result independently; font source selection remains governed by `FontSourcePolicy`.
5. Rust and native bindings can use the native catalog backend. WASM introduces a versioned browser prepare/session protocol. Native ABI 3 remains unchanged and therefore does not expose the new host-catalog handshake.

#### F3. Render a theme with canvas and effects

1. Family layout produces layout bounds.
2. Semantic styles produce stroke and effect outsets.
3. Root assembly calculates paint bounds and final viewport.
4. The shared ID allocator emits defs, font declarations, canvas layers, and effects.
5. Family content is emitted with semantic bindings.
6. Compatibility CSS and custom pipeline stages run before terminal validation.
7. Terminal validation rebuilds `SvgResourceClosure`, reconciles declared postprocessor resource deltas, and computes the document-level residuals before sealing the SVG.

#### F4. Use Options JSON

1. The binding accepts exactly one of `{ "preset": ... }` or `{ "spec": ... }`; a complete spec owns its assets.
2. The binding validates bounded resource payloads and compiles the selected value under constructor-owned admission and resource policies.
3. An omitted request `theme` inherits the constructor theme, `null` clears it, and an empty object or preset/spec mixture is rejected.
4. `site_config` and source config retain Mermaid precedence after the compiled theme layer, but ordinary binding/untrusted requests reject raw `site_config.themeCSS`; a trusted Rust/CLI compatibility mode must be enabled by constructor policy before it is accepted.
5. Request values may tighten but cannot relax system-font, measurement-fallback, `BestEffort` admission, raw-resource, or resource-budget ceilings.
6. Bindings return existing output bytes plus bounded, redacted theme/resource diagnostics through the established report channel.

#### F5. Use raw Mermaid `themeCSS`

1. A trusted host supplies `themeCSS` through `MermaidConfig`.
2. Scoping provides namespace isolation only. Parity/readable SVG may retain trusted CSS and remains unsafe for untrusted DOM insertion without the canonical browser safety policy and host CSP/isolation.
3. `resvg_safe` tokenizes CSS and rejects imports, external URLs, active declarations, and unsupported resource forms independently from portability grading.
4. The report marks the result `Unverified`; strict portable export rejects it and best-effort export proceeds only under host policy with the residual recorded.

#### F6. Use `RenderProfile::MermanModern`

1. The profile contributes behavior defaults and private Flowchart render policy only.
2. The user selects a visual `ThemePreset` independently.
3. Non-Flowchart operations do not require ELK.
4. Flowchart capability admission follows the effective renderer after site/source overrides.

#### F7. Finalize arbitrary SVG

1. A trusted Rust host or explicit low-level CLI path creates a `RenderSession` with authorized resources; Options JSON theme inputs and general bindings cannot inject arbitrary SVG or raw defs.
2. `SvgPipeline::resvg_safe` validates the complete SVG and rebuilds its terminal resource closure.
3. The resulting `ResvgCompatibleSvg` retains authorized resources and unresolved-resource evidence even though no typed Mermaid operation report exists.
4. Low-level exporters obey the retained catalog and host admission policy. Every export mode rejects incomplete closure. A resource-closed result remains `Unverified` and can proceed only when the host explicitly admits it through `BestEffort`.

### Acceptance Examples

- AE1. Default `info`, Flowchart, Class, Sequence, and Ishikawa fixtures remain unchanged when no theme/profile is selected. Covers R1 and R16.
- AE2. A WOFF2-backed Hand Drawn fixture uses the same face fingerprint for wrap, bbox, standalone SVG, PNG, and PDF; missing or restricted font data fails before layout. Covers R9-R16 and R25-R30.
- AE3. A Geometric Collage fixture assigns deterministic ordinal fills to nodes, actors, states, classes, ER entities, tasks, pie slices, commits, and timeline events without CSS `:nth-child`. Covers R17-R19 and R39.
- AE4. A Cyberpunk fixture emits a layered gradient/pattern canvas and bounded glow effects into real SVG defs and canvas elements, with no root CSS background image. Covers R20-R23.
- AE5. A Hand Drawn fixture scopes turbulence/displacement IDs per diagram and expands the final viewport so filter output is not clipped. Covers R21-R23.
- AE6. An Aurora fixture compiles under `BestEffort` portability admission with a named backdrop-blur residual and fails `RequirePortable`. Covers R22, R24, and R39.
- AE7. A raw `themeCSS` fixture remains usable for browser SVG through an explicitly trusted lane but is reported as `Unverified`; `RequirePortable` rejects it after the independent CSS/resource safety gate instead of silently claiming parity. Covers R5, R24, and R32.
- AE8. `RenderProfile::MermanModern` plus a custom theme retains Neo/ELK/private Flowchart behavior without adding Redux/slate colors. Covers R4, R7, and R34.
- AE9. Two documents with identical SVG text and different catalogs produce different resource fingerprints and cannot have their reports or resources recombined through public constructors. Covers R25-R29 and R41.
- AE10. The raw-SVG CLI path retains explicit font resources through terminal finalization and export. Covers R25 and R29.
- AE11. Options JSON rejects external font URLs, file paths, raw SVG defs, duplicate asset IDs, decoded expansion beyond policy, and unknown semantic targets. Covers R11, R22, R35, and R36.
- AE12. Existing scoped-CSS tests cover root sibling escapes for plain and attribute-qualified selectors, and the fix remains in the advanced compatibility lane. Covers R24.
- AE13. A request theme cannot turn an embedded-only, require-portable renderer into system-font or `BestEffort` portability admission, cannot alter the host's measurement-fallback axis, and cannot raise any constructor resource ceiling. Covers R42-R43.
- AE14. One document reports `Portable` for self-contained SVG and a target-specific PDF residual when an admitted filter requires local rasterization; the compile report does not pretend to know the final exporter result. Covers R41 and R44.
- AE15. A browser measurer with the right font bytes but no shaping/run attestation is accepted only through `AcceptHostDependent` and cannot satisfy `RequirePortable`; the native catalog backend and a fully attested catalog-backed host can satisfy portable admission for targets that admit their contracts. Covers R13, R43, and R45.
- AE16. The same typography property set by theme defaults, Mermaid class definitions, assigned classes, and inline source style resolves through one source-backed precedence matrix before layout. Covers R5, R19, and R46.
- AE17. A custom postprocessor that adds an undeclared font or external resource cannot pass the resource-closure gate; a declared bounded resource delta is included in a closed terminal inventory but remains `Unverified` and therefore fails `RequirePortable`. Covers R25, R47, and R48.
- AE18. Options JSON rejects mixed preset/spec values, empty theme objects, budget-raising overlays, unsafe raw CSS/resource lanes, and unredacted system-font diagnostics. Covers R35-R36, R42-R43, and R48-R49.

### Success Criteria

- The public theme vocabulary describes the lifecycle of each value without using `presentation` as a mixed owner.
- The default parity path does not acquire new font, canvas, effect, or export work.
- Portable custom themes use one font catalog and one resolved style plan across layout and every output.
- Canvas and effect paint cannot be clipped by a viewport that was planned before their outsets were known.
- The modern capability corpus has a source-backed expression result for every theme mechanism and a named residual for anything not portable.
- Unreleased provisional APIs and compatibility shims are deleted instead of preserved.

### Scope Boundaries

#### In Scope

- `merman-render` theme compilation, text-layout resources, structured root assembly, semantic style binding, and reports.
- `merman` facade APIs and `RenderedDocument` orchestration.
- `merman-export` font resolution and output-compositing terminology.
- Options JSON, CLI, WASM/native binding discovery, examples, docs, changelog, and alpha.4 migration.
- Representative fixtures for fonts, semantic ordinals, canvas, filters, portability reporting, and modern theme capability coverage.
- Removal or renaming of provisional alpha.4 presentation/theme APIs and dead code.

#### Out of Scope

- A browser CSS cascade, selector engine, or Mermaid DOM compatibility layer in Rust.
- Interactive annotation overlays, theme editors, or application UI state from `modern_mermaid`.
- Network font fetching, caller-supplied font paths, and theme-driven file reads inside `merman-render` or `merman-export`. Existing system-font discovery remains available only when the resolved font-source set includes `System`.
- Arbitrary XML, raw `<defs>`, JavaScript, external resource URLs, or custom postprocessor code inside `DiagramTheme`, Options JSON, or general binding theme schemas. Existing trusted low-level Rust/CLI SVG finalization remains in scope under F7.
- Forced pixel-perfect normalization for browser `getBBox()`, font rasterization, foreignObject, D3 wrapper noise, RoughJS geometry, or PDF filter rasterization.
- Changing the pinned Mermaid baseline from 11.16.0.

#### Deferred

- Shipping all 24 `modern_mermaid` themes as supported Merman presets. This plan proves expressiveness and may ship selected first-party presets; importing another project's full theme catalog requires a separate product and licensing decision.
- An application annotation-theme extension may consume `DiagramTheme` metadata later without entering core SVG ownership.
- A global cross-operation fontdb cache may be added only after operation-owned correctness and resource accounting are proven.

### Dependencies and Assumptions

- The workspace currently pins `usvg/resvg 0.47`, `fontdb 0.23.0`, `ttf-parser 0.25.1`, and `rustybuzz 0.20.1` through the existing dependency graph.
- Use `wuff 0.2.8` with its pure-Rust Brotli path for WOFF2 decoding unless implementation proves an incompatibility with the workspace MSRV, WASM targets, resource accounting, or required conformance corpus.
- Font data used in committed fixtures must include a repository-compatible license and explicit embedding permission. Remote font URLs in the reference project are evidence, not redistributable test assets by default.
- The current branch has unrelated historical commits and uncommitted work. Implementation must stage only files owned by each unit.

## Planning Contract

### Key Technical Decisions

- KTD1. **Replace the provisional API instead of preserving compatibility.** (session-settled: user-directed - chosen over incremental compatibility wrappers: alpha.4 is unreleased and the user explicitly permits breaking deletion.) `HostTheme`, mixed presentation terminology, and the current visual fallback are migration inputs, not compatibility authorities. Covers R2-R8 and R33-R35.
- KTD2. **Build bottom-up from portable rendering capability before finalizing the public theme API.** (session-settled: user-directed - chosen over renaming the current surface first: the user requires proof that the renderer can express modern visuals before exposing a customization contract.) The implementation order is host policy/resource accounting, canonical fonts, prepared text layout, document retention, root paint/effects, semantic styles, then public convenience APIs. Covers R9-R32 and R42-R49.
- KTD3. **Compile one deep immutable theme module under host-owned policy.** `DiagramThemeCompiler` validates one complete `DiagramThemeSpec` into `DiagramTheme`; per-operation `ResolvedDiagramTheme` contains only family-applicable rules, prepared typography, structured paint resources, fingerprints, and diagnostics. Rejected: a mega `HostTheme` string bag, request-owned ceilings, or unrelated setters. Covers R2, R3, R17-R24, R42, and R49.
- KTD4. **Canonicalize fonts before any adapter sees them.** WOFF2 is decoded once with `wuff`; `ttf-parser` validates canonical SFNT faces; `rustybuzz` shapes the authoritative portable path; usvg/fontdb receives the same retained bytes. A host backend must attest its contract or remain `HostDependent`. Rejected: treating matching font bytes alone as layout equivalence. Covers R9-R16, R29-R30, R43, and R45.
- KTD5. **Retain authorized resources and rebuild terminal closure.** `ResvgCompatibleSvg` carries the session's authorized resources plus final `SvgResourceClosure` for typed and trusted raw SVG paths. `RenderedDocument` adds immutable operation/theme reports and replaces the facade tuple. Rejected: resources only in `RasterOptions`, only in the facade, a live `RenderSession` retained after completion, or stale pre-postprocess resource claims. Covers R25-R32 and R47-R48.
- KTD6. **Assemble canvas and effects before final viewport completion.** Extend the structured root document path around `RootViewportSpec`, `RootViewportPlan`, `begin_document`, and `finish_document`; do not mutate root viewBox through a late string postprocessor. Covers R20-R23.
- KTD7. **Use semantic targets plus one source-style admission matrix instead of a CSS selector engine.** Family renderers bind semantic targets while they own typed models and stable encounter order. Theme defaults and Mermaid source styles resolve through one shared origin/property contract before layout; raw CSS remains an unverified compatibility lane. Covers R5, R17-R19, R24, and R46.
- KTD8. **Assess portability at compile, document, and export stages.** Theme compilation reports required capabilities, document finalization records source/pipeline residuals, and each output target makes its own admission decision. No target-agnostic `DiagramTheme::portability()` may claim facts owned by `merman-export`. Covers R13, R22, R24, R32, R41, and R44-R45.
- KTD9. **Keep `RenderProfile` behavior-only.** Rename `PresentationProfile`; remove visual fallback; retain profile aspect admission only where it carries real behavior or capability evidence. Rejected: deleting the useful Merman Modern behavior bundle or putting it inside `DiagramTheme`. Covers R4, R7, R8, and R34.
- KTD10. **Use one closed complete theme value at boundaries.** Rust APIs take bytes. Options JSON chooses either a preset or a complete spec whose assets are internal to that spec; it never merges preset, token, spec, and asset patches. CLI and typed bindings may read files or convert `ThemeTokens` before constructing that value. Reusing a compiled renderer/theme provides reuse without a global asset registry. Covers R35-R37 and R42.
- KTD11. **Keep default Mermaid parity, native portable layout, and host-attested layout as distinct paths.** Default fixtures retain vendored Mermaid metrics and current DOM behavior. A custom catalog opts into native prepared layout for portable output; a host backend is downgraded unless it satisfies the attestation contract. Covers R1, R13, R16, and R45.
- KTD12. **Use a separate theme fixture catalog.** `merman-fixture-render-context` continues to bind upstream fixture bytes to test context and evidence only. Theme resource manifests own licensed assets, expected fingerprints, portability claims, and output coverage. Covers R38-R40.
- KTD13. **Keep security ceilings outside theme values.** `ThemeAdmissionPolicy` and the existing `RenderResourcePolicy` belong to the host/renderer and resolve as monotonic ceilings. Typed untrusted inputs are budgeted; raw CSS, arbitrary SVG, and postprocessors remain explicit trusted lanes. Covers R42-R43 and R47-R49.

### High-Level Technical Design

```text
ThemeTokens / DiagramThemeSpec / ThemePreset
                    |
                    v
    DiagramThemeCompiler(host admission + resources)
     fonts + typography + semantic rules
     canvas + effects + Mermaid theme inputs
                    |
                    v
           immutable CompiledDiagramTheme
                    |
         per operation + detected family
                    v
             ResolvedDiagramTheme
      prepared text layout + style plan
      canvas/effect plan + resource ids
                    |
          layout bounds and family SVG
                    v
       structured root document assembly
        paint bounds + defs + canvas + CSS
                    |
    SvgPipeline terminal validation + resource closure
                    v
 ResvgCompatibleSvg(svg + resources + closure report)
                    |
                    v
 RenderedDocument(svg + document reports + fingerprints)
          |              |             |
         SVG           PNG/JPEG        PDF
          \____________ target-specific admission ____________/
```

#### Proposed module ownership

```text
crates/merman-render/src/diagram_theme/
  mod.rs                public compiled-theme boundary
  spec.rs               DiagramThemeSpec and declared requirements
  tokens.rs             ThemeTokens and token presets
  compiler.rs           policy-aware validation and fingerprints
  admission.rs          ThemeAdmissionPolicy and policy resolution
  assets.rs             FontCatalog and charged asset accounting
  typography.rs         resolved text styles and prepared layout
  semantic.rs           typed targets, applicability, ordinal palettes
  source_styles.rs      source-origin/property precedence contract
  canvas.rs             canvas paints, layers, patterns, bounds
  effects.rs            typed filter graph and bleed calculation
  portability.rs        capability, document, and target assessments
  resolved.rs           per-operation family projection

crates/merman-render/src/svg/
  root_document.rs      structured root/defs/canvas assembly
  resource_ids.rs       scoped collision-checking allocator
  resource_closure.rs   final resource inventory and reconciliation
  pipeline/             compatibility transforms and terminal safety

crates/merman/src/svg/operation.rs
  RenderedDocument      typed completed-operation seam
```

The existing private `crates/merman-render/src/theme.rs` forwarding shim must be renamed or absorbed into `diagram_theme::resolved` so the public concept has one canonical name.

### Public Model Shape

```rust
pub struct DiagramTheme(Arc<CompiledDiagramTheme>);

pub struct DiagramThemeSpec {
    mermaid: MermaidThemeInputs,
    typography: TypographySpec,
    styles: ThemeRuleSet,
    canvas: CanvasSpec,
    effects: SvgEffectSet,
    assets: ThemeAssets,
    requirements: ThemeRequirements,
}

pub struct ThemeRequirements {
    required_capabilities: BTreeSet<ThemeCapability>,
    required_text_capabilities: BTreeSet<TextLayoutCapability>,
    available_font_sources: BTreeSet<FontSource>,
    embedding: FontEmbeddingRequirement,
}

pub struct DiagramThemeCompiler {
    admission: ThemeAdmissionPolicy,
    resources: RenderResourcePolicy,
}

impl DiagramThemeCompiler {
    pub fn compile(&self, spec: DiagramThemeSpec) -> Result<DiagramTheme, ThemeCompileError>;
    pub fn compile_preset(&self, preset: ThemePreset) -> Result<DiagramTheme, ThemeCompileError>;
}

impl DiagramTheme {
    pub fn fingerprint(&self) -> ThemeFingerprint;
    pub fn capabilities(&self) -> &ThemeCapabilityReport;
}

pub struct ThemeAdmissionPolicy { /* host-owned monotonic ceilings */ }
pub enum ThemePortabilityRequirement { RequirePortable, BestEffort }
pub enum FontSource { Embedded, System }
pub struct FontSourcePolicy { /* allowed set plus host-owned priority */ }
pub enum HostMeasurementFallback { AcceptHostDependent, NativeCatalog, VendoredDefault }
pub struct HostMeasurementFallbackPolicy { /* ordered allowed fallback set; empty means fail */ }

pub struct ThemeTokens { /* semantic design-system values */ }

impl ThemeTokens {
    pub fn into_theme_spec(self) -> DiagramThemeSpec;
}

pub enum RenderProfile {
    MermanModern,
}

pub struct RenderedDocument {
    svg: ResvgCompatibleSvg,
    report: RenderOperationReport,
    theme_report: ThemeResolutionReport,
    portability: DocumentPortabilityReport,
}
```

Public builders may provide convenience methods, but every convenience must compile through the same `DiagramThemeSpec` path. Public fields remain private to preserve validation and future extension.

`ThemeRequirements` declares capabilities that must be admitted and font sources that the theme can actually supply or accept. It never carries font-source priority, host-measurement fallback order, portability admission mode, or resource ceilings; those remain host-owned policy axes. An empty `available_font_sources` set is invalid. A custom catalog that requires its retained bytes expresses that by admitting only `Embedded`, while a theme that can tolerate system substitution may admit both sources and let the host choose priority.

### Theme Rule Model

`ThemeRuleSet` is declarative and semantic:

```rust
pub struct ThemeRule {
    target: ThemeTarget,
    family: Option<RenderFamilyKind>,
    variant: Option<ThemeVariant>,
    ordinal: Option<OrdinalSelector>,
    style: ThemeStylePatch,
}
```

The compiler applies one applicability matrix:

| Case | Result |
| --- | --- |
| Unknown target or variant | Compile error |
| Explicit family plus statically invalid target-family combination | Compile error |
| Rule explicitly scoped to a different operation family | Not applicable; recorded only in debug evidence |
| Known target absent because the selected family has no such semantic element | No match, not a portability residual |
| Family adapter fails to bind a target it declares as supported | Internal capability error |

Family renderers attach stable semantic target IDs while they still own typed models and encounter order. Rules never receive raw selectors, DOM paths, element IDs, XML, or arbitrary CSS declarations. Theme rules are defaults below source-owned Mermaid class/inline styles. U1 and U7 must derive the exact origin/property precedence matrix from the pinned Mermaid source before implementing family adapters.

### Font and Text Layout Contract

- `FontAsset` owns canonical SFNT bytes and validated face metadata.
- `FontCatalog` owns ordered stacks, aliases, generic families, available/required font-source evidence, embedding policy, and fingerprint; `RenderResourcePolicy` owns all allocation ceilings and `FontSourcePolicy` owns priority.
- `TextLayoutBackend::prepare(PrepareCatalogRequest)` returns `PreparedTextLayout` bound to catalog fingerprint, backend contract version, backend identity, normalized typography capabilities, and a reusable session token.
- Native `PreparedTextLayout` uses the canonical catalog and `rustybuzz` and is the authoritative portable path.
- Host-backed layout returns loaded catalog face evidence and, where supported, actual face/run evidence. Without the full attestation contract its highest grade is `HostDependent` even when bytes match. A fully attested catalog-backed host may be `Portable` for a target that explicitly admits the attested backend contract.
- Protocol requests carry resolved style, direction/script/language, feature/variation settings, catalog identity, and session token, not only CSS family strings.
- WASM introduces a versioned prepare/session API. Rust traits can expose the same lifecycle. Native ABI 3 and bindings without this lifecycle use native prepared layout rather than silently extending the ABI record.
- the terminal report compares the prepared-layout identity with the retained export catalog and records the chosen fallback transition.
- trusted `@font-face` emission is generated from typed assets. It bypasses raw-CSS parsing but still passes byte, MIME, embedding, and terminal reference validation.
- family-local prepared labels, including `PreparedFlowchartSvgLabel`, retain the source-token and visible-text views plus the prepared measurement result so layout and SVG do not tokenize, wrap, or measure the same label twice.
- `AcceptHostDependent` is a reachable fallback for valid bounded host results that lack portable attestation. It always records `HostDependent` and therefore fails `RequirePortable`.
- `VendoredDefault` is restricted to the no-custom-font default Mermaid-parity path. A custom catalog cannot silently discard its typography by selecting vendored default metrics; it must use native catalog layout, accept an explicitly host-dependent result, or fail.

### Canvas and Effect Contract

- `CanvasSpec` compiles into defs plus one or more first-child canvas elements.
- `RootViewportPlan` receives content bounds, canvas inset, stroke outset, and filter bleed before final width/height/viewBox emission.
- `SvgEffectSet` is an acyclic graph with deterministic IDs and bounded primitives.
- every filter declares or computes its region. Missing regions are compile errors for portable themes.
- PDF may locally rasterize supported filters. That output remains portable when semantics are preserved and the report states the rasterized region; it is not claimed to be pure vector.

### Portability Contract

Portability has three owners rather than one overloaded theme getter:

| Stage | Owner | Result |
| --- | --- | --- |
| Compile | `DiagramThemeCompiler` | `ThemeCapabilityReport`: admitted typed mechanisms, required resources, host dependencies, unsupported features |
| Finalize | `RenderedDocument` / `ResvgCompatibleSvg` | `DocumentPortabilityReport`: actual family, source-style admission, host-layout evidence, postprocessor effects, terminal resource closure |
| Export | SVG/PNG/JPEG/PDF exporter | `ExportPortabilityReport`: target capability, local rasterization, font resolution, final admission decision |

| Grade | Meaning at a concrete assessment stage/target | Strict export |
| --- | --- | --- |
| `Portable` | All admitted semantics, resources, bounds, and fallback decisions required by that target are represented. This is semantic/resource portability, not identical platform text pixels. | Allowed |
| `HostDependent` | The target used system fonts or an under-attested host measurement result and records exact dependency reasons. A fully attested catalog-backed host may instead be `Portable` when the target admits that backend contract. | Rejected by `RequirePortable`; allowed by `BestEffort` |
| `SvgOnly` | The theme uses a browser/SVG feature without an admitted native representation. | Rejected |
| `Unverified` | Raw CSS, custom postprocessors, or untracked resource mutations invalidate proof. | Rejected |

Later stages may preserve or weaken earlier evidence but cannot upgrade an unproven input. `ThemePortabilityRequirement` belongs to host/operation admission, not inside a replaceable request theme. PDF filter rasterization can still be portable for PDF when its semantics, bounds, resources, and raster plan are admitted; it does not change the SVG target's result.

Resource closure and portability are independent gates:

| Gate | Question | Consequence |
| --- | --- | --- |
| `SvgResourceClosure` | Are all terminal SVG references authorized, bounded, self-consistent, and available to the selected exporter? | `Unresolved` always rejects export, including `BestEffort`; `Closed` permits portability admission to run. |
| Portability admission | Is there sufficient evidence for the requested target's visual semantics, layout, bounds, and fallback decisions? | `RequirePortable` accepts only `Portable`; `BestEffort` may accept `HostDependent`, `SvgOnly` where the chosen target is SVG, or `Unverified` with explicit residuals. |

A declared postprocessor resource delta can produce `SvgResourceClosure::Closed` while the document remains `Unverified`. A trusted low-level arbitrary-SVG input can also pass resource closure and target safety checks, but it has no typed Mermaid operation provenance and therefore cannot receive a Mermaid `Portable` document report.

### Options JSON Shape

```json
{
  "render_profile": "merman-modern",
  "theme": {
    "preset": "editor-dark"
  },
  "site_config": {},
  "svg": {}
}
```

or:

```json
{
  "render_profile": "merman-modern",
  "theme": {
    "spec": {
      "typography": {},
      "styles": [],
      "canvas": {},
      "effects": [],
      "requirements": {},
      "assets": {
        "fonts": [
          {
            "id": "example-regular",
            "format": "woff2",
            "data_base64": "..."
          }
        ]
      }
    }
  },
  "site_config": {},
  "svg": {}
}
```

The tagged union has no internal merge contract. A request-level preset or spec replaces the inherited theme as one complete compiled value; `null` clears it; omission inherits it; `{}` is invalid. `ThemeTokens` is a typed Rust/host adapter that must be converted to a complete spec before crossing the Options JSON boundary. Constructor-owned `ThemeAdmissionPolicy` and `RenderResourcePolicy` are never part of this replacement.

### Policy Resolution and Trust Boundaries

Policy resolution is monotonic:

1. The constructor selects `ThemeAdmissionPolicy`, `RenderResourcePolicy`, `FontSourcePolicy`, `HostMeasurementFallbackPolicy`, and `ThemePortabilityRequirement` ceilings.
2. A compiled preset/spec can declare stricter requirements or narrower sources.
3. A request can replace the theme but cannot relax constructor ceilings.
4. Source config can change Mermaid semantics but cannot enable forbidden resource sources or raise budgets.
5. Document finalization and exporters may downgrade or reject; they never upgrade missing evidence.

"Monotonic" is defined per axis rather than by enum ordering:

| Axis | Representation | Resolution rule |
| --- | --- | --- |
| Resource ceilings | Stable limit-ID to numeric ceiling | Pointwise minimum; hard caps always win |
| Portability requirement | `BestEffort < RequirePortable` | Choose the stricter requirement |
| Font sources | Allowed set over `{Embedded, System}` plus host priority | Intersect allowed sets and preserve host priority; empty intersection is an admission error |
| Host measurement fallback | Ordered allowed set over `{AcceptHostDependent, NativeCatalog, VendoredDefault}` | Intersect allowed sets and preserve host order; `AcceptHostDependent` can only yield `HostDependent`; `VendoredDefault` is ineligible for a custom catalog; empty means fail rather than invent another fallback |
| Trusted compatibility lanes | Per-capability booleans | Logical AND; requests cannot enable a lane disabled by the host |

Themes declare required capabilities and available resources, not a new priority order. For example, a theme that requires an embedded catalog is rejected by a `system_only` host policy instead of silently changing either side's meaning. Fallback eligibility is evaluated after capability-set intersection, so a custom catalog cannot make `VendoredDefault` eligible merely because the host listed it.

| Input lane | Trust posture | Required handling |
| --- | --- | --- |
| Typed `DiagramThemeSpec` and embedded font bytes through bindings | Potentially untrusted | Closed schema, canonical parsing, host-owned budgets, no I/O, structured errors |
| Generated typed canvas/effects/font CSS | Merman-owned | Deterministic emission, ID namespace, terminal resource closure, browser-policy parity |
| Raw `themeCSS` | Trusted Rust/CLI compatibility input only; rejected from ordinary binding/untrusted Options JSON requests | Explicit host enablement, CSS tokenization for safe outputs, no external loads, `Unverified`, separate CSS safety outcome, browser CSP/isolation |
| Rust `SvgPostprocessor` | Trusted extension | Bounded declared resource delta may close resources but remains `Unverified`; unresolved closure always rejects export |
| Complete arbitrary SVG | Trusted low-level Rust/CLI input | `resvg_safe`, terminal closure, no Mermaid operation report or Mermaid `Portable` claim, target-specific safety/admission |
| Browser DOM insertion | Separate consumer boundary | Canonical `platforms/web/src/svg-safety-policy.ts`, generated VS Code copy, freshness check, host CSP/isolation |

Theme resource accounting extends the existing stable `RenderResourcePolicy` descriptors. New stable limit IDs cover encoded theme JSON/base64, compressed font bytes, decoded SFNT bytes, face/table/catalog totals, emitted font CSS bytes, effect primitive count, filter region/pixels, turbulence octaves, blur/displacement, and postprocessor resource deltas. Hard implementation caps remain non-overridable. Preflight order is encoded length, canonical base64 length, declared WOFF2 expansion, bounded decode, font parse/accounting, projected SVG data expansion, then target filter raster planning.

Typed embedded `@font-face` support must update the canonical Web policy and generated VS Code policy together. Only compiler-emitted font MIME/container pairs are admitted; canonical base64, signature agreement, per-face and aggregate encoded/decoded limits, and external-URL rejection are mandatory. Portability reports and safety-policy results remain separate concepts.

General binding reports expose catalog-owned asset IDs, requested aliases, fingerprints, grades, and coarse fallback categories. They do not expose system font paths, fontdb source identifiers, or complete loaded-family lists. Exact host diagnostics require an explicitly trusted debug surface.

### System-Wide Impact

- `merman-core`: no theme resource ownership. It continues to own Mermaid config parsing and source precedence.
- `merman-render`: primary implementation owner for compiled themes, prepared text layout, structured root assembly, semantic bindings, and resource reports.
- `merman-export`: consumes retained resources and output compositing policy only.
- `merman`: owns the typed operation and `RenderedDocument` facade.
- `merman-bindings-core`: validates Options JSON, compiles themes, maps structured errors, and exposes discovery.
- CLI/WASM/FFI/UniFFI/JNI: project structured resources into core without duplicating theme compilation.
- Web/VS Code safety policy: admit only compiler-owned bounded font data and preserve generated-policy freshness.
- security/fuzzing: update rendering threat contracts and add a hostile theme/font resource target to the existing `fuzz/` package.
- docs/examples/playground: rename concepts and demonstrate theme/profile/output independence.
- fixture tooling: retain upstream evidence catalog; add a separate theme capability corpus.

### Risks and Mitigations

| Risk | Mitigation |
| --- | --- |
| Custom font shaping changes default parity | Keep the existing vendored measurement path as the no-theme default; opt into catalog-backed layout only when the compiled theme requires it. |
| WOFF2 decompression amplification | Charge compressed and decoded bytes, face count, table count, and total catalog bytes before retention; decode once. |
| Font licensing or embedding restrictions | Parse OpenType permissions, require an explicit embedding policy, reject restricted committed fixtures, and document caller responsibility. |
| A semantic target model grows into another selector language | Keep targets closed and family-owned; add descriptors only when a renderer can bind a stable semantic element. |
| Effects are clipped or allocate excessive native images | Compute bleed before viewport finalization and enforce primitive, region, pixel, octave, blur, displacement, and nesting caps. |
| Embedded fonts bloat every exported document | Deduplicate catalog assets by content hash; allow native-only or host-dependent policy; defer global cache until correctness is measured. |
| Matching font bytes are mistaken for matching layout | Treat native catalog layout as authoritative; require versioned shaping/run attestation for a portable host backend and otherwise report `HostDependent`. |
| Request data weakens a reusable renderer | Keep admission, font-source, measurement-fallback, and resource ceilings constructor-owned and monotonic. |
| Postprocessors invalidate the captured resource set | Rebuild terminal resource closure, reconcile declared deltas, and reject unresolved resources in every export mode; only a closed-but-`Unverified` result can reach `BestEffort` admission. |
| Font/CSS inputs expand the untrusted attack surface | Use the trust matrix, stable staged budgets, canonical browser policy, bounded diagnostics, fuzzing, and host isolation guidance. |
| Current uncommitted changes obscure ownership | Start with a keep/supersede inventory and commit retained orthogonal fixes separately. Never restore or reset user changes. |
| Binding JSON becomes too large | Enforce compressed and decoded payload limits, recommend constructor-level compiled themes for reuse, and let typed CLI/binding helpers accept bytes before serializing or entering core. Defer a registry until measured evidence justifies its lifecycle cost. |
| Modern theme corpus drives fixture-specific hacks | Admit mechanisms, not theme names. A rule enters core only when it has a semantic owner and a reusable typed representation. |

### Sources and Research

- Mermaid `11.16.0@7c0cafcf`, especially config/theme injection and family render sources under `repo-ref/mermaid/packages/mermaid/src`.
- Local `modern_mermaid@a021cbc`, especially `src/utils/themes.ts`, `src/fonts.css`, and `src/components/Preview.tsx`.
- Existing ADR-0049 for default vendored measurement, ADR-0059 for native export, ADR-0063 for the terminal SVG pipeline, and ADR-0077 as the superseded presentation ownership decision.
- Current `usvg/resvg 0.47`, `fontdb 0.23.0`, `ttf-parser 0.25.1`, and `rustybuzz 0.20.1` source in the resolved Cargo graph.
- `wuff 0.2.8` crate source and conformance documentation for pure-Rust WOFF2 decoding.
- Independent repository agents for modern theme inventory, contract surface, minimal/common/flexible design alternatives, resource architecture, test review, and canvas/effect pipeline placement.

## Implementation Units

### U0. Reconcile the current dirty branch against the superseding plan

- **Covers:** R1-R8, R24, R34.
- **Approach:** Inventory every uncommitted file. Keep the request-scoped site-config materialization fix, the scoped-CSS root-sibling isolation fix, and any source-backed profile behavior separation that still matches KTD9. Remove or rewrite the conditional visual fallback, provisional direct `HostTheme` documentation, shallow API claims, and tests that encode the superseded model. Commit retained orthogonal fixes separately before theme implementation.
- **Files:** Current `git status` set, especially `crates/merman-render/src/presentation/*`, `crates/merman/src/svg/mod.rs`, bindings request code, scoped CSS tests, ADR-0077, presentation docs, examples, and migration guide.
- **Verification:** Diff audit proves no unrelated user changes were deleted; retained scoped-CSS tests cover attribute-qualified `+`, `~`, comments, quoted `]`, escapes, repeated attributes, and unclosed selectors.
- **Done when:** The branch has a reviewable retained baseline and no test or doc claims the superseded visual fallback is final architecture.

### U1. Add the licensed theme capability and source-style corpus

- **Covers:** R38-R41, R46 and AE2-AE6, AE16.
- **Approach:** Create a theme-specific manifest for assets, expected fingerprints, required mechanisms, portability grade, and output coverage. Add representative source diagrams and theme specs for token baseline, mixed-script typography, ordinal palette, layered canvas, Hand Drawn effects, and Aurora residual. Record the 24-theme mechanism matrix without importing application annotation data. Extract a source-backed style-origin/property precedence matrix from Mermaid 11.16.0 for admitted families before U7 changes any adapter.
- **Files:** New `fixtures/themes/` tree, focused test helpers under `crates/merman-render/tests/support/` or a small publish-false fixture crate only if multiple packages need the same validated manifest, and alignment docs.
- **Patterns:** Follow the provenance rigor of `merman-fixture-render-context` without adding theme responsibilities to that crate.
- **Verification:** Manifest path validation, SHA-256 validation, license presence, duplicate ID rejection, compile-only mechanism coverage for all 24 reference themes, and pinned-source citations for class/default-class/assigned-class/inline/directive/themeCSS precedence.
- **Done when:** Every new low-level feature has a fixture consumer before the public API is finalized.

### U2. Implement host-owned policy, canonical font assets, and catalog compilation

- **Covers:** R9-R11, R16, R36, R42-R43, R48-R49.
- **Approach:** Add `ThemeAdmissionPolicy`, capability-set policy resolution, `FontAsset`, `FontFaceMetadata`, `FontCatalogSpec`, `FontCatalog`, embedding/source requirements, generic-family mapping, aliases, fingerprints, and stable resource-limit descriptors. Decode WOFF2 with `wuff` into canonical SFNT and validate with `ttf-parser`. Keep bytes immutable and I/O-free. Add a `theme_font_asset` target to the existing fuzz package.
- **Files:** New `crates/merman-render/src/diagram_theme/admission.rs` and `assets.rs`, workspace and crate manifests, `crates/merman-render/src/resources.rs`, `fuzz/Cargo.toml`, new `fuzz/fuzz_targets/theme_font_asset.rs`, `.github/workflows/fuzz.yml`, `docs/security/FUZZING.md`, fuzz config tests, bindings error/report types, fixture assets.
- **Verification:** Complete capability-set intersection/empty-intersection matrices, reachable `AcceptHostDependent`, custom-catalog rejection of `VendoredDefault`, constructor/request monotonic policy, stable limit IDs and hard caps, TTF/OTF/TTC/WOFF2 success, malformed and collection edge cases, decompression amplification, duplicate IDs, restricted embedding, alias collision, deterministic fingerprint, bounded base64/WOFF2/SFNT/catalog fuzz smoke, and no-default-path regression.
- **Done when:** One compiled catalog can be consumed without reparsing or rediscovering bytes by later adapters, and hostile payloads cannot raise host ceilings or bypass staged accounting.

### U3. Introduce fingerprint-bound prepared text layout

- **Covers:** R12-R16a, R19, R43, R45, R49 and AE2, AE15.
- **Approach:** Replace the custom-font path's loose per-request CSS-family measurement with `TextLayoutBackend::prepare` and `PreparedTextLayout`. Extend resolved typography. Implement native shaping and cluster fallback with `rustybuzz`. Define a versioned `prepare_catalog` request/result/session contract covering backend identity, normalized shaping/style capabilities, direction/script/language/features/variations, loaded catalog faces, optional run evidence, timeout/rejection, cache lifetime, and invalidation. Rust and WASM can implement this lifecycle; Native ABI 3 and bindings without it use native layout. Route layout, wrap, SVG bbox, and computed length through the prepared service. Introduce family-local immutable prepared labels, beginning with `PreparedFlowchartSvgLabel`, so source tokens, visible text, wrapping, bbox height, and computed width are calculated once while preserving host request order. Keep default vendored metrics unchanged.
- **Files:** `crates/merman-render/src/environment.rs`, `crates/merman-render/src/text/*`, new `diagram_theme/typography.rs`, binding host-measurement protocol code and docs, affected family label preparation paths.
- **Verification:** Mixed Latin/CJK fallback, weight/style/feature/variation selection, direction/script/language, tokenize/wrap/measure reuse, host prepare success/mismatch/timeout/invalidation, under-attested-host downgrade, fully attested host admission, missing glyph, each orthogonal policy transition, unchanged request ordering, existing default measurement snapshots, and bounded text/features/variations `rustybuzz` shaping fuzz smoke.
- **Done when:** A custom-font operation cannot measure with a catalog different from the one retained for export, and every applicable family label path reuses one immutable prepared result without changing observable host-measurement request order.

### U4. Make resource-bearing sealed SVG and `RenderedDocument` canonical

- **Covers:** R25-R32, R44, R47-R48 and AE9-AE10, AE17.
- **Approach:** Capture authorized resources in `RenderSession`; retain them in `ResvgCompatibleSvg` at both typed-family and trusted arbitrary-SVG terminal paths. Rebuild `SvgResourceClosure` from final SVG after CSS/postprocessors and reconcile bounded declared resource deltas. Add facade `RenderedDocument` with private construction. Replace the tuple return. Route every binary convenience API through one document. Add resource/theme fingerprints and document-stage residuals to frozen reports. Mark `into_string` as a lossy projection.
- **Files:** `crates/merman-render/src/environment.rs`, `src/svg/pipeline/mod.rs`, `src/family.rs`, `crates/merman/src/svg/operation.rs`, `src/svg/mod.rs`, CLI raw-SVG path, security/export docs.
- **Verification:** typed and trusted raw SVG resource retention, closure reconstruction, declared and undeclared postprocessor resources, unresolved-resource rejection in every export mode, closed-but-`Unverified` `BestEffort` admission, report correlation, compile-fail forgery tests, postprocessor measurement inclusion before report freeze, and all convenience paths using the same document.
- **Done when:** No canonical headless export path can lose or substitute authorized resources or describe stale pre-postprocess resources as complete.

### U5. Make native exporters consume the document font catalog

- **Covers:** R29-R32, R41, R44 and AE2, AE9, AE10, AE14.
- **Approach:** Replace unconditional `shared_system_fontdb` use with a catalog adapter. Build embedded-only databases without system scanning. For embedded-then-system, clone the bounded shared system database, load retained faces, record their IDs, and use a custom resolver that queries retained IDs before system candidates. Preserve the cached legacy database only for system-only. Do not add an unbounded global cache keyed by arbitrary catalogs. Use the catalog default and generic families. Before widening output coverage, run a go/no-go vertical tranche with one licensed mixed-script catalog through U3 prepared layout, U4 sealed SVG, PNG, and PDF. Rename raster/PDF compositing options and emit target-specific portability/admission reports.
- **Files:** `crates/merman-export/src/lib.rs`, facade output methods, CLI/output config, docs and export tests.
- **Verification:** the vertical tranche proves resource identity and reports before broad exporter expansion; embedded-only never resolves an installed font, embedded priority wins family collisions, system-only preserves current behavior, PNG/JPEG/PDF share fingerprints, SVG/PDF can report different justified target outcomes, PDF filter output reports local rasterization, and matte/page-paint semantics remain separate from canvas.
- **Done when:** Export cannot silently choose a font environment that layout did not authorize.

### U6. Add structured SVG resource IDs, canvas, effects, and paint bounds

- **Covers:** R20-R23, R30, R42, R47-R49 and AE4-AE6, AE13, AE17.
- **Approach:** Introduce the scoped ID allocator and structured defs writer. Refactor root SVG assembly so content bounds, stroke outsets, filter bleed, and canvas insets reach `RootViewportPlan` before root completion. Emit trusted font declarations, defs, canvas, and effect bindings in deterministic order. Extend stable resource limits through filter raster planning. Update the canonical Web SVG safety policy and generated VS Code copy for compiler-owned bounded font data. Keep terminal validation as the final authority.
- **Files:** `crates/merman-render/src/svg/parity/root_svg.rs`, new `svg/root_document.rs`, `svg/resource_ids.rs`, and `svg/resource_closure.rs` as justified, new `diagram_theme/canvas.rs` and `effects.rs`, pipeline final validation/reference planning, `platforms/web/src/svg-safety-policy.ts`, `tools/vscode-extension/src/preview-svg-safety-policy.ts`, generation/freshness scripts, exporter filter tests.
- **Verification:** multi-diagram ID isolation, gradient/pattern layering, blend modes, filter graph cycles, primitive/region/pixel caps, bleed/viewBox expansion, deterministic serialization, canonical font base64 plus MIME/signature agreement, SVG font serialization fuzzing and negative resource-reference coverage, Web/VS Code policy freshness, external URL rejection, and no layout feedback loop.
- **Done when:** The admitted canvas/effect corpus survives standalone SVG and enabled native outputs without clipping or external resources.

### U7. Compile semantic theme rules before layout

- **Covers:** R5, R17-R19, R21-R24, R39, R46 and AE3, AE7, AE16.
- **Approach:** Define semantic targets, the applicability matrix, variants, ordinal selectors, typed style patches, style origins, and property admission classes. Resolve a family-local immutable style plan after parse/effective config and before layout using the U1 source-backed precedence matrix. Update family layout/render code to consume that plan and emit semantic bindings. Source properties not admitted as layout-before-freeze, paint-only, or non-visual produce an explicit `Unverified` residual. Do not infer semantics by reparsing final SVG DOM.
- **Files:** New `diagram_theme/semantic.rs`, `source_styles.rs`, `compiler.rs`, `resolved.rs`; family layout/render modules for admitted targets; `TextStyle`; theme CSS compatibility integration.
- **Verification:** ordinal stability under family encounter order, all applicability cases, competing theme/class/default-class/assigned-class/inline/directive values, pre-layout typography effects, paint/layout field separation, unsupported source-property residuals, no selector strings in portable specs, and reference mechanism fixtures across Flowchart, Class, Sequence, State, ER, Gantt, Pie, GitGraph, Timeline, and Ishikawa.
- **Done when:** The modern mechanism matrix is expressed through typed semantic owners rather than CSS selector emulation.

### U8. Replace the public theme and profile model

- **Covers:** R2-R8, R33-R35, R42-R45.
- **Approach:** Introduce `DiagramThemeCompiler`, the final immutable public theme types, orthogonal host policy axes, and staged portability reports. Rename `PresentationProfile` to `RenderProfile`; remove visual fallback and shallow resolved presentation types. Replace `HostTheme` with `ThemeTokens` plus full `DiagramTheme`. Remove target-agnostic `DiagramTheme::portability()`. Rename the private renderer theme shim to avoid two meanings. Make `HeadlessRenderer::with_theme` and `with_render_profile` order-independent and invalidate materialized state correctly.
- **Files:** `crates/merman-render/src/presentation/*` replaced by `diagram_theme/*` and render-profile owner, `crates/merman/src/svg/mod.rs`, examples, tests, public reexports.
- **Verification:** compile-fail removal tests, constructor/request downgrade rejection, orthogonal policy transitions, staged report ownership, setter order and cache invalidation, default parity, profile/theme independence, explicit site-config precedence, and no Redux visual fallback.
- **Done when:** A new user can explain every public concept by lifecycle and ownership without using the word presentation as a catch-all.

### U9. Replace Options JSON, discovery, and first-party consumers

- **Covers:** R35-R37, R42-R49 and AE8, AE11, AE13, AE15, AE18.
- **Approach:** Replace the unreleased presentation group with the closed preset-or-spec schema. Add bounded base64 resource decoding inside a complete spec, explicit omit/null/empty behavior, monotonic constructor/request policy resolution, and bounded redacted diagnostics. Generate discovery from Rust descriptors. Migrate CLI, WASM, FFI, UniFFI, JNI, Typst, examples, and Playground controls. Add a versioned WASM prepare/session API. Keep Native ABI 3 slots unchanged and use native catalog layout plus the existing Options JSON operation path there; typed helpers may accept bytes before constructing the core value.
- **Files:** `crates/merman-bindings-core`, `merman-cli`, `merman-wasm`, `merman-ffi`, `merman-uniffi`, `merman-android-jni`, `merman-typst-plugin`, web tools, discovery docs and tests.
- **Verification:** preset/spec exclusivity, omit/null/empty behavior, array completeness, bounded base64, constructor downgrade rejection, redaction/probing tests, compiled-theme constructor reuse, discovery on slim/full artifacts, unknown newer IDs, CLI config, WASM prepare/session and byte transport, ABI 3 invariance, and cross-language error projection.
- **Done when:** Every first-party surface exposes the same owners and no surface reconstructs theme semantics independently.

### U10. Delete obsolete code and rewrite contracts

- **Covers:** R34, R38-R49.
- **Approach:** Delete old presentation names, profile visual fallback, obsolete descriptors, migration shims, duplicated theme compiler code, root background theme ownership, stale examples, and docs that recommend raw composition as full expressiveness. Update ADR-0077 or supersede it with a new ADR for compiled theme/resource/policy ownership. Preserve the superseded-requirement mapping and write an alpha.3-to-alpha.4 breaking migration table. Update security docs with the trust matrix, browser boundary, host isolation, resource ceilings, and diagnostics contract.
- **Files:** Changelog, ADRs, `docs/security/THREAT_MODEL.md`, `docs/security/RENDERING_SECURITY.md`, rendering docs, Options JSON docs, release guide, examples, discovery docs, deprecated tests and modules.
- **Verification:** repository-wide `rg` proves only intentional historical references remain; docs examples compile; no Compound Engineering badge is added.
- **Done when:** There is one canonical theme architecture and no parallel deprecated model to maintain.

### U11. Run proportional verification, visual proof, simplification, and review

- **Covers:** R1-R49.
- **Approach:** Run focused tests after each unit, then serial cross-package verification. Render representative SVG/PNG/JPEG/PDF outputs and inspect structural/pixel evidence without broad normalization. Run multi-agent correctness, security, API-contract, data/resource, performance, maintainability, and test reviews. Apply valid findings, then run a simplicity pass and commit final docs.
- **Files:** All touched implementation, fixture, test, and documentation files; dated alignment receipt if the project pattern requires one.
- **Verification:** Commands and gates in the Verification Contract.
- **Done when:** All acceptance examples pass, every residual is named, R41-R49 have target/policy/security evidence, and reviewers find no unresolved P0/P1 or contract-breaking P2 issue.

## Verification Contract

### Unit-level commands

Run commands serially and reuse the workspace target directory:

```bash
cargo fmt --all -- --check
cargo nextest run -p merman-render font_catalog
cargo nextest run -p merman-render text_measurement
cargo nextest run -p merman-render --test presentation_test
cargo nextest run -p merman-render --test layout_snapshots_test
cargo nextest run -p merman-export --features png,jpeg,pdf
cargo nextest run -p merman --all-features --test prepared_render
cargo nextest run -p merman --all-features --test presentation_layering
cargo nextest run -p merman --all-features --test resvg_safe_fixture_smoke
cargo nextest run -p merman-bindings-core
cargo nextest run -p merman-cli
cargo nextest run -p merman-wasm --all-features
cargo nextest run -p merman-ffi --all-features
cargo nextest run -p merman-uniffi --all-features
cargo nextest run -p merman-android-jni --all-features
cargo test -p merman-typst-plugin --no-default-features
cargo test -p merman-typst-plugin --no-default-features --features merman-bindings-core/svg,merman-bindings-core/math,merman-bindings-core/system-clock,merman-bindings-core/system-timezone,merman-bindings-core/system-random
cargo run -p xtask -- verify-native-abi
node platforms/web/scripts/svg-safety-policy.test.mjs
python3 scripts/test_fuzz_config.py
cargo +nightly-2026-07-01 fuzz run --fuzz-dir fuzz --sanitizer address theme_font_asset -- -runs=64 -timeout=10 -max_len=262144
```

Rename test filters/files in the command list when the implementation deletes presentation terminology. Record the final exact commands in the completion receipt.

### Cross-target checks

```bash
cargo check -p merman-render --all-features
cargo check -p merman --all-features
cargo check -p merman-wasm
cargo check -p merman-wasm --target wasm32-unknown-unknown --no-default-features --features svg
cargo check -p merman-ffi
cargo check -p merman-uniffi
cargo check -p merman-android-jni
cargo check -p merman-typst-plugin
cargo check -p merman-cli
```

Use package-supported feature sets if a target intentionally excludes native exporters. Do not force a feature closure that the package metadata forbids.

### Binding and consumer gates

Run these after U9 in equipped environments; record any platform gate delegated to CI:

```bash
npm --prefix platforms/web run build
npm --prefix platforms/web test
npm --prefix playground run build
npm --prefix playground test
npm --prefix tools/vscode-extension run check
python3 scripts/verify-platform-bindings.py
python3 scripts/build-python-uniffi-wheel.py --run-smoke
dart run tool/abi3_contract_test.dart  # run from platforms/flutter
```

The final receipt must name the exact artifact-profile feature sets used for FFI, UniFFI, JNI, WASM, and Typst rather than treating a host-target `cargo check` as transport proof. Android instrumentation, Apple/Swift, Flutter native, and C ABI smokes may run in their existing CI jobs when the local machine lacks those toolchains.

### Behavioral gates

- Default no-theme SVG goldens remain unchanged for the representative family matrix.
- Constructor policies cannot be weakened by request themes, source config, or binding defaults.
- Font catalog fingerprints and prepared-backend identities match measurement, sealed SVG resources, document report, and native exporter evidence.
- A host with matching bytes but insufficient shaping/run attestation is `HostDependent`; the native catalog path can satisfy portable admission.
- Compile capability, document residual, and SVG/PNG/JPEG/PDF target reports are distinct and monotonic.
- Theme defaults and every admitted Mermaid source-style origin use the source-backed property/precedence matrix before layout.
- Terminal resource closure catches undeclared postprocessor fonts, external references, and stale catalog claims.
- Strict portability rejects host fonts, raw CSS, custom postprocessors, unsupported filters, and Aurora backdrop blur.
- Best-effort mode records every fallback and approximation.
- Canvas/effect fixtures have non-clipped SVG bounds and nonblank PNG/JPEG/PDF output.
- Resource limits cover encoded JSON/base64, compressed and decoded font bytes, faces, tables, catalog totals, CSS font data, filter primitives/regions/pixels, and postprocessor deltas, with request overlays only tightening ceilings.
- Options JSON and every binding enforce preset/spec exclusivity, omit/null/empty semantics, external-path/URL rejection, and unknown raw structural-input rejection.
- Browser SVG font data passes the canonical Web policy, the generated VS Code copy stays current, and raw CSS security outcomes remain independent from portability grades.
- Public reports are bounded and redact system-font paths, provider IDs, and installed-family inventories.
- `git diff --check` passes and staged commits contain only owned files.

### Visual evidence

- Compare source-backed DOM structure for representative default fixtures.
- Inspect generated `<defs>`, canvas child order, resource IDs, semantic target attributes/classes, viewBox, font declarations, and report fingerprints.
- Use bounded PNG pixel sampling or image hashes only for stable theme-owned paint. Do not assert platform text raster pixels.
- For PDF, assert page geometry, font/resource provenance, filter rasterization plan, and nonblank output rather than byte equality.

### Review gates

- Correctness: rule precedence, cache invalidation, family applicability, and document correlation.
- Security: trust-boundary matrix, monotonic host policies, font decompression/shaping fuzzing, data URLs, CSS/attribute sanitization, filter resource amplification, ID injection, diagnostics redaction, browser-policy parity, and Options JSON limits.
- API contract: breaking rename completeness, compiler/admission ownership, tagged-union schema, staged portability reports, discovery, host prepare/session transport, and ABI 3 invariance.
- Performance: one-time compile/decode, no repeated tokenize/wrap, no unconditional system scan for embedded-only, bounded allocations, and default-path cost neutrality.
- Maintainability: no duplicate theme compilers, no family DOM selector tables, no shallow forwarding aggregates, and no mini browser engine.
- Testing: default parity, representative modern mechanisms, raw SVG resources, all outputs, and negative resource cases.

## Definition of Done

### Global Completion Criteria

- `DiagramTheme` is the only complete visual recipe owner.
- `ThemeTokens` is clearly documented and tested as a convenience adapter, not the full theme model.
- `RenderProfile` owns behavior only and has no visual fallback.
- `ThemeAdmissionPolicy` and `RenderResourcePolicy` are host-owned monotonic ceilings; themes and requests cannot weaken them.
- portability requirement, font-source policy, and host-measurement fallback are distinct public concepts.
- custom font themes use one fingerprinted catalog across measurement, SVG, and native export.
- native catalog layout is the authoritative portable path; host layout requires versioned attestation or is reported as host-dependent.
- canvas, effects, and paint bounds are assembled structurally before terminal SVG validation.
- `RenderedDocument` is the canonical facade-level Mermaid operation completion type; `ResvgCompatibleSvg` remains the low-level sealed-SVG export artifact.
- terminal `SvgResourceClosure` reconciles final resources after CSS and postprocessors.
- compile, document, and exporter portability reports do not claim each other's capabilities.
- raw CSS and custom postprocessors cannot silently retain a portable grade.
- Options JSON is a closed preset-or-complete-spec union, and general binding diagnostics are bounded and redacted.
- Web/VS Code SVG safety policies, rendering threat docs, and the theme-font fuzz target cover the new resource boundary.
- the modern capability corpus proves 23 modeled themes and one explicit Aurora residual.
- provisional presentation APIs, dead code, and obsolete docs are removed.
- focused nextest, cross-target checks, visual evidence, and multi-agent reviews pass.
- local commits are reviewable and no PR is opened.

### Unit Completion Matrix

| Unit | Completion evidence |
| --- | --- |
| U0 | Dirty-branch keep/supersede audit and retained-fix commit |
| U1 | Licensed theme fixture manifest, 24-theme mechanism matrix, and pinned source-style precedence evidence |
| U2 | Monotonic host policy, canonical catalog, staged budget, and byte/container/catalog fuzz evidence |
| U3 | Versioned native/host prepare-session tests plus one-result label reuse, host-request-order evidence, and shaping fuzz evidence |
| U4 | Terminal resource-closure, resource-bearing sealed SVG, and unforgeable `RenderedDocument` tests |
| U5 | Catalog-driven target-specific SVG/PNG/JPEG/PDF vertical tranche, admission, and font resolution tests |
| U6 | Structured canvas/effect/viewBox/ID tests, SVG font serialization fuzzing, and Web/VS Code safety-policy parity |
| U7 | Semantic applicability, source-origin precedence, ordinal, and portability coverage across representative families |
| U8 | Final compiler/policy/Rust API and default/profile/theme precedence tests |
| U9 | Tagged-union Options JSON, redacted discovery, WASM session, CLI, and binding parity tests |
| U10 | Deletion audit, supersession mapping, ADR, security docs, examples, and migration table |
| U11 | Serial verification receipt and resolved review findings |

### Traceability

| Requirement group | Primary units |
| --- | --- |
| R1-R8 | U0, U8, U10 |
| R9-R16a | U1, U2, U3, U5 |
| R17-R24 | U1, U6, U7 |
| R25-R32 | U4, U5, U6 |
| R33-R37 | U8, U9, U10 |
| R38-R41 | U1, U10, U11 |
| R42-R45 | U2, U3, U5, U8, U9, U11 |
| R46-R49 | U1, U2, U3, U4, U6, U7, U9, U10, U11 |

## Implementation-Time Unknowns

- Confirm `wuff 0.2.8` against the repository MSRV, no-default-feature WASM build, decompression limits, and the W3C WOFF2 conformance corpus before making it a workspace dependency. Replace it only with a documented pure-Rust alternative that satisfies the same contract.
- Decide the exact semantic target catalog only after each participating family identifies stable model-owned targets. The plan forbids raw selectors but does not force speculative targets that no renderer can bind.
- Measure the artifact-size cost of typed embedded `@font-face` data. A target admitted under `RequirePortable` always emits self-contained data when embedding permissions allow it; host-dependent modes may deliberately omit it and must downgrade the target report.
