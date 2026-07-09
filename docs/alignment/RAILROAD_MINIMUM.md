# Railroad Minimum (Mermaid@11.16.0)

This document tracks the first local support slice for Mermaid `railroad-beta`,
`railroad-ebnf-beta`, `railroad-abnf-beta`, and `railroad-peg-beta`.

Upstream references at pinned Mermaid 11.16.0:

- Detector: `packages/mermaid/src/diagrams/railroad/*Detector.ts`
- Parser adapters: `packages/mermaid/src/diagrams/railroad/parser.ts`
- DB/model types: `packages/mermaid/src/diagrams/railroad/railroadTypes.ts`
- Renderer: `packages/mermaid/src/diagrams/railroad/railroadRenderer.ts`
- Styles: `packages/mermaid/src/diagrams/railroad/styles.ts`

## Implemented

- Detection:
  - `railroad-beta` maps to `railroad`
  - `railroad-ebnf-beta` maps to `railroadEbnf`
  - `railroad-abnf-beta` maps to `railroadAbnf`
  - `railroad-peg-beta` maps to `railroadPeg`
- Parser:
  - shared AST for all four dialects
  - IR calls: `terminal`, `nonterminal`, `sequence`, `choice`, `optional`, `zeroOrMore`,
    `oneOrMore`, and `special`
  - EBNF choice, sequence, optional, repetition, exception-as-sequence, and special text
  - ABNF alternation, concatenation, repetition bounds, optional groups, comments, and numeric values
  - PEG ordered choice, sequence, prefix predicates, suffix operators, any-char, grouping, and comments
  - common `title`, `accTitle`, and `accDescr`
- LSP/editor facts:
  - header, common directives, rule symbols, terminal labels, nonterminal references, and special text
  - lossy fact scanning remains available when strict parsing fails
  - AST nodes retain source spans for editor-facing facts; render serialization omits span fields
- Render model:
  - typed `RailroadDiagramRenderModel`
  - all four upstream ids project to the shared `railroad` render model kind
- Layout:
  - source-backed recursive railroad dimensions for terminals, nonterminals, specials, sequences,
    choices, optional paths, and repetitions
  - `railroad.padding`, `verticalSeparation`, `horizontalSeparation`, `arcRadius`, `fontSize`,
    `fontFamily`, `markerRadius`, and `useMaxWidth`
  - deterministic text measurement through the existing headless `TextMeasurer`
- SVG:
  - root `railroad-diagram` class and `aria-roledescription="railroad"`
  - rule groups, rule names, start/end markers, terminal/nonterminal/special boxes, connector paths,
    and accessibility DOM for `accTitle` and `accDescr`
  - Railroad-specific style options and theme fallbacks from `styles.ts`

## Admission State

`railroad`, `railroadEbnf`, `railroadAbnf`, and `railroadPeg` are recorded as
`CompatibilityOnly` in the admission inventory:

- semantic JSON fixtures are normalized under `fixtures/railroad*/`
- layout goldens are normalized under `fixtures/railroad*/`
- local SVG rendering is implemented
- upstream SVG baselines and family-local compare commands are deferred to the U7 Mermaid 11.16
  baseline refresh

## Known Gaps

- No committed Railroad upstream SVG corpus yet.
- No dedicated `xtask compare-railroad-svgs` command yet.
- Browser `getBBox()` text dimensions are approximated through the repository's deterministic text
  measurement path.
- The upstream renderer currently ignores `compactMode`, `showMarkers`, repetition separators, and
  repetition maximums during drawing; the local compatibility renderer follows that behavior rather
  than inventing extra semantics.
