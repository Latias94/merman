# Theme Interface Reference Models

**Originally researched:** 2026-08-14

**Status:** non-normative research reference. ADR-0082 supersedes this note for all Merman
authoring-contract decisions.

**Research value: high** -- Mature UI and editor projects converge on simple public tokens,
semantic aliases, component/slot recipes, scoped escape hatches, and explicit override precedence.
Merman adopts only the subset recorded in ADR-0082; recommendations below are evidence and design
history, not an alternative interface specification.

> **Normative outcome:** use
> [ADR-0082](../adr/0082-versioned-theme-authoring-facade.md) for `ThemeDefinitionV1`, token fields
> and defaults, expansion order, null/clear behavior, palette collisions, materialization result
> types, digest ownership, and trace maturity. The portable-theme plan owns delivery gates. If this
> note conflicts with either document, this note loses.

## Scope and Evidence Boundary

This note compares first-party theme interface designs that are directly relevant to Merman's
upper-level diagram theme wrapper:

- Ant Design 5 design tokens.
- Material UI theme customization.
- Chakra UI and Panda CSS tokens, semantic tokens, recipes, slot recipes, and conditions.
- Visual Studio Code color themes and user overrides.
- Tailwind CSS v4 theme variables.
- Mermaid `themeVariables` and `themeCSS`, treated as compatibility input and a cautionary model.

The focus is interface shape, not visual taste. Merman is a Rust Mermaid renderer for editors,
static sites, bindings, and deterministic exports. It needs user customization, not a product
branding system. Existing Merman docs already define a lower-level experimental complete
`DiagramThemeSpec` with `mermaid`, `typography`, `styles`, `canvas`, `effects`, `requirements`, and
`assets`, plus the important rule that theme selection is a complete value rather than a generic
deep-merge field. This note is about the simpler wrapper that should sit above that lower-level
compiler surface.

## Executive Conclusion

The comparison supports one architectural conclusion: ordinary authors need a deep pre-render
module, not direct exposure to the complete theme program and not a second styling language.
ADR-0082 narrows the research options into this proposed version 1 decision:

1. `ThemeDefinitionV1` contains a small cross-family `ThemeTokensV1` record and the existing typed
   `ThemeRuleSet` for advanced overrides.
2. One Rust-owned `ThemeMaterializer` lowers that definition into a complete
   `DiagramThemeSpec`; bindings do not implement their own expansion.
3. Token expansion is a fixed, versioned table rather than a public Seed/Map/Alias algorithm or a
   new semantic/family recipe hierarchy.
4. Version 1 has no `mode` condition. Light and dark are separate definitions, which keeps replay
   independent of render-time context.
5. Radius, spacing, shadows, family slots, arbitrary algorithms, CSS variables, and DTCG exchange
   remain rule-level, alpha, or future adapter concerns rather than stable token fields.
6. Materialization diagnostics are public-contract candidates, while detailed expansion traces and
   inspection remain alpha/debug-only.

Ant Design remains the strongest reference for deterministic derivation, Chakra/Panda for typed
semantic authoring, VS Code for inspection, Tailwind for namespace discipline, and Mermaid for
compatibility constraints. None is copied as Merman's public schema.

## Reference Models

### Ant Design 5

Ant Design exposes `theme` through `ConfigProvider`. The public layers are global `token`,
`algorithm`, component token overrides, CSS variables, nested inheritance, and theme debugging
tools. Its documentation names a three-layer token derivation chain: Seed Tokens express initial
design intent, Map Tokens are derived from Seed Tokens, and Alias Tokens are derived from Map
Tokens. Ant Design also lets algorithms derive map values from seed values, and supports preset
algorithms such as default, dark, and compact. Component tokens can override global tokens, and
component-local algorithms are opt-in rather than always inherited. Nested providers inherit
unchanged token values from parents.

Interface lessons for Merman:

- Use a derivation pipeline. Users should provide small seed inputs like `accent`, `background`,
  `surface`, `text`, `radius`, and `shadow`, then Merman derives map values such as muted surfaces,
  borders, label backgrounds, marker paints, and chart series.
- Separate primitive design intent from semantic diagram roles. Ant Design's Alias Token idea maps
  well to Merman roles such as `node.fill`, `edge.stroke`, `cluster.label`, and `note.background`.
- Add an algorithm hook, but keep it closed and named at first: `algorithm: ["dark", "compact"]`
  or `mode: "dark"`, not arbitrary user code in portable bindings.
- Component tokens map to diagram-family recipes. A future `families.flowchart.node` override
  should be possible, but ordinary users should not need it.
- Include a theme inspector or catalog export. Ant Design has official tooling for debugging
  tokens; Merman already has artifact metadata and should project a user-readable effective theme.

Do not copy:

- React context semantics. Merman's engine/request inheritance is not a component tree.
- Runtime CSS-in-JS machinery.
- Arbitrary algorithm functions, because Merman needs JSON, Rust, Node, Python, WASM, and
  deterministic rendering portability.

Primary source: Ant Design customize theme docs describe `token`, algorithms, component tokens,
nested inheritance, `getDesignToken`, theme editor, Seed/Map/Alias derivation, `components`, and
`cssVar`.

### Material UI

Material UI's `createTheme` model is broad and pragmatic. The main variables are `palette`,
`typography`, `spacing`, `breakpoints`, `zIndex`, `transitions`, and `components`. The `components`
key centralizes component `defaultProps`, `styleOverrides`, and variants. `styleOverrides` are
slot-keyed, with `root` as the outer slot. Variants are matching rules over component props, and
later rules win when precedence matters. MUI also has CSS theme variables; it reserves an
auto-generated `vars` field and documents CSS variables as improving debugging, third-party
integration, and multi-color-scheme behavior, while acknowledging HTML-size tradeoffs.

Interface lessons for Merman:

- Use conventional top-level buckets: `palette`, `typography`, `spacing`, `shape`, `stroke`,
  `shadow`, and `components` or `families`.
- Represent variants as match rules over stable semantic props: `target: "node"`,
  `variant: "primary"`, `family: "flowchart"`, `slot: "label"`. Rule order should be explicit and
  deterministic.
- Slot names are useful for diagrams: `node.body`, `node.label`, `edge.path`, `edge.marker`,
  `cluster.background`, `cluster.label`, `actor.box`, `activation.bar`.
- Reserve generated fields. MUI rejects user-supplied `vars`; Merman should similarly reserve
  `compiled`, `resolved`, `evidence`, or `internal`.
- CSS-variable export can help editor/static-site users inspect and restyle surrounding pages, but
  it should be an export artifact, not the primary compile input.

Do not copy:

- General CSS object values in the public portable schema.
- Component `defaultProps`; diagrams are parsed documents, not prop-driven component instances.
- Heavy variant matrices unless the matching domain is small and catalog-backed.

Primary sources: MUI theming docs list the main theme variables and nested providers; themed
component docs define `components`, default props, slot-keyed `styleOverrides`, and variant arrays;
CSS theme variable docs describe debugging, global availability, multi-scheme support, and
tradeoffs.

### Chakra UI and Panda CSS

Chakra UI v3 explicitly builds its theming system around Panda CSS. Both systems model tokens as
platform-agnostic design decisions with a required `value` wrapper and optional descriptions.
They support primitive tokens, semantic tokens that usually reference primitive tokens, nested
tokens with `DEFAULT`, conditional semantic values for modes such as dark, and type generation for
developer feedback. Chakra's system object can return raw token values or CSS variables, and
semantic tokens intentionally resolve as CSS variables because their value may change by condition.

Recipes and slot recipes are the most relevant concept for diagrams. Recipes contain `base`,
`variants`, `compoundVariants`, and `defaultVariants`. Slot recipes add a `slots` list and per-slot
base and variant styles. Panda pre-generates CSS for recipe combinations; Chakra provides runtime
helpers. Conditions are named selectors or at-rules, with Panda requiring a placeholder for current
selector placement and supporting condition extension.

Interface lessons for Merman:

- Use `{ value, description? }` internally or in an advanced schema, but expose a shorthand for
  ordinary JSON users. A user should be able to write `"accent": "#6366f1"` without wrapping it.
- Add semantic tokens for diagram roles. A high-level `semantic` section can reference primitives:
  `node.fill: "{palette.surface}"`, `edge.stroke: "{palette.muted}"`, `note.fill:
  "{palette.warningSurface}"`.
- Support conditional values for a closed set of Merman-owned conditions: light/dark,
  output target, and later family capability. Arbitrary selectors should not be portable input.
- Model Flowchart/Sequence/State differences as slot recipes, not as raw renderer internals.
  Sequence can expose `actor`, `message`, `activation`, and `note` slots; Flowchart can expose
  `node`, `edge`, `cluster`, `label`, and `marker`; State can expose `state`, `transition`,
  `composite`, and `note`.
- Generate types or schemas from the catalog. Chakra and Panda use typegen; Merman can generate
  JSON Schema plus Rust/TS/Python surface docs from `theme-catalog`.

Do not copy:

- Full CSS pseudo-condition syntax. Merman diagrams are not DOM authoring surfaces.
- Panda's build-time atomic CSS extraction model. Merman renders SVG/PDF/raster from Rust.
- Chakra's React hooks and provider API.

Primary sources: Chakra overview, token, semantic token, recipe, slot recipe, and condition docs;
Panda token, recipe, slot recipe, and condition docs.

### Visual Studio Code

VS Code separates workbench UI colors from editor token colors. Workbench colors are named UI
locations, with user overrides through `workbench.colorCustomizations`. Syntax color overrides use
`editor.tokenColorCustomizations`. Semantic tokens add a separate selector language:
`type.modifier:language`, plus standard token types/modifiers, extension-declared custom token
types, subtype inheritance for styling, and fallback to TextMate scopes when semantic rules are
missing. VS Code also gives users a scope inspector to see computed token information and the
styling rules that apply.

Interface lessons for Merman:

- Split "diagram surface colors" from "source editor highlighting". Merman already has editor
  semantic-token contracts; the diagram theme API should not mix rendered diagram styling with
  Mermaid source highlighting.
- Add a user override layer that can be scoped by theme or family, similar to VS Code's
  per-theme settings. For example: `overrides["editor-dark"].semantic.node.primary.fill`.
- Provide an inspector. Merman should be able to answer: for this SVG element or semantic target,
  which rule applied, which values were derived, and which values fell back.
- Define a standard semantic role catalog and allow custom roles only with a declared super-role.
  VS Code's semantic subtype model is a good fit for theme inheritance without forcing every
  renderer to understand every custom role.
- Fallback must be visible. VS Code has TextMate fallback; Merman can fall back from a family slot
  to a semantic role, then to derived map tokens, then to Mermaid compatibility values, while
  reporting that chain.

Do not copy:

- TextMate scope strings as a public diagram styling selector.
- Language-token selector complexity for visual diagram elements unless a future editor-facing
  source theme explicitly needs it.

Primary sources: VS Code color theme guide, theme color reference, semantic highlighting guide, and
user theme configuration docs.

### Tailwind CSS v4

Tailwind v4 moved theme configuration into CSS with the `@theme` directive. Theme variables are
special CSS variables that also determine which utility classes exist. Namespaces such as
`--color-*`, `--font-*`, `--text-*`, `--spacing-*`, `--radius-*`, `--shadow-*`, and
`--drop-shadow-*` map to corresponding utilities. Users extend defaults by adding variables,
override defaults by redefining variables, remove a namespace with `--color-*: initial`, or disable
all default variable-driven utilities with `--*: initial`. `@theme static` forces all variables to
be generated, and themes can be shared as ordinary CSS files.

Interface lessons for Merman:

- Namespace discipline matters. Merman should prefer stable namespaces like `palette.*`,
  `typography.*`, `radius.*`, `stroke.*`, `shadow.*`, `semantic.node.*`, and
  `families.flowchart.*`.
- Treat unknown namespace resets carefully. A future "strict custom theme" mode can require all
  needed roles, but the default user mode should extend Merman's defaults.
- CSS-variable export is valuable for static-site integration and docs examples. A generated
  `:root` or SVG-scoped CSS variable file can make a Merman theme portable across host pages.
- Tailwind's idea that variables determine available utilities maps to Merman catalogs: published
  theme roles should determine what users can override and what diagnostics can name.

Do not copy:

- CSS as the only authoring language. Merman needs JSON-compatible, strongly validated, portable
  config.
- Utility-class generation. Merman is not a class authoring framework.
- Arbitrary `initial` namespace deletion as the default behavior; diagram readability needs
  fallback.

Primary source: Tailwind theme variable docs.

### Mermaid

Mermaid's official theming model is configuration-oriented. It supports site-wide initialization
and diagram frontmatter, a small set of named themes, and customization through `themeVariables`.
The docs describe `base` as the modifiable theme for custom themes. Some color variables are
derived from other variables for readability, and the engine recognizes hex colors rather than
CSS color names for theme calculations. The config schema also includes `themeCSS` as a raw string,
and lists `look`, layout, limits, and many diagram-specific config sections in the same broad
configuration space.

Interface lessons for Merman:

- Keep Mermaid compatibility exact and bounded. `theme`, `themeVariables`, `themeCSS`, `look`, and
  diagram configs are interoperability inputs, not necessarily a good high-level user API.
- Preserve Mermaid precedence, because users expect frontmatter/directives to work.
- The derived-color idea is useful, but the variable names are historical rather than generally
  semantic. Merman should project them into a typed role layer before renderers consume them.
- Raw CSS must remain a quarantined escape hatch. Merman already documents that general bindings
  reject raw `themeCSS` in the typed theme surface and route bounded Mermaid behavior through
  explicit owners; that boundary should stay.

Do not copy:

- `themeVariables: any` as the main schema.
- Mixing `theme`, `themeCSS`, `look`, layout, security, and resource limits in one user mental
  bucket.
- Raw CSS as the answer for renderer-owned effects such as SVG filters, clipping margins,
  text metrics, or PDF/raster portability.

Primary sources: Mermaid theming docs and config schema docs.

## ADR-0082 Disposition

The research originally explored a public primitive/semantic/family hierarchy with a `mode` field,
radius, spacing, stroke, shadow, aliases, and slot recipes. ADR-0082 deliberately chooses a smaller
version 1 interface. The following table records the disposition so the earlier reference-model
notes are not mistaken for active requirements.

| Research pattern | Version 1 decision | Later option |
| --- | --- | --- |
| Seed/Map/Alias derivation | Keep the derivation idea inside one fixed Rust-owned expansion table. Do not expose an algorithm interface. | A new expansion version may add deterministic derivations. |
| Primitive palette buckets | Replace them with the flat cross-family `ThemeTokensV1` record defined by ADR-0082. | Import adapters may accept other naming systems and lower into the record. |
| Public semantic alias tree | Do not add one. Expansion emits existing typed `ThemeTarget` rules. | Reconsider only if the complete `ThemeRuleSet` cannot express a proven author need. |
| Family and slot recipes | Use the existing family-scoped `ThemeRuleSet`; do not wrap it in another recipe AST. | Catalog-backed helpers may be added without changing renderer input. |
| `mode: light|dark` | Excluded. Authors materialize separate definitions. | A future multi-mode exchange adapter may produce multiple definitions. |
| Radius, spacing, stroke width, and shadow tokens | Excluded from stable tokens until expansion, resource, family-consumer, and C6a evidence is unambiguous. | Remain available through complete typed rules/specs where supported. |
| Preset overrides | Excluded. Materialize a preset to a complete editable spec. | A separately designed typed patch contract could be considered later. |
| Inspector | Keep as alpha/debug and join authoring provenance with compiler resolution. | Stabilize only after real consumers prove a bounded result shape. |
| CSS variables and DTCG | Treat as possible import/export formats, never renderer input. | Add adapters after the native authoring contract is stable. |
| Mermaid variables and CSS | Preserve bounded Mermaid compatibility; keep raw CSS quarantined. | No general CSS styling language is planned for ordinary bindings. |

### Normative authoring shape

The version 1 input is intentionally small:

```json
{
  "authoring_schema_version": 1,
  "expansion_version": 1,
  "tokens": {
    "canvas": "#f8fafc",
    "surface": "#ffffff",
    "text": "#172033",
    "border": "#cbd5e1",
    "line": "#64748b",
    "accent": "#6366f1",
    "series": ["#6366f1", "#16a34a", "#d97706"],
    "typography": {
      "font_stack": ["Inter", "system-ui", "sans-serif"],
      "font_size_px": 14,
      "line_height": 1.4
    }
  },
  "styles": [
    {
      "kind": "rule",
      "family": "sequence",
      "target": "note",
      "style": { "fill": "#fff7ed" }
    }
  ]
}
```

The omitted token fields use the exact ADR defaults. The `styles` field is the existing typed rule
language, not a new semantic or slot wrapper. At render time callers still select either a preset or
the complete materialized spec:

```json
{ "theme": { "preset": "editor-light" } }
```

or:

```json
{ "theme": { "spec": { "styles": [], "typography": {}, "canvas": {} } } }
```

Editing a preset first resolves the current catalog entry to an immutable preset ID/revision pair,
calls `materialize_preset(preset_ref)`, then edits the returned complete spec. The result is a
`MaterializedPreset`, not a `MaterializedTheme` with invented authoring versions.

### Diagnostics and inspection

The reference projects validate the need for generated types, live previews, constrained
namespaces, and inspector tooling. ADR-0082 separates their maturity:

- materialization diagnostics use versioned codes, severity, and authoring paths;
- fatal errors return no partial spec;
- canonical authored-definition and complete-spec bytes are the current identity surfaces; a
  separate materialization or preset digest should exist only after a real cache or replay consumer
  requires it;
- expansion trace is returned only by an internal or explicitly alpha inspection operation; and
- only compiler resolution may identify a theme-internal winner, while only a concrete render report
  may claim actual application, residuals, portability, or admission.

Diagnostics should use public authoring paths and semantic targets. They should not expose filter
regions, evidence ledgers, route-cutover receipts, renderer taxonomy, or resource-policy internals as
ordinary authoring concepts.

### Plan fit

The portable-theme addendum now treats ADR-0082 as the sole authoring-contract authority and keeps
the authoring work separate from renderer capability deepening. The implementation order is:

1. close the current C1-C5 correctness and resource gates;
2. complete the native 18-cell C6a engine gate;
3. prove the pre-freeze family consumers and every candidate expansion row;
4. implement the Rust-owned executable token and expansion table plus authoring witnesses;
5. declare an alpha C7a candidate, then migrate first-party bindings and examples; and
6. freeze C7a only after the author-task rollout verification.

This research note does not make C7a eligible. C7a remains blocked, and none of the deferred ideas
above should widen the public interface while that gate is open.

## Sources

- Ant Design, [Customize Theme](https://ant.design/docs/react/customize-theme/): design token
  customization, Seed/Map/Alias derivation, algorithms, component tokens, inheritance, CSS
  variables, and theme editor.
- Material UI, [Theming](https://mui.com/material-ui/customization/theming/): theme provider,
  top-level theme variables, custom variables, TypeScript augmentation, and nesting.
- Material UI, [Themed components](https://mui.com/material-ui/customization/theme-components/):
  component default props, slot-keyed style overrides, and variant matching.
- Material UI, [CSS theme variables](https://mui.com/material-ui/customization/css-theme-variables/overview/):
  CSS-variable motivations, debugging, multi-scheme behavior, and tradeoffs.
- Chakra UI, [Theming overview](https://chakra-ui.com/docs/theming/overview): Panda-based system
  architecture, config keys, conditions, strict tokens, typegen, and system token APIs.
- Chakra UI, [Tokens](https://chakra-ui.com/docs/theming/tokens): token value wrapper, references,
  nesting, and token types.
- Chakra UI, [Semantic Tokens](https://chakra-ui.com/docs/theming/semantic-tokens): semantic token
  references, conditional values, nesting, and recipe usage.
- Chakra UI, [Recipes](https://chakra-ui.com/docs/theming/recipes): base styles, variants,
  compound variants, defaults, and semantic-token use.
- Chakra UI, [Slot Recipes](https://chakra-ui.com/docs/theming/slot-recipes): multi-part component
  slots, per-slot variants, compound variants, and unstyled behavior.
- Chakra UI, [Conditions](https://chakra-ui.com/docs/theming/customization/conditions): named
  conditions for selectors and state.
- Panda CSS, [Tokens](https://panda-css.com/docs/theming/tokens): token and semantic-token format,
  condition values, token namespaces, CSS-variable output.
- Panda CSS, [Recipes](https://panda-css.com/docs/concepts/recipes): CVA-like base, variants,
  compound variants, defaults, type helpers, and pre-generation behavior.
- Panda CSS, [Slot Recipes](https://panda-css.com/docs/concepts/slot-recipes): slot recipes,
  atomic output, compound variants, slot targeting, and config registration.
- Panda CSS, [Conditions](https://panda-css.com/docs/customization/conditions): condition
  extension, selector placeholder, token use in conditions, mixed and multi-block conditions.
- Visual Studio Code, [Color Theme](https://code.visualstudio.com/api/extension-guides/color-theme):
  workbench colors, token color customizations, semantic colors, and theme creation flow.
- Visual Studio Code, [Theme Color](https://code.visualstudio.com/api/references/theme-color):
  workbench color IDs, user override setting, CSS variable availability, and color formats.
- Visual Studio Code, [Semantic Highlight Guide](https://code.visualstudio.com/api/language-extensions/semantic-highlight-guide):
  standard token types and modifiers, custom token subtypes, semantic highlighting enablement,
  selectors, and TextMate fallback.
- Visual Studio Code, [Themes user docs](https://code.visualstudio.com/docs/configure/themes):
  semantic token customizations, scope inspector, and theme generation from current settings.
- Tailwind CSS, [Theme variables](https://tailwindcss.com/docs/theme): `@theme`, variable
  namespaces, default extension/override/reset, static generation, and sharing themes as CSS.
- Mermaid, [Theme Configuration](https://mermaid.js.org/config/theming.html): named themes,
  site-wide and diagram-specific configuration, `themeVariables`, derived colors, and hex-color
  calculation constraint.
- Mermaid, [Config Schema](https://mermaid.js.org/config/schema-docs/config.html): `theme`,
  `themeVariables`, `themeCSS`, `look`, layout, and other configuration fields.
