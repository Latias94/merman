---
title: Frontend evidence for general theme authoring
date: 2026-10-19
status: research
---

# Frontend evidence for general theme authoring

## Scope

Primary-source research for a general Merman authoring interface that can retain shared-value
bindings, expose theme-specific choices through common controls, and describe visual effects.
All linked sources were accessed on 2026-10-19. This note supplies evidence and evaluation
questions; it does not choose a Merman document schema or assert renderer capabilities.

## shadcn: semantic consumption and composable starting configurations

shadcn components consume semantic CSS variables. Surface and content colors are paired, such
as `primary` and `primary-foreground`; changing a variable changes its consumers without editing
component classes. New token pairs can be added. A base radius supplies a derived scale, and
light/dark appearances override the same names. These mechanisms separate the role a component
consumes from the literal value chosen by a theme.
[Source: theming and token convention](https://ui.shadcn.com/docs/theming#token-convention).

Its preset CLI can decode a shareable configuration and reopen it for customization. The example
configuration exposes independent fields including style, base color, theme color, chart color,
fonts, radius, and menu choices. This is useful evidence for a preset being a combination of
shared dimensions rather than a separate settings application. The page does not establish
lossless encoding of arbitrary hand-authored CSS or token graphs.
[Source: preset commands](https://ui.shadcn.com/docs/changelog/2026-04-preset-commands).

**Implication:** borrow explicit role consumption and reusable starting data. Do not copy a fixed
inventory of web-application roles into diagram semantics, or assume a small preset code can
represent every authored theme. A common editor must be able to show extra values without
knowing the preset's name.

## Chakra: reusable values, semantic aliases, and multi-part recipes

Chakra groups design decisions into token categories and wraps each value in an object that can
also hold a description. Its token syntax supports references inside composite CSS property
values, including shadows. Generated typings provide token-name completion.
[Source: tokens](https://chakra-ui.com/docs/theming/tokens).

Semantic tokens can hold literals or aliases to other tokens, including conditional light/dark
values. Recipes can consume these semantic names. This creates a reusable intermediate level
between a palette and a styled object.
[Source: semantic tokens](https://chakra-ui.com/docs/theming/semantic-tokens).

Recipes describe base styles, named variant axes, defaults, and compound combinations. Consumers
choose variants through a typed API. This supports appearance families with the same machinery,
but still requires an explicit finite set of variant choices.
[Source: recipes](https://chakra-ui.com/docs/theming/recipes).

Slot recipes extend that model to named parts: one variant can adjust several slots together.
The checkbox example declares `root`, `control`, and `label`, with per-slot base and variant
styles. The recipe can live in the shared theme. This is a concrete model for changing a
semantic object and its associated label together while retaining separately editable parts.
[Source: slot recipes](https://chakra-ui.com/docs/theming/slot-recipes).

Chakra also offers generated theme types and an optional `strictTokens` mode. Its system can
return a raw token value separately from a CSS-variable representation. Its condition system,
however, accepts selectors and media/container queries.
[Source: system overview](https://chakra-ui.com/docs/theming/overview).

**Implication:** named, typed resources and stable semantic parts can support a generic inspector.
Presets can define different collections of resources and recipe values without adding
preset-specific frontend components. Optional variant axes are useful only where the diagram
model supplies a meaningful selection input. Arbitrary variant evaluation, browser selectors,
and responsive conditions would add an execution model that a headless diagram renderer does
not automatically possess. CSS string interpolation is convenient in Chakra; a portable Merman
document can instead use structured references at typed property boundaries.

## MUI: common defaults and explicit local customization

MUI's theme separates palette, typography, spacing, and other shared values from component
configuration. Applications can add custom theme variables; TypeScript consumers augment the
theme interfaces to describe them. Generated CSS variables are a separate output. The docs
warn that user-supplied theme values must be validated before being written to stylesheets.
[Source: theming](https://mui.com/material-ui/customization/theming/).

The `components` section provides default props and slot-based style overrides. Conditional
variants can alter an existing prop value or add a new visual variant. Variant definitions are
ordered so later entries can take precedence. Instance-level `sx` can override theme styling.
[Source: themed components](https://mui.com/material-ui/customization/theme-components/).

For extensible branded themes, MUI recommends object spreading for tokens and arrays for
component-style composition, preserving previous variants and states. It discourages generic
deep-merge utilities in this path because of first-render overhead.
[Source: building extensible themes](https://mui.com/material-ui/guides/building-extensible-themes/).

**Implication:** distinguish shared design choices, semantic defaults, and scoped exceptions.
Composition rules should be explicit for each collection. A generic recursive merge cannot
decide whether an effect stack should append, replace, clear, or inherit. MUI's JavaScript
callbacks, arbitrary CSS selectors, specificity, and runtime module augmentation are not a
serialization contract for untrusted JSON. Their convenience does not justify implementing a
CSS interpreter or executable theme functions in Merman.

## DTCG: typed authoring data and retained relationships

The DTCG Format Module 2025.10 is a Community Group specification, not a W3C Standard. Its useful
mechanisms are explicit token types, aliases, and composite values. It recommends preserving
references until an actual value is needed and reports unresolved paths, cycles, and type
mismatches. Groups alone do not determine token meaning. Shadow values contain color, offsets,
blur, spread, and optional inset; arrays can describe multiple shadows. Composite sub-values
can reference other appropriately typed tokens. Extensions are namespaced, must survive tools
that do not understand them, and should remain optional metadata rather than essential value
semantics.
[Sources: format status](https://www.designtokens.org/tr/2025.10/format/#status-of-this-document),
[references](https://www.designtokens.org/tr/2025.10/format/#reference-resolution),
[shadow](https://www.designtokens.org/tr/2025.10/format/#shadow),
[extensions](https://www.designtokens.org/tr/2025.10/format/#extensions).

**Implication:** retaining a binding is different from retaining its currently resolved value.
An editor needs both a shared-resource operation and an explicit detach operation. Borrowing
these concepts does not establish full DTCG compatibility: conformance would require the
complete applicable reference, type, extension, and processing rules. Essential diagram effects
belong in the supported public model, not opaque editor metadata.

## Shadows require renderer semantics, not CSS-shaped names alone

CSS box shadows use the box silhouette, support inset/spread, and layer the first listed shadow
above later shadows. They do not influence layout. CSS blur-radius rendering approximates a
Gaussian blur whose standard deviation is half that radius.
[Source: CSS Backgrounds and Borders, shadow blur and layering](https://www.w3.org/TR/css-backgrounds-3/#shadow-blur).

SVG `feDropShadow` instead blurs the input alpha, offsets it, colors it, and composites it below
the input. Its `stdDeviation` is the Gaussian standard deviation. The CSS `drop-shadow()`
function likewise uses a standard deviation, and its Level 1 grammar does not accept spread or
multiple shadows within one function. Therefore assigning the same numeric `blur` field to
CSS box-shadow and SVG drop-shadow does not imply matching appearance.
[Sources: SVG drop-shadow primitive](https://www.w3.org/TR/filter-effects-1/#feDropShadowElement),
[CSS drop-shadow function](https://www.w3.org/TR/filter-effects-1/#funcdef-filter-drop-shadow).

**Implication:** evaluate hard offset shadows, soft shadows, centered glows, and layered shadows
as distinct parameter combinations. A common editor can expose typed color, offset, softness,
opacity, and ordering controls, but the public contract must specify which silhouette is
shadowed, the blur convention, the composition order, and the supported units. Inset and spread
need actual backend support or explicit rejection. Filter clipping, effect bounds, text/label
inclusion, export bleed, and repeated effects must be checked against Merman's own renderer;
these frontend references cannot answer those implementation questions.

## Questions to settle before choosing the Merman interface

These are design deductions from the mechanisms above, not claims that the referenced systems
already solve Merman's domain:

1. Can the same document represent a plain theme and a theme with extra gradients, custom
   effect stacks, and canvas resources, with no preset-name condition in the editor?
2. Are a shared value and its consumers stored separately, so export/import preserves later
   edits to the shared value? Can a user detach one consumer without changing the others?
3. Can a semantic default be refined for a family or object part without duplicating unrelated
   values? Is precedence visible and deterministic?
4. Are collections given explicit replacement, composition, and clearing semantics? Do
   whole-resource references and property-level references interact predictably?
5. Is a theme-specific setting an instance of a known renderer-supported type, or does it
   introduce a genuinely new rendering operation? Typed arbitrary names solve the former;
   extension metadata cannot make the latter render.
6. Can the headless runtime resolve and validate the document once, then hand concrete values
   to existing rendering stages? Does authoring preserve enough data for future editing?
7. Can a generic inspector derive basic controls from types and optional presentation metadata
   while keeping that metadata independent of rendering meaning?

## Limits and uncertainties

- This research does not infer bindings from equal literal values or identify which existing
  Merman presets retain an editable source; those require repository evidence.
- These systems demonstrate separate pieces of a viable model. None of the cited pages
  specifies an entire headless diagram authoring protocol or proves a Merman implementation.
- A finite supported type set keeps validation and controls tractable. Adding a new type still
  requires deliberate renderer, validation, serialization, and editor support.
- CSS variables and JavaScript themes can keep relationships live through a browser or runtime.
  Portable JSON needs an explicit data model for those relationships and a defined resolver.
- No framework adoption or migration cost is evaluated here. The evidence supports borrowing
  mechanisms; it does not require adding Chakra, MUI, Tailwind, or DTCG dependencies.
