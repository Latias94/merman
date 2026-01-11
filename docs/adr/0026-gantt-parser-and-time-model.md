# ADR 0026: Gantt Parser & Time Model (Mermaid@11.12.2 parity)

- Status: Accepted
- Date: 2026-01-11
- Baseline: Mermaid `@11.12.2`

## Context

Mermaid’s gantt implementation is split between:

- A Jison grammar (`packages/mermaid/src/diagrams/gantt/parser/gantt.jison`) that parses statements and task rows.
- A stateful DB (`packages/mermaid/src/diagrams/gantt/ganttDb.js`) that:
  - stores raw tasks + cross references (`taskDb`)
  - compiles tasks iteratively to resolve forward references (`after <id...>`, `until <id...>`)
  - performs date arithmetic including `inclusiveEndDates` and `excludes` adjustments.

`merman` is a headless 1:1 re-implementation. Upstream behavior is the specification.

## Decision

### Parser technology

Implement gantt parsing in `merman-core` as a hand-written, line-oriented parser that mirrors the
Jison grammar’s observable behavior:

- Accept `gantt` header with optional indentation.
- Parse statement lines (`dateFormat`, `title`, `section`, `inclusiveEndDates`, `topAxis`, `weekday`, `weekend`, `includes`, `excludes`, `todayMarker`, accessibility fields, `click ...`).
- Parse task lines `<taskTxt>:<taskData>` with Mermaid-compatible tag extraction (`active`, `done`, `crit`, `milestone`, `vert`).
- Preserve Mermaid quirks such as allowing task IDs like `__proto__` and `constructor` (safe in Rust).

Rationale: gantt is primarily line-driven with stateful DB semantics and iterative compilation; a
grammar-first AST pipeline does not reduce complexity for parity and makes error alignment harder.

### Time model

Store task times internally as “JS Date” milliseconds since UNIX epoch (i64), computed using:

- `dateFormat == "x" | "X"`: numeric strings are interpreted as milliseconds (`new Date(Number(str))` parity).
- Other `dateFormat`: parse into a local datetime, then convert to epoch milliseconds (parity with
  `dayjs(...).toDate()` and JavaScript Date’s internal milliseconds).

This keeps the semantics comparable to Mermaid (ordering, comparisons, relative references) while
remaining serializable for headless output.

## Consequences

- `merman-core` will include a gantt-specific DB that mirrors Mermaid’s raw task compilation loop:
  compile until all tasks are processed or a max iteration count is reached.
- Date parsing will be implemented incrementally, guided by upstream test vectors (starting with
  `YYYY-MM-DD`, `YYYYMMDD`, `ss`, and timestamp formats used by Mermaid’s own tests).
- Output JSON will include epoch millisecond fields (`startTime`, `endTime`, `renderEndTime`) plus
  a `raw` block mirroring Mermaid’s `raw.startTime/raw.endTime` compilation inputs.

## Revisit criteria

Reconsider the implementation if Mermaid moves gantt parsing to `@mermaid-js/parser` (like
architecture), or if gantt syntax becomes significantly more expression-based than line-oriented.
