# ADR 0077: Presentation, Theme, Mermaid Config, And SVG Output Ownership

- Status: superseded
- Date: 2026-08-02
- Superseded: 2026-08-10 by the alpha.4 typed diagram-theme architecture

> [!IMPORTANT]
> This ADR records an interim prerelease design and is not the current API contract. Alpha.4
> removed `HostTheme`, `PresentationProfile::MermanModern`, `presentation.*`, and the associated
> profile/aspect discovery surface. The current experimental path is
> `DiagramThemeSpec`/`ThemePreset` -> `DiagramThemeCompiler` -> `DiagramTheme` ->
> `RenderRequest::with_theme`, with official Mermaid configuration, layout, runtime policy, and
> SVG output kept in their existing independent owners. Options JSON uses top-level `theme` with
> exactly one `preset` or complete `spec`. No mixed product-profile compatibility alias remains,
> and the C6 cross-family SVG/PNG/PDF portability matrix is not yet proven.

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
   - `HeadlessRenderer` accepts these owners directly through `with_presentation_profile`, `with_host_theme`, `with_site_config`, and `with_svg_pipeline`. A public `Presentation` aggregate is rejected because it adds no invariant or behavior of its own.

2. The seven editor presets remain theme-only data. Selecting one does not change the SVG output pipeline or root background. Mermaid defaults are represented by no presentation selection, not a `mermaid` preset.

3. `merman-modern` is one first-party presentation profile with independently resolved aspects.
   - Behavior defaults provide Neo look independently from visual theme selection.
   - A Redux/slate visual fallback is applied only when neither a non-empty host theme nor a later site-level Mermaid `theme` owns the appearance.
   - A private Flowchart SVG aspect provides compact routed corners and padded edge-label masks.
   - An optional Flowchart layout aspect defaults ordinary Flowcharts to ELK.
   - Non-Flowchart inputs do not require ELK. An explicit non-ELK `flowchart.defaultRenderer` disables only the layout aspect for an ordinary Flowchart. A `flowchart-elk` source always requires ELK.

4. The private Flowchart policy is typed and travels with the prepared render operation to the Flowchart SVG renderer. It does not enter `MermaidConfig` or `LayoutExecution`. Official effective config continues to own detector selection, Neo sizing, and ELK layout.

5. Configuration precedence is structural and independent of builder call order:
   1. base `Engine` config;
   2. presentation profile behavior defaults;
   3. the profile visual fallback when no later theme owner exists;
   4. explicit host theme data;
   5. explicit renderer or binding `site_config`;
   6. source frontmatter and directives, subject to hardened secure keys.

6. Omitting both presentation inputs contributes no override and preserves Mermaid parity. In reusable-engine Options JSON, omitted or empty presentation fields continue to inherit constructor values through the binding overlay. Schema 2 does not add nullable clear operations; callers that need a parity renderer use an engine without base presentation inputs.

7. Runtime discovery reports known presentation entries separately from artifact availability. Profile aspects expose applicability and missing capability IDs so a slim artifact can accept a known profile for operations that do not activate its unavailable aspect.

8. Stable profile IDs preserve semantic intent, aspect boundaries, and override behavior rather than pixel-identical output. Source-backed improvements may evolve inside those boundaries; a materially different product bundle requires a new profile ID.

9. Native ABI 3 remains unchanged. Options JSON schema 2 and high-level source APIs may break before alpha.4 because their final contracts have not been released.

## Consequences

- Default rendering remains byte-compatible because no presentation selection produces no Mermaid patch, private policy, or output change.
- Rust and binding implementations can share one presentation resolver instead of compiling theme and output behavior independently.
- Ordinary Rust callers configure profile and host theme directly on `HeadlessRenderer`; low-level resolved-policy types remain hidden from the facade.
- Host applications can combine editor tokens, official Mermaid config, product presentation, and output compatibility without a preset lattice.
- Exact CSS-heavy or structural theme recipes use `MermaidConfig` and `SvgPipeline` instead of expanding `HostTheme` into an arbitrary CSS/SVG container.
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
6. Keep a public `Presentation` value that only stores optional profile and theme fields.
   It is a shallow forwarding container with no independent invariant. The renderer already owns configuration precedence and cache invalidation, so direct setters produce a smaller and more truthful public API.
7. Add arbitrary CSS, backgrounds, SVG definitions, and resource loading to `HostTheme`.
   Those values have different lifecycle, security, layout, and export semantics. Composite application themes should combine the existing owners rather than turning the semantic theme adapter into a second rendering configuration language.
