# Theme discovery and reference designs

Date: 2026-10-29. Status: implemented and locally verified.

The subsequent [product readiness plan](2026-10-29-theme-product-plan.md) supersedes this
plan's immediate-setting/Cancel behavior and initial release-check evidence. This document
records the earlier implementation; the current guide defines the revised user contract.

## Goal

Prepare the main branch's theme experience for a bounded `v0.10.0-alpha.1`.
A user must be able to choose a Mermaid base theme or Merman styling, understand
the chosen preset's design scope, preview it, and act on any required settings
without reading execution JSON. Ship the two reviewed reference designs as
ordinary, discoverable presets and reproducible examples.

This work does not publish a release, promise all 24 reference designs, or resume
the paused general performance investigation. Maintenance `0.9.x` is outside this
feature's scope.

## Evidence and design decisions

The audit of main `1c4cd0da9` exercised the public browser engine and native
SVG/PNG/PDF for Brutalist Flowchart and a Ghibli-style Pie. Existing authoring,
canvas, palettes, import and draft lifecycles are sufficient. The missing pieces
are discoverability, an installed Ghibli recipe, and typed Pie legend/size
consumers. A broad authoring rewrite would not solve those concrete gaps.

- Keep the Rust preset catalog as the single authority for recipe colors,
  curated family designs and application notes. SDKs project that metadata;
  Playground must not maintain a second support table or preset-ID conditions.
- Add defaulted `preview_colors` and `application_notes` to the unreleased
  version-one discovery contract. A note has an open `id`, `family_id`, readable
  `message`, and optional `recommended_config` object. Suggestions never mutate
  a diagram merely because a preset was selected.
- Preserve four different facts: recipe availability, curated design scope,
  mechanism support and this execution's output admission. Missing family
  designs mean unreviewed. Empty qualification cells grant no output promise.
- Keep Mermaid settings and Merman styling visibly separate. Mermaid settings
  apply immediately; Merman selections use the established draft/Apply/Cancel
  lifecycle. Explain where these choices affect the comparison engine.
- Show a selected preset's reviewed families and current-family treatment near
  selection. Provide an explicit action for recommended configuration, preserving
  unrelated configuration and reporting invalid JSON rather than overwriting it.
- Explain the current render outcome outside Advanced. Never associate retained
  preview evidence with a newly selected candidate or pending render. Keep exact
  raw metadata available for diagnosis.
- Make Ghibli a native editable recipe with dedicated Pie design scope: warm
  paper, four tiled gradient texture layers, five muted slice colors, and typed
  title/percentage/legend styling. Do not claim unreviewed family designs.
- Give Pie text properties real measurement, painting and terminal evidence
  owners. Preserve source/config precedence and unchanged no-theme Mermaid
  output. Unsupported selectors/facets must retain an honest residual.
- Brutalist hard shadows still require Classic and supported basic shapes.
  Offer Classic explicitly; preserve look/layout ownership and diagnostics.
- Remove the catalog's hard-coded test-only recipe/resource hashes, revision
  copies and inert qualification-invalidation descriptor. They only compared
  duplicated constants against the compiler and did not qualify rendering.
  Keep runtime-computed identities where actual compatibility/session and
  qualification consumers reject mismatched content. Ordinary theme use exposes
  neither mechanism to callers.

## Implementation ownership

1. Main agent: core catalog contract, Ghibli recipe, shared fixtures, examples,
   CLI discovery, developer documentation, integration and validation.
2. Pie worker: Pie prepared typography/layout/painting/evidence, semantic target
   and mechanism routing, focused Rust regressions.
3. Playground worker: theme panel/cards, pure presentation/configuration helpers,
   translations and focused user interaction contracts.
4. SDK worker: public metadata types/normalization, platform projections and
   consumer contracts, Web usage documentation.

Workers edit disjoint surfaces in the shared feature worktree. Cargo commands are
run sequentially by the main agent against the existing shared target directory.

## Acceptance scenarios

- Choose a Mermaid base theme with Merman styling disabled; the UI explains its
  immediate application and does not imply that Merman presets style Mermaid.js.
- On Flowchart, select Brutalist: the draft is visible, the family is dedicated,
  the hard-shadow condition is explained, and configuration changes require an
  explicit action. Apply commits the styling; Cancel restores the prior styling.
- Apply recommended settings with unrelated configuration present; unrelated
  values survive. Invalid authored JSON produces an actionable error.
- On Pie, select Ghibli: colors, texture, title, percentages and legend actually
  render. Typed theme evidence accounts for the requested supported properties.
- On an unreviewed family, the panel states unreviewed without claiming either
  universal support or an unsupported renderer.
- Rapid preset/source/config changes cannot show a previous render's status as
  the current selection's outcome. Advanced retains exact same-render metadata.
- Export a preset recipe, import it in a clean session and reproduce the result
  with the documented source/options. Separate configuration stays explicit.
- A CLI/SDK consumer can discover preset names, design scope and application
  notes without scraping UI text. Unknown metadata IDs survive normalization.

## Validation owners

- Core catalog/contract: focused `cargo nextest` in `merman-theme-contract`,
  `merman-render`, `merman-theme-authoring-fixtures`, bindings metadata and CLI
  contracts. Regenerate shared fixture data from its existing Rust owner.
- Renderer: focused Pie typography/ownership/evidence tests, then the full Linux
  structure policy command from `.agents/skills/contribute-merman-pr/SKILL.md`.
  Unchanged no-theme baselines must pass without comparator changes.
- Web: source normalization/consumer tests and fresh full WASM input identity.
- Playground: runtime/store/helper tests, TypeScript, lint, prepared build and
  focused Chromium theme interactions. Pages owns the wider browser matrix.
- Visual review: render the two public examples through fresh Web and native
  SVG/PNG/PDF; inspect images and report actual evidence/admission independently.
- Every commit: precise staging, `git diff --check`, path-length gate and Rust
  formatting where applicable. No dependency upgrade or release publication.

## Completion record

The Rust catalog owns eleven alpha recipes, representative colors and optional
family application notes. Web and Flutter project the additive metadata; CLI
prints readable design scopes and suggestions. Playground offers direct preset
selection with current-family treatment, optional swatch browsing, explicit
configuration merging and a same-render outcome summary. Reviewed families,
recipe validity, theme application and target admission remain distinct.

Pie now prepares its title, percentage and legend properties once for measurement,
CSS emission and terminal accounting. Ghibli uses the ordinary typed recipe path
with 25/17/12px roles, a muted five-color palette and four paper-texture layers.
Entity-decoded whitespace and XML-filtered empty labels cannot claim visible
application. The catalog's test-only hash/revision/invalidation copies are gone.

Remote main through `dc3ca0f22` was merged in `a985a4b9f`. The removed upstream
`svgPlan` field stays removed. Automatic merges in README, Flutter contracts and
Playground scripts retain both intents. Local owner checks include:

- 3,363 renderer unit/Pie/support tests after the merge; one intentional skip.
- 70 binding theme contracts, 62 theme-wire contracts and 22 CLI theme contracts.
- 29 Web discovery/authoring/package tests; Dart contracts and analysis.
- 170 Playground runtime tests, TypeScript, ESLint and production artifact checks.
- Desktop selection/import/Apply/Cancel, invalid-source recovery, late file reads,
  current-input evidence, Git Graph/Quadrant Chart names and keyboard-focus cases;
  mobile 320px, mid-width and shortened viewport cases.
- Fresh full browser WASM inputs verified as `c43e3f7b8611`; real Chromium renders
  confirm Classic Brutalist and Ghibli theme application. Exported Ghibli JSON
  reproduces the preset's SVG bytes for identical inputs.
- Full SVG comparison with `--dom-modes structure,parity,parity-root
  --dom-decimals 3 --diagnostic-browser-text-layout --report-root` passed without
  comparator or residual-policy changes. Linux CI owns the full painted-content
  containment browser lane.
- Native SVG/PNG/PDF exports for both gallery examples completed. PNG and PDF
  previews were visually reviewed; checked-in PNGs sit beside reproducible
  sources/configuration in `docs/rendering/theme-gallery/`.

These observations do not populate public qualified cells or remove browser/font
dependencies. Default Neo still reports unapplied Brutalist effects; the UI offers
Classic explicitly. Other reference designs and unreviewed family aesthetics are
outside this change. No alpha version, release tag or package was published.
