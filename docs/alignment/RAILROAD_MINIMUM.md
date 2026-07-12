# Railroad Minimum (Mermaid@11.16.0)

This document tracks the first local support slice for Mermaid `railroad-beta`,
`railroad-ebnf-beta`, `railroad-abnf-beta`, and `railroad-peg-beta`.

Upstream references at pinned Mermaid 11.16.0:

- Detector: `packages/mermaid/src/diagrams/railroad/*Detector.ts`
- Parser adapters: `packages/mermaid/src/diagrams/railroad/parser/*.ts`
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
  - root `railroad-diagram` class and dialect-specific `aria-roledescription`
  - source-backed recursive `railroad-sequence`, `railroad-choice`, `railroad-optional`, and
    `railroad-repetition` groups, including upstream child/path DOM order
  - rule groups, rule names, start/end markers, terminal/nonterminal/special boxes, connector paths,
    and accessibility DOM for `accTitle` and `accDescr`
  - Railroad-specific style options and theme fallbacks from `styles.ts`

## Admission State

`railroad`, `railroadEbnf`, `railroadAbnf`, and `railroadPeg` are admitted to the primary
SVG parity matrix:

- semantic JSON fixtures are normalized under `fixtures/railroad*/`
- layout goldens are normalized under `fixtures/railroad*/`
- Mermaid 11.16 SVG baselines include per-file input/SVG hashes and pinned renderer provenance
- `compare-railroad-svgs`, `compare-railroad-ebnf-svgs`, `compare-railroad-abnf-svgs`, and
  `compare-railroad-peg-svgs` pass fresh structural DOM parity for their normalized fixtures

## Root Viewport Residuals

Structural parity is green for all four families. Each family has one root-only `viewBox` height
residual because upstream Chromium derives the SVG height from browser font `getBBox().height`,
while the headless renderer uses deterministic text metrics:

| Family | Fixture | Upstream height | Local height |
|---|---|---:|---:|
| `railroad` | `basic_ir` | 194.5 | 192.25 |
| `railroadEbnf` | `choice_optional_repetition` | 174 | 171.25 |
| `railroadAbnf` | `repetition_optional_numval` | 221 | 216.75 |
| `railroadPeg` | `prefix_suffix_any` | 107 | 105.5 |

The strict global `parity-root` sweep accepts only these exact family, fixture, descendant-match,
and `viewBox` fragments through `RootParityResidualPolicy`. A changed value, a new fixture, or any
descendant DOM difference remains a failure. Do not close this browser-bounded residual with
character-count width floors, fixture-specific viewport pins, or other viewport magic; revisit it
only with a source-backed browser measurement model.

## Known Gaps

- Browser `getBBox()` text dimensions are approximated through the repository's deterministic text
  measurement path.
- The upstream renderer currently ignores `compactMode`, `showMarkers`, repetition separators, and
  repetition maximums during drawing; the local compatibility renderer follows that behavior rather
  than inventing extra semantics.
