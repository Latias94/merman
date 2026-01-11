# ADR 0027: Timeline Parser Technology (Mermaid@11.12.2 parity)

- Status: Accepted
- Date: 2026-01-11
- Baseline: Mermaid `@11.12.2`

## Context

Mermaid’s timeline implementation is driven by:

- A Jison grammar (`packages/mermaid/src/diagrams/timeline/parser/timeline.jison`) that tokenizes:
  - `title` / `section`
  - task “period” lines
  - event lines introduced by `:\s`, where colons without trailing whitespace are part of the event
    (notably for `http://...` links).
- A small DB (`packages/mermaid/src/diagrams/timeline/timelineDb.js`) that stores:
  - `sections[]`
  - `rawTasks[]` with an incrementing `id`
  - `events[]` attached to the most recently added task.

`merman` must be headless and 1:1 compatible: upstream behavior is the specification.

## Decision

Implement timeline parsing in `merman-core` as a hand-written, line-oriented parser that mirrors
the Jison grammar’s observable behavior:

- Require `timeline` header.
- Parse `title` and `section` as “rest-of-line” statements (preserving `#` and `;`).
- Parse tasks as “period” statements and events as repeated `:\s...` segments.
- Split events only on `:` followed by whitespace (so `http://` stays inside an event).
- Store output as a DB-like semantic snapshot, deferring HTML sanitization to the shared
  `commonDb`-aligned post-processing step (see ADR-0020).

Rationale: timeline is primarily line-driven, and the event splitting behavior is lexer-oriented.
A dedicated hand-written parser is simpler than introducing a general-purpose lexer/LALR pipeline
for a diagram whose semantics are primarily “last task + appended events”.

## Consequences

- The timeline parser is easy to keep aligned using Mermaid’s own test vectors
  (`packages/mermaid/src/diagrams/timeline/timeline.spec.js`) ported into Rust tests.
- Event splitting semantics become a small, diagram-local utility, reused only within timeline.

## Revisit criteria

Reconsider the implementation if Mermaid migrates timeline parsing to `@mermaid-js/parser`, or if
timeline gains significantly more structured syntax that benefits from a shared parser framework.

