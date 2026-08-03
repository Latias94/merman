# ADR 0077: Presentation, Theme, Mermaid Config, And SVG Output Ownership

- Status: accepted
- Date: 2026-08-02

## Context

The alpha.3 `HostThemeProfile` API combines four independent decisions: host semantic theme tokens, arbitrary Mermaid configuration, Merman-owned presentation behavior, and SVG output cleanup. It also represents `merman-modern` and Mermaid defaults as theme presets even though the former selects product behavior and the latter is simply the absence of an override.

This mixed owner creates observable defects. Theme presets silently select `resvg-safe` output, Rust builder order changes configuration precedence, Merman-only Flowchart keys appear in Mermaid configuration, and a reusable renderer cannot explain which part of a product profile is active for the current diagram and compiled capabilities.

PR #28 added source-backed ELK processing, Neo geometry, route cutting, compact edge corners, and padded edge labels. The implementation must preserve those corrections while separating the product-specific policy from official Mermaid semantics.

## Decision

1. Host theme data, first-party presentation profiles, official Mermaid config, and SVG output policy have separate owners.
   - `HostTheme` owns optional appearance, typography, semantic roles, and a series palette.
   - `PresentationProfile` owns named Merman product behavior. The first profile is `merman-modern`.
   - `MermaidConfig` remains the authority for official Mermaid fields such as `theme`, `themeVariables`, `themeCSS`, `look`, `layout`, `flowchart.defaultRenderer`, and `elk.*`.
   - `SvgOutputPolicy` and `SvgPipeline` remain the only owners of parity, readable, resvg-safe, scoped CSS, background, CSS override, and duplicate-fallback behavior.

2. The seven editor presets remain theme-only data. Selecting one does not change the SVG output pipeline or root background. Mermaid defaults are represented by no presentation selection, not a `mermaid` preset.

3. `merman-modern` is one first-party presentation profile with independently resolved aspects.
   - Global defaults provide the Redux/slate palette and Neo look.
   - A private Flowchart SVG aspect provides compact routed corners and padded edge-label masks.
   - An optional Flowchart layout aspect defaults ordinary Flowcharts to ELK.
   - Non-Flowchart inputs do not require ELK. An explicit non-ELK `flowchart.defaultRenderer` disables only the layout aspect for an ordinary Flowchart. A `flowchart-elk` source always requires ELK.

4. The private Flowchart policy is typed and travels with the prepared render operation to the Flowchart SVG renderer. It does not enter `MermaidConfig` or `LayoutExecution`. Official effective config continues to own detector selection, Neo sizing, and ELK layout.

5. Configuration precedence is structural and independent of builder call order:
   1. base `Engine` config;
   2. presentation profile defaults;
   3. explicit host theme data;
   4. explicit renderer or binding `site_config`;
   5. source frontmatter and directives.

6. An empty presentation layer contributes no override. It preserves Mermaid parity when no lower presentation exists and inherits constructor presentation in a reusable-engine request. Schema 2 does not add nullable clear operations; callers that need a parity renderer use an engine without a base presentation.

7. Runtime discovery reports known presentation entries separately from artifact availability. Profile aspects expose applicability and missing capability IDs so a slim artifact can accept a known profile for operations that do not activate its unavailable aspect.

8. Stable profile IDs preserve semantic intent, aspect boundaries, and override behavior rather than pixel-identical output. Source-backed improvements may evolve inside those boundaries; a materially different product bundle requires a new profile ID.

9. Native ABI 3 remains unchanged. Options JSON schema 2 and high-level source APIs may break before alpha.4 because their final contracts have not been released.

## Consequences

- Default rendering remains byte-compatible because no presentation selection produces no Mermaid patch, private policy, or output change.
- Rust and binding implementations can share one presentation resolver instead of compiling theme and output behavior independently.
- Host applications can combine editor tokens, official Mermaid config, product presentation, and output compatibility without a preset lattice.
- Merman-only Flowchart behavior is no longer advertised as Mermaid config and cannot be activated by raw site config.
- Capability discovery can remain truthful for full and slim artifacts without rejecting valid non-Flowchart operations.
- Alpha.3 callers must migrate mixed `HostThemeProfile` and `host_theme` usage to the separate theme, presentation, top-level `site_config`, and SVG output owners.

## Rejected Alternatives

1. Keep extending `HostThemeProfile`.
   This preserves the ownership defect and makes every future profile combine unrelated decisions.
2. Add a generic presentation plugin registry or options DSL.
   There is one first-party implementation and no second provider that justifies a public strategy abstraction.
3. Put private Flowchart policy in layout interfaces.
   The current private values are consumed by SVG edge paths, label masks, and viewBox calculation; widening layout APIs would create an unused abstraction.
4. Treat root `layout` as an override for the profile's Flowchart renderer.
   The pinned Mermaid detector derives Flowchart layout from `flowchart.defaultRenderer`; changing that precedence would require a separate provenance model and could break parity.
5. Preserve editor-theme output coupling as compatibility behavior.
   Output compatibility is an explicit host decision and already has a dedicated pipeline owner.
