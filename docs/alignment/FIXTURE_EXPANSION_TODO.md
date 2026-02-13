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
- 2026-02-12: More `packages/mermaid/src/docs/*` fixtures batch import (25 fixtures; ER/Gantt/GitGraph/XYChart) with upstream SVG baselines.
- 2026-02-12: Flowchart docs batch stabilization: Flowchart-v2 rendering-elements parity fixes (curly brace shapes + SVG-label escaped tag spacing) and a small refresh of root viewport overrides (State/Class/GitGraph/Mindmap/Architecture) to keep the global `parity-root` gate green.
- 2026-02-12: Imported a small set of additional docs-site fixtures (intro/examples) for Mindmap/Class/Sequence/Flowchart and refreshed root viewport overrides as needed to keep the global `parity-root` gate green.
- 2026-02-12: Imported additional Mermaid docs fixtures from `repo-ref/mermaid/docs/*` (Contributing flowcharts + `docs/diagrams/*` code-flow + mermaid-api sequence) and a small Mindmap syntax delta; refreshed Flowchart/Sequence root viewport overrides to keep the global `parity-root` gate green. (Skipped/deleted `layout: elk` / `look: ...` / math examples per deferred parity items.)
- 2026-02-11: State `stateDiagram.md` docs examples batch import (11 new fixtures; additional blocks were skipped as duplicates).
- 2026-02-11: Sequence `sequenceDiagram.md` docs examples batch import (16 new fixtures; additional blocks were skipped as duplicates).
- 2026-02-11: Class `classDiagram.md` docs examples batch import (13 new fixtures, including `hideEmptyMembersBox` and inline style variants).
- 2026-02-11: GitGraph `gitgraph.md` docs examples batch import (5 new fixtures: branch/line hiding and theme variants).
- 2026-02-11: External fixtures from `mermaid-rs-renderer` (Mindmap + Kanban + Flowchart + Sequence + Architecture: 5 fixtures).
- 2026-02-12: State Cypress rendering fixtures batch import (19 fixtures from `repo-ref/mermaid/cypress/integration/rendering/*.spec.{js,ts}` for state diagrams). Refreshed State root viewport overrides and hardened State SVG parity for `config.look=default` and composite self-loop nesting rules, keeping the global `parity-root` gate green.
- 2026-02-13: State Cypress rendering fixtures second batch import (8 fixtures; 27 total). Hardened State edge path handling by resolving `state-<id>-<n>` cluster endpoint references to the actual cluster bounds, and refreshed State root viewport overrides as needed to keep the global `parity-root` gate green.
- 2026-02-13: Sequence Cypress rendering fixtures import (19 fixtures from `repo-ref/mermaid/cypress/integration/rendering/sequencediagram*.spec.js`, with upstream SVG baselines). Hardened Sequence note wrapping, actor menu properties (`forceMenus`), and nested `rect` DOM ordering, keeping the global `parity-root` gate green.
- 2026-02-13: Mindmap Cypress rendering fixtures import (18 fixtures from `repo-ref/mermaid/cypress/integration/rendering/mindmap.spec.ts`, with upstream SVG baselines). Added Mindmap root viewport overrides for the new fixtures and aligned multi-line label DOM shape (`<p>` vs raw text nodes) to keep the global `parity-root` gate green.
- 2026-02-13: Class Cypress rendering fixtures import (19 fixtures from `repo-ref/mermaid/cypress/integration/rendering/classdiagram*.spec.{js,ts}`, with upstream SVG baselines). Added fixture-derived Class root viewport overrides and hardened Class SVG parity for multiline IDs (attribute `&#10;`), HTML label line breaks (`<br />`), themeVariables-driven colors, and single-namespace wrapper DOM, keeping the global `parity-root` gate green. (Deferred `classdiagram_handdrawn_v3.spec.*` due to deeper v3 DOM structure differences.)
- 2026-02-13: Architecture Cypress rendering fixtures import (3 fixtures from `repo-ref/mermaid/cypress/integration/rendering/architecture.spec.ts`, with upstream SVG baselines). Refreshed Architecture root viewport overrides for the new fixture IDs and kept the global `parity-root` gate green. (Most additional Cypress cases remain covered by the existing `upstream_architecture_cypress_*_normalized` fixtures; raw shorthand cases are still CLI-incompatible at `@11.12.2`.)
- 2026-02-13: Flowchart Cypress rendering fixtures import (14 fixtures from `repo-ref/mermaid/cypress/integration/rendering/*flowchart*.spec.*`, with upstream SVG baselines). Refreshed Flowchart root viewport overrides for the new handdrawn fixtures and kept the global `parity-root` gate green. (Deferred 4 `flowchart-elk` fixtures to `fixtures/_deferred/**` because `layout: elk` is not yet supported by the headless layout pipeline.)
- 2026-02-13: Flowchart Cypress rendering fixtures second batch import (16 fixtures from `repo-ref/mermaid/cypress/integration/rendering/*flowchart*.spec.*`, with upstream SVG baselines). Refreshed Flowchart root viewport overrides for the new handdrawn fixtures and hardened Flowchart SVG parity for strict-mode link href sanitization (deep-link protocols) and colored marker id separators, keeping the global `parity-root` gate green. (Deferred 11 additional `flowchart-elk` fixtures to `fixtures/_deferred/**` because `layout: elk` is not yet supported by the headless layout pipeline.)
- 2026-02-13: Flowchart Cypress rendering fixtures third batch import (13 fixtures from `repo-ref/mermaid/cypress/integration/rendering/*flowchart*.spec.*`, with upstream SVG baselines). Hardened Flowchart SVG parity for edge-id curve overrides by porting D3 `curveBumpY` and `curveCatmullRom`, keeping the global `parity-root` gate green. (Deferred 14 `flowchart-elk` fixtures to `fixtures/_deferred/**` because `layout: elk` is not yet supported by the headless layout pipeline.)
- 2026-02-13: Flowchart Cypress rendering fixtures fourth batch import (6 fixtures from `repo-ref/mermaid/cypress/integration/rendering/*flowchart*.spec.*`, with upstream SVG baselines). Refreshed Flowchart root viewport overrides for two new fixture IDs and kept the global `parity-root` gate green. (Deferred additional `flowchart-elk` fixtures to `fixtures/_deferred/**` because `layout: elk` is not yet supported by the headless layout pipeline.)
- 2026-02-13: Flowchart Cypress rendering fixtures fifth batch import (5 fixtures from `repo-ref/mermaid/cypress/integration/rendering/*flowchart*.spec.*`, with upstream SVG baselines). Added additional nested-subgraph direction fixtures and a multiline-text handdrawn fixture, keeping the global `parity-root` gate green. (Deferred `flowchart-elk` fixtures to `fixtures/_deferred/**` because `flowchart-elk` diagrams are not yet supported by the headless layout pipeline.)
- 2026-02-13: Flowchart Cypress rendering fixtures sixth batch import (14 fixtures from `repo-ref/mermaid/cypress/integration/rendering/*flowchart*.spec.*`, with upstream SVG baselines). Hardened Flowchart subgraph header parsing parity (internal whitespace affecting generated subgraph IDs) and refreshed Flowchart root viewport overrides for new fixture IDs, keeping the global `parity-root` gate green. (Deferred additional `flowchart-elk` fixtures to `fixtures/_deferred/**` because `layout: elk` is not yet supported by the headless layout pipeline.)
- 2026-02-13: Flowchart Cypress rendering fixtures seventh batch import (11 fixtures from `repo-ref/mermaid/cypress/integration/rendering/*flowchart*.spec.*`, with upstream SVG baselines). Hardened Flowchart parity for colored edge marker ids/defs (linkStyle + class-derived stroke) and class assignment to edge IDs, and refreshed Flowchart root viewport overrides for new fixture IDs, keeping the global `parity-root` gate green. (Deferred additional `flowchart-elk` fixtures to `fixtures/_deferred/**` because `layout: elk` is not yet supported by the headless layout pipeline.)

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
- Flowchart "layout and look" syntax reference examples (frontmatter `config: { look: ..., layout: ... }`) were briefly imported but removed due to deeper SVG DOM structure deltas (marker grouping / root wrappers / transition class). Track these as a dedicated Flowchart "layout+look" parity work item.
- Sequence config directive examples that require `sequence.wrap=true` and `sequence.width` layout parity.
- Sequence math rendering (`$$...$$`) parity (upstream uses browser math rendering and `<foreignObject>` output).
- C4 docs fixtures imported from Mermaid docs were temporarily tried and then removed because they require deeper SVG DOM parity
  beyond root viewport overrides (e.g. `textLength` and `tspan dy` behavior). Track these as a dedicated C4 parity work item.
- A complex Gantt docs fixture (`timeline with comments + frontmatter config`) was skipped due to non-trivial DOM deltas; revisit
  as a dedicated Gantt parity item after additional renderer hardening.
- Class Cypress `classdiagram_handdrawn_v3.spec.*` fixtures were deferred because they exercise the newer classDiagram-v3 DOM shape
  (different top-level group structure than Stage-B `render_class_diagram_v2_svg`).

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
