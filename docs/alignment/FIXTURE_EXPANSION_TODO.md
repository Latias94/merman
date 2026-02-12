# Fixture Expansion TODO (Mermaid@11.12.2)

This document tracks fixture expansion work that is not yet imported into `fixtures/**`.

Policy:

- Upstream baseline is Mermaid `@11.12.2` (see `repo-ref/REPOS.lock.json`).
- Prefer small, reviewable batches.
- Every imported fixture must remain traceable to an upstream source file.
- After each batch, keep the global parity gates green:
  - `cargo nextest run`
  - `cargo run --release -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3`

## High priority (diagram parity risk)

These diagrams are layout-/measurement-sensitive and historically most likely to regress:

1. Flowchart
2. State
3. Sequence
4. Architecture
5. Class
6. Mindmap

TODO:

- Import additional upstream syntax docs examples (10–30 fixtures per batch) from:
  - `repo-ref/mermaid/docs/syntax/*.md`
- Import missing upstream unit/integration tests that are not yet covered by `docs/alignment/*_UPSTREAM_TEST_COVERAGE.md`.
- Add “stress” fixtures:
  - dense graphs, long labels, many clusters, deep nesting, mixed HTML/SVG labels.

Recently imported (keep gates green after each batch):

- 2026-02-11: Mindmap `mindmap.md` single-node shape snippets (square/rounded/circle/bang/cloud/hexagon/default).
- 2026-02-11: Architecture `architecture.md` docs examples (`example_002`, `icons_018`).
- 2026-02-11: Flowchart `flowchart.md` docs examples batch import (20 fixtures, including new shapes, image nodes, animations, and curve-style variants).
- 2026-02-11: Flowchart additional docs batch (16 fixtures from `docs/syntax/flowchart.md` and a small set from
  `packages/mermaid/src/docs/*` covering directives/theming/contributing examples).
- 2026-02-12: Flowchart directives docs (`directives.md`) legacy `graph TD` directive examples (theme `forest`, `flowchart.curve=linear`).
- 2026-02-12: Flowchart additional shape fixtures from `docs/syntax/flowchart.md` (hexagon/parallelogram/trapezoids/double-circle + process/event/terminal/subprocess).
- 2026-02-12: Mermaid config docs batch import from `packages/mermaid/src/docs/config/*.md` (accessibility + directives + theming + tidy-tree), keeping the global `parity-root` gate green.
- 2026-02-11: State `stateDiagram.md` docs examples batch import (11 new fixtures; additional blocks were skipped as duplicates).
- 2026-02-11: Sequence `sequenceDiagram.md` docs examples batch import (16 new fixtures; additional blocks were skipped as duplicates).
- 2026-02-11: Class `classDiagram.md` docs examples batch import (13 new fixtures, including `hideEmptyMembersBox` and inline style variants).
- 2026-02-11: GitGraph `gitgraph.md` docs examples batch import (5 new fixtures: branch/line hiding and theme variants).
- 2026-02-11: External fixtures from `mermaid-rs-renderer` (Mindmap + Kanban: 2 fixtures).

## Medium priority (coverage growth)

TODO:

- Expand fixtures for:
  - `gantt`, `gitgraph`, `timeline`, `kanban`, `journey`, `packet`
  - `block`, `c4`, `requirement`, `radar`, `treemap`, `xychart`, `quadrantchart`, `sankey`

Focus areas:

- edge-case parsing (escaping, unicode/punctuation)
- config variants that change layout/labels
- error-handling surfaces (without weakening Mermaid parity contract)

Deferred (tracked for future import / parity work):

- Flowchart frontmatter config `layout: elk` (requires ELK layout parity support; current headless layout is dagre-ish).
- Sequence config directive examples that require `sequence.wrap=true` and `sequence.width` layout parity.
- Sequence math rendering (`$$...$$`) parity (upstream uses browser math rendering and `<foreignObject>` output).

## Special case: ZenUML (external diagram)

Upstream source:

- Mermaid docs: `repo-ref/mermaid/docs/syntax/zenuml.md`
- ZenUML implementation reference: `repo-ref/zenuml-core`

Current status:

- Snapshot-gated only (no upstream SVG baselines).
- Translation-based compatibility mode implemented in:
  - `crates/merman-core/src/diagrams/zenuml.rs`

Imported from docs (snapshot-gated):

- Sync message / method-call syntax (`A.SyncMessage(...) { ... }`)
- Creation messages (`new A1`, `new A2(with, parameters)`)
- Reply messages (assignment, `return result`, `@return` / `@reply`)
- `try/catch/finally`

TODO (next incremental batches from docs):

- Nesting semantics for sync blocks and mixed message types inside them.
- Rendered comments semantics (`// ...`) beyond “comment above next message”.
- Participant annotators beyond `@Actor` (document the mapping / degradation policy).
- Negative fixtures: known-unsupported statements should fail deterministically in strict mode.

## Bookkeeping

- If a fixture batch changes the upstream SVG corpus size, update:
  - `docs/alignment/STATUS.md`
  - `docs/alignment/PARITY_HARDENING_PLAN.md`
