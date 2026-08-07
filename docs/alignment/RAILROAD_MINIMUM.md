# Railroad Minimum (Mermaid@11.16.1)

This document tracks the first local support slice for Mermaid `railroad-beta`,
`railroad-ebnf-beta`, `railroad-abnf-beta`, and `railroad-peg-beta`.

Upstream references at pinned Mermaid 11.16.1:

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
  - ABNF alternation, concatenation, Mermaid-compatible binary64 repetition bounds (including
    rounding and positive infinity), optional groups, comments, and numeric values
  - PEG ordered choice, sequence, prefix predicates, suffix operators, any-char, grouping, and comments
  - common `title`, `accTitle`, and `accDescr`
  - semantic JSON encodes finite repetition bounds as numbers and infinity as `null`
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

Normal parity is green for all four families. The current 11.16 corpus has eight root-only width
residuals: four Railroad, two EBNF, one ABNF, and one PEG. Normalized descendants and root heights
match in every case. The signed width differences range from `-0.016px` to `+0.047px`, matching the
Chromium text-bbox lattice used by upstream `measureText()`; the deterministic vendored profile is
within three `1/64px` steps.

The strict global `parity-root` sweep accepts only the exact family, fixture, descendant profile,
artifact hashes, and root attributes recorded by `RootParityResidualPolicy`. A changed value, a new
fixture, or any descendant DOM difference remains a failure. See
`docs/alignment/ROOT_PARITY_RESIDUAL_CATALOG.md` for the current counts and source audit. Do not
close this browser-bounded residual with character-count width floors, fixture-specific viewport
pins, or other viewport magic; revisit it only with a general browser measurement model.

## Known Gaps

- Browser `getBBox()` text dimensions are approximated through the repository's deterministic text
  measurement path.
- The upstream renderer currently ignores `compactMode`, `showMarkers`, repetition separators, and
  repetition maximums during drawing; the local compatibility renderer follows that behavior rather
  than inventing extra semantics.
