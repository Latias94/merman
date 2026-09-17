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
