# Quadrant Chart Minimum Slice (Phase 1)

This document defines the initial, test-driven minimum slice for Quadrant Chart parsing in `merman`.

Baseline: Mermaid `@11.12.2`.

Upstream references:

- Grammar: `repo-ref/mermaid/packages/mermaid/src/diagrams/quadrant-chart/parser/quadrant.jison`
- DB behavior: `repo-ref/mermaid/packages/mermaid/src/diagrams/quadrant-chart/quadrantDb.ts`
- Style validation: `repo-ref/mermaid/packages/mermaid/src/diagrams/quadrant-chart/utils.ts`
- Tests:
  - `repo-ref/mermaid/packages/mermaid/src/diagrams/quadrant-chart/parser/quadrant.jison.spec.ts`
  - `repo-ref/mermaid/packages/mermaid/src/diagrams/quadrant-chart/quadrantDb.spec.ts`

## Supported (current)

- Header:
  - `quadrantChart` (case-insensitive).
  - Allows leading empty lines and comment lines.
- Comments:
  - Lines starting with `%%` are ignored.
  - Inline `%% ...` is ignored outside quoted strings.
- Title:
  - `title <free text>` (stored under `title`, sanitized by the common DB pass).
- Accessibility:
  - `accTitle: ...`
  - `accDescr: ...`
  - `accDescr { ... }` multi-line block (terminated by the first `}`).
- Axis labels:
  - `x-axis <left> --> <right>`
  - `y-axis <bottom> --> <top>`
  - If the right/top side is omitted after the delimiter, the left/bottom text is appended with ` ? `
    before trimming/sanitization (matches Jison rule behavior).
- Quadrant labels:
  - `quadrant-1 <text>`
  - `quadrant-2 <text>`
  - `quadrant-3 <text>`
  - `quadrant-4 <text>`
- Points:
  - `<text>: [x, y]` where `x` and `y` are constrained to Mermaid’s token rules:
    - `1` or `0` or `0.<digits>` (values like `1.2` are rejected).
  - Optional class name:
    - `<text>:::<className>: [x, y]`
  - Optional inline styles (comma-separated):
    - `radius: <number>`
    - `color: <hex>`
    - `stroke-color: <hex>`
    - `stroke-width: <n>px`
- Classes:
  - `classDef <className> <styles...>` using the same style keys/validation as point styles.

## DB-level behavior (Phase 1)

- Quadrant/axis/point text is sanitized at DB-time using Mermaid’s common sanitizer.
- Points are stored in reverse insertion order (prepended) to match Mermaid’s `addPoints()` behavior.
- Style validation and error messages mirror Mermaid’s `InvalidStyleError` and unsupported-style errors.

## Output shape (Phase 1)

- Headless output snapshot:
  - `type`
  - `title`, `accTitle`, `accDescr`
  - `quadrants`: `{ quadrant1Text, quadrant2Text, quadrant3Text, quadrant4Text }`
  - `axes`: `{ xAxisLeftText, xAxisRightText, yAxisBottomText, yAxisTopText }`
  - `points`: `{ text, x, y, className, styles }[]`
  - `classes`: `{ [className]: styles }`
  - `config`

## Alignment goal

This is an incremental slice. The ultimate goal is full Mermaid `quadrantChart` parsing and DB behavior
compatibility at the pinned baseline tag.

