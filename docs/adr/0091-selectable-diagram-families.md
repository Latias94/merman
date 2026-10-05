# ADR-0091: Selectable Diagram Families

## Status

Accepted for the current development source and the next release after `0.8.0-alpha.6`.

## Date

2026-09-26

## Context

An embedded Markdown consumer needs Flowchart and Gantt without carrying implementations for
other diagram families. Selecting only SVG previously removed optional operations and engines,
but retained every parser and typed model. Filtering a runtime registry would not remove those
implementations or establish a smaller final native artifact.

The existing family-owned architecture remains the semantic boundary. A new crate per family
would add package and release coordination without changing that ownership. Cargo can select
implementations inside the existing crates while retaining their shared parser and layout code.

## Decision

1. Add positive `diagram-*` selectors for the 34 logical built-in families and `all-diagrams` as
   their union. Aliases belong to one owner. Selectors enable no optional output, layout engine,
   math backend, or system adapter by themselves.
2. Keep a single parser-owned catalog with complete lightweight identities, aliases, detectors,
   headers, and configuration namespaces. Conditionally bind its parser, typed-model, and editor
   callbacks. Compiled implementation enumerations and complete known-family metadata are
   distinct projections of that catalog.
3. Compile out family-exclusive parser modules, generated grammar inclusions, semantic types,
   layout code, and output handlers. Keep genuinely shared implementation under the union of its
   owners. Flowchart and Swimlane share grammar/models and existing cross-layout routes while
   retaining separate language admission. Mindmap may retain the label helpers it uses.
4. Preserve empty defaults in low-level crates. The facade defaults to `all-diagrams + complete-svg-elk`;
   CLI defaults explicitly include all families; Rustdoc defaults to
   `all-diagrams + svg + layout-cytoscape + layout-elk`. `complete-svg` continues to mean
   `svg + layout-cytoscape + math`, and `complete-svg-elk` continues to add ELK.
5. Forward selectors through existing dependency edges. Facade selectors weak-forward into
   optional renderer and language-tool crates. Cargo feature union can independently widen core;
   every local output consumer must handle core variants for which it has no handler.
6. Keep disabled known built-ins distinct from unknown input. Strict parsing returns
   `UnsupportedDiagram`; suppressed semantic/render parsing produces Error, while suppressed
   unknown input returns `None`. Cancellation remains unsuppressed and custom registry overlay
   precedence remains unchanged. Editor diagnostics preserve the original error; actionable
   headers/templates require a compiled parser.
7. Validate local render availability after cancellation, provenance, and semantic/layout pair
   checks, before optional backend planning. Report a missing local handler explicitly rather
   than returning a ready plan. Once the handler exists, preserve existing missing-engine and
   resource error ordering. Parser availability does not imply SVG or ASCII support.
8. Keep selectors separate from runtime capability IDs. `feature-surface-v1.json` retains its
   output/API/engine vocabulary. Migrate artifact profiles and readers together to
   `artifact-profiles-v2.json`, schema 2, with required `expected.diagram_families`: sorted unique
   compiled logical parser IDs, excluding Error infrastructure. Verify the actual built target's
   parser set, not the verifier's own linked core. Existing distributed recipes retain all
   families; this decision creates no additional prebuilt products.

## Compatibility and migration

Direct low-level users and facade users with defaults disabled must add `all-diagrams` to retain
the previous parser surface, or enable the families they require. Public family-exclusive model
types and enum variants are conditional; consumers must select their owners and gate imports and
matches as appropriate. Error and CustomJson infrastructure remain available without a family.
Cargo features are additive, so another dependency enabling all core families can widen an
application's parser set. A no-default edge cannot enforce exclusions on the whole graph.

The [feature guide](../FEATURES.md#select-diagram-families) owns the copyable Flowchart + Gantt and
parser-only embedding recipes. Published alpha.6 packages retain their previous feature surface;
this decision requires neither a version bump nor a publication operation on its own.

## Superseded scope

- ADR-0073 retains family-owned semantics, one catalog, typed rendering, and custom registry
  ownership. This decision replaces the assumption that complete identity implies compiled
  implementation availability, and narrows actionable editor suggestions to available parsers.
- ADR-0076 retains the runtime capability vocabulary, positive feature model, and exact artifact
  recipe authority. This decision supersedes its rejection of one-feature-per-diagram designs
  and its artifact schema-1 reference. Selectors are a separate namespace, not new capability IDs.
- ADR-0088 retains explicit math, and ADR-0089 retains the Mermaid 12 ELK product defaults. This
  decision adds `all-diagrams` to those defaults and supersedes the statement that no per-diagram
  feature exists. Explicit SVG aggregates retain their output/backend membership.

## Verification and consequences

Use existing feature and artifact validators, isolated consumer fixtures, and a bounded matrix:
no-family, singleton, Flowchart + Gantt, Flowchart/Swimlane separation, all families, and core-wide
with a narrower renderer. Check that omitted public types cannot be imported, selected behavior
matches the pinned source, and disabled-family errors preserve their public contracts. Reuse
existing CI/release paths rather than introducing a source-analysis compiler inside xtask.

The native acceptance experiment compares pre-change full, candidate full, and candidate
Flowchart + Gantt using the same runtime-fed consumer, toolchain, target, lock resolution, profile,
LTO, panic, stripping, engines, math, and outputs. Measure final executable or cdylib bytes and
preserve each artifact before another build. The subset must be smaller than candidate full and
pass the same exercised corpus. The [measurement report](../performance/diagram_selection_native_2026-09-26.md)
owns the results; no reduction or completed measurement is asserted by this ADR.

Shared implementation and additive feature union bound the achievable reduction. The feature
surface permits explicit embedding tradeoffs while preserving the full default experience.
