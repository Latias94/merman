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

## Medium priority (coverage growth)

TODO:

- Expand fixtures for:
  - `gantt`, `gitgraph`, `timeline`, `kanban`, `journey`, `packet`
  - `block`, `c4`, `requirement`, `radar`, `treemap`, `xychart`, `quadrantchart`, `sankey`

Focus areas:

- edge-case parsing (escaping, unicode/punctuation)
- config variants that change layout/labels
- error-handling surfaces (without weakening Mermaid parity contract)

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
