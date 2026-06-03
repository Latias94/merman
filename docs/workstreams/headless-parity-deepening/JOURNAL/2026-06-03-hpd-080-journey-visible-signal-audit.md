# HPD-080 Journey And Timeline Visible Signal Audit

Date: 2026-06-03

## Question

Do the Journey and Timeline dark-theme renderability smoke cases count only colors that current SVG
output actually uses, or do they also count Mermaid stylesheet tokens that have no matching DOM in
the compact public-smoke sources?

## Source Check

Pinned Mermaid 11.15 `user-journey/styles.js` emits both Journey-specific rules and several
Flowchart-like inherited rules:

- visible on current Journey DOM: generic `line`, `.legend`, `.label`, `.face`, `.task-type-*`,
  `.section-type-*`, and `.actor-*`;
- inert on current Journey DOM: `.edgePath .path`, `.flowchart-link`, `.edgeLabel`, `.cluster text`,
  `.node ...`, and `.arrowheadPath`.

Pinned `journeyRenderer.ts` appends the final activity line with `stroke="black"` and no
`flowchart-link` / `edgePath` / `edgeLabel` class. Pinned `svgDraw.js` creates the marker path
without an `.arrowheadPath` class.

This means `themeVariables.lineColor`, `edgeLabelBackground`, `mainBkg`, `nodeBorder`,
`titleColor` through `.cluster text`, and `arrowheadColor` can appear in the Journey stylesheet
without being visible in the current SVG DOM.

Pinned Mermaid 11.15 `timeline/styles.js` also emits `.disabled` rules, but ordinary Timeline
sources emit `timeline-node section-*`, `node-bkg`, `node-line-*`, and `lineWrapper` elements. The
compact public smoke source has no `class="disabled"` element, so `tertiaryColor` and
`clusterBorder` in that case were proving provider emission, not visible renderability.

## Decision

Tighten the public Journey dark-theme smoke to count only visible current-output surfaces:

- `textColor` through generic `line`, legend, and label rules;
- `faceColor` through `.face`;
- `fillType0` through task/section classes;
- `actor0` through actor classes.

Tighten the public Timeline dark-theme smoke to count visible section surfaces instead:

- `cScale0` through the first visible section background;
- `cScaleLabel0` through first visible section text and lineWrapper color;
- `cScaleInv0` through first visible section line rules.

Keep focused public render tests for the source-backed boundaries. The Journey test asserts that:

- Journey emits `line { stroke: textColor }`;
- the activity line still carries Mermaid 11.15's black presentation attribute;
- the inherited `.flowchart-link` and `.edgeLabel` CSS rules are emitted but no matching DOM class
  exists;
- `.arrowheadPath` remains inert because the marker path has no matching class.

The Timeline test asserts that visible `section--1` CSS consumes the configured `cScale0`,
`cScaleLabel0`, and `cScaleInv0` values, while `.disabled` CSS is emitted without any matching
disabled DOM in the compact source.

## Verification

- `cargo nextest run -p merman --features render --test theme_renderability_smoke journey_theme_smoke_does_not_count_inert_flowchart_rules_as_visible timeline_theme_smoke_counts_section_dom_not_disabled_css_as_visible representative_dark_theme_diagrams_keep_visible_theme_signals`

## Follow-Up

Continue HPD-080 scans with this rule: a CSS token is a renderability signal only when the current
renderer emits a matching element/attribute/class or the token is consumed by inline renderer
configuration. Pure stylesheet presence is source evidence, not visible coverage.
