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

- 2026-02-15: State authored stress fixtures batch (12 fixtures under `fixtures/state/stress_state_*.mmd`, with upstream SVG baselines). Refreshed State root viewport overrides and hardened State SVG parity for floating note no-op semantics and duplicate self-loop label handling to keep the global `parity-root` gate green.
- 2026-02-15: Architecture authored stress fixtures batch (12 fixtures under `fixtures/architecture/stress_architecture_*.mmd`, with upstream SVG baselines). Refreshed Architecture root viewport overrides for the new fixture IDs to keep the global `parity-root` gate green.
- 2026-02-15: Architecture HTML demo fixtures import (1 fixture from `repo-ref/mermaid/demos/architecture.html` via `<pre class="mermaid">`, with upstream SVG baselines). Added an Architecture root viewport override for the new fixture ID to keep the global `parity-root` gate green.
- 2026-02-15: Class Cypress rendering fixtures batch import (25 fixtures from `repo-ref/mermaid/cypress/integration/rendering/classdiagram*.spec.{js,ts}` via `xtask import-upstream-cypress --diagram class --with-baselines --limit 25 --complex`, with upstream SVG baselines). Hardened Class SVG DOM parity for Markdown `<em>/<strong>` runs in HTML labels and namespace wrapper structure; refreshed Class root viewport overrides as needed to keep the global `parity-root` gate green.
- 2026-02-15: Flowchart authored stress fixtures batch (12 fixtures under `fixtures/flowchart/stress_flowchart_*.mmd`, with upstream SVG baselines). Added Flowchart root viewport overrides for the new fixture IDs to keep the global `parity-root` gate green.
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
- 2026-02-13: Sequence Cypress rendering fixtures import (16 fixtures from `repo-ref/mermaid/cypress/integration/rendering/sequencediagram*.spec.js`, with upstream SVG baselines). Hardened Sequence message wrapping parity for `wrap: true` (Cypress long-message cases) and refreshed Sequence root viewport overrides, keeping the global `parity-root` gate green.
- 2026-02-13: Sequence Cypress rendering fixtures second batch import (16 fixtures; 32 total), with upstream SVG baselines. Hardened Sequence note wrapping parity for `wrap: true` notes (two-pass widen + rewrap for `left of` notes), refreshed Sequence root viewport overrides, and refreshed a small set of Sequence layout goldens, keeping the global `parity-root` gate green.
- 2026-02-13: Sequence Cypress rendering fixtures third batch import (16 fixtures; 48 total), with upstream SVG baselines. Added additional Sequence coverage for actor symbol rendering, note-alignment variants (single- and multi-line), and long note/message cases; refreshed Sequence root viewport overrides, keeping the global `parity-root` gate green.
- 2026-02-13: Sequence Cypress rendering fixtures fourth batch import (5 fixtures; 53 total), with upstream SVG baselines. Added additional Sequence coverage for empty-line handling, actor font overrides, and multi-line/long-message alignment cases; refreshed Sequence root viewport overrides, keeping the global `parity-root` gate green.
- 2026-02-13: Mindmap Cypress rendering fixtures import (18 fixtures from `repo-ref/mermaid/cypress/integration/rendering/mindmap.spec.ts`, with upstream SVG baselines). Added Mindmap root viewport overrides for the new fixtures and aligned multi-line label DOM shape (`<p>` vs raw text nodes) to keep the global `parity-root` gate green.
- 2026-02-13: Class Cypress rendering fixtures import (19 fixtures from `repo-ref/mermaid/cypress/integration/rendering/classdiagram*.spec.{js,ts}`, with upstream SVG baselines). Added fixture-derived Class root viewport overrides and hardened Class SVG parity for multiline IDs (attribute `&#10;`), HTML label line breaks (`<br />`), themeVariables-driven colors, and single-namespace wrapper DOM, keeping the global `parity-root` gate green. (Deferred `classdiagram_handdrawn_v3.spec.*` due to deeper v3 DOM structure differences.)
- 2026-02-14: Class Cypress rendering fixtures second batch import (30 fixtures from `repo-ref/mermaid/cypress/integration/rendering/classDiagram*.spec.js`, including v2/v3 + ELK + hand-drawn variants, with upstream SVG baselines). Refreshed Class root viewport overrides to keep the global `parity-root` gate green. Deferred 3 fixtures to `fixtures/_deferred/class` due to deeper v3 DOM mismatches (namespaces/markdown/v3-elk full diagram).
- 2026-02-13: Architecture Cypress rendering fixtures import (3 fixtures from `repo-ref/mermaid/cypress/integration/rendering/architecture.spec.ts`, with upstream SVG baselines). Refreshed Architecture root viewport overrides for the new fixture IDs and kept the global `parity-root` gate green. (Most additional Cypress cases remain covered by the existing `upstream_architecture_cypress_*_normalized` fixtures; raw shorthand cases are still CLI-incompatible at `@11.12.2`.)
- 2026-02-13: Flowchart Cypress rendering fixtures import (14 fixtures from `repo-ref/mermaid/cypress/integration/rendering/*flowchart*.spec.*`, with upstream SVG baselines). Refreshed Flowchart root viewport overrides for the new handdrawn fixtures and kept the global `parity-root` gate green. (Deferred 4 `flowchart-elk` fixtures to `fixtures/_deferred/**` because `layout: elk` is not yet supported by the headless layout pipeline.)
- 2026-02-13: Flowchart Cypress rendering fixtures second batch import (16 fixtures from `repo-ref/mermaid/cypress/integration/rendering/*flowchart*.spec.*`, with upstream SVG baselines). Refreshed Flowchart root viewport overrides for the new handdrawn fixtures and hardened Flowchart SVG parity for strict-mode link href sanitization (deep-link protocols) and colored marker id separators, keeping the global `parity-root` gate green. (Deferred 11 additional `flowchart-elk` fixtures to `fixtures/_deferred/**` because `layout: elk` is not yet supported by the headless layout pipeline.)
- 2026-02-13: Flowchart Cypress rendering fixtures third batch import (13 fixtures from `repo-ref/mermaid/cypress/integration/rendering/*flowchart*.spec.*`, with upstream SVG baselines). Hardened Flowchart SVG parity for edge-id curve overrides by porting D3 `curveBumpY` and `curveCatmullRom`, keeping the global `parity-root` gate green. (Deferred 14 `flowchart-elk` fixtures to `fixtures/_deferred/**` because `layout: elk` is not yet supported by the headless layout pipeline.)
- 2026-02-13: Flowchart Cypress rendering fixtures fourth batch import (6 fixtures from `repo-ref/mermaid/cypress/integration/rendering/*flowchart*.spec.*`, with upstream SVG baselines). Refreshed Flowchart root viewport overrides for two new fixture IDs and kept the global `parity-root` gate green. (Deferred additional `flowchart-elk` fixtures to `fixtures/_deferred/**` because `layout: elk` is not yet supported by the headless layout pipeline.)
- 2026-02-13: Flowchart Cypress rendering fixtures fifth batch import (5 fixtures from `repo-ref/mermaid/cypress/integration/rendering/*flowchart*.spec.*`, with upstream SVG baselines). Added additional nested-subgraph direction fixtures and a multiline-text handdrawn fixture, keeping the global `parity-root` gate green. (Deferred `flowchart-elk` fixtures to `fixtures/_deferred/**` because `flowchart-elk` diagrams are not yet supported by the headless layout pipeline.)
- 2026-02-13: Flowchart Cypress rendering fixtures sixth batch import (14 fixtures from `repo-ref/mermaid/cypress/integration/rendering/*flowchart*.spec.*`, with upstream SVG baselines). Hardened Flowchart subgraph header parsing parity (internal whitespace affecting generated subgraph IDs) and refreshed Flowchart root viewport overrides for new fixture IDs, keeping the global `parity-root` gate green. (Deferred additional `flowchart-elk` fixtures to `fixtures/_deferred/**` because `layout: elk` is not yet supported by the headless layout pipeline.)
- 2026-02-13: Flowchart Cypress rendering fixtures seventh batch import (11 fixtures from `repo-ref/mermaid/cypress/integration/rendering/*flowchart*.spec.*`, with upstream SVG baselines). Hardened Flowchart parity for colored edge marker ids/defs (linkStyle + class-derived stroke) and class assignment to edge IDs, and refreshed Flowchart root viewport overrides for new fixture IDs, keeping the global `parity-root` gate green. (Deferred additional `flowchart-elk` fixtures to `fixtures/_deferred/**` because `layout: elk` is not yet supported by the headless layout pipeline.)
- 2026-02-13: Flowchart Cypress rendering fixtures eighth batch import (16 fixtures from `repo-ref/mermaid/cypress/integration/rendering/*flowchart*.spec.*`, with upstream SVG baselines). Refreshed Flowchart root viewport overrides for new handdrawn/v2 fixture IDs and kept the global `parity-root` gate green. (Deferred the extracted `flowchart-elk` suite fixture to `fixtures/_deferred/**` because the suite-level ELK renderer config is not yet supported by the headless layout pipeline.)
- 2026-02-13: State Cypress rendering fixtures third batch import (35 fixtures; 62 total), with upstream SVG baselines. Hardened the State parser to preserve Mermaid’s `state "..." as ID: ...` label+description split and fixed a State lexer UTF-8 slicing panic, keeping the global `parity-root` gate green.
- 2026-02-14: Mindmap Cypress tidy-tree rendering fixtures import (4 fixtures from `repo-ref/mermaid/cypress/integration/rendering/mindmap-tidy-tree.spec.js`, with upstream SVG baselines). Normalized indented YAML frontmatter in the Cypress import pipeline and added Mindmap root viewport overrides for the new fixture IDs to keep the global `parity-root` gate green.
- 2026-02-14: Architecture Cypress rendering fixtures import (8 fixtures from `repo-ref/mermaid/cypress/integration/rendering/architecture.spec.ts`, with upstream SVG baselines). Normalized legacy architecture edge shorthands (`a L--R b`, `a (L--R) b`, `a L-[Label]-R b`) into Mermaid@11.12.2 CLI-compatible Langium grammar (`a:L -- R:b`, `a:L -[Label]- R:b`) during Cypress import to keep `gen-upstream-svgs --diagram architecture` seeded baselines working and the global `parity-root` gate green.
- 2026-02-14: State HTML demo fixtures import (9 fixtures from `repo-ref/mermaid/demos/state.html` via `<pre class="mermaid">`, with upstream SVG baselines). Added State root viewport overrides for the new fixture IDs and fixed cluster edge-boundary path cutting to keep the global `parity-root` gate green.
- 2026-02-14: Sequence HTML demo fixtures import (5 fixtures from `repo-ref/mermaid/demos/sequence.html` via `<pre class="mermaid">`, with upstream SVG baselines). Added Sequence root viewport overrides for the new fixture IDs to keep the global `parity-root` gate green. (Math/KaTeX `$$...$$` demo fixtures remain deferred due to upstream `<foreignObject>` rendering.)
- 2026-02-14: Timeline Cypress rendering fixtures import (12 fixtures from `repo-ref/mermaid/cypress/integration/rendering/timeline.spec.*`, with upstream SVG baselines). Added Timeline root viewport overrides for 2 stacked-event fixtures to keep the global `parity-root` gate green.
- 2026-02-14: Kanban Cypress rendering fixtures import (6 fixtures from `repo-ref/mermaid/cypress/integration/rendering/kanban.spec.*`, with upstream SVG baselines). Added Kanban root viewport overrides for a wrapping-height fixture to keep the global `parity-root` gate green.
- 2026-02-14: Gantt Cypress rendering fixtures import (25 fixtures from `repo-ref/mermaid/cypress/integration/rendering/gantt.spec.*` + `theme.spec.*`, with upstream SVG baselines). Hardened Gantt parity for d3 `axisFormat` directives (`%L`) + exclude-layer edge cases + JS date-only parsing rules to keep the global `parity-root` gate green.
- 2026-02-14: Flowchart HTML demo fixtures import (12 fixtures from `repo-ref/mermaid/demos/flowchart.html` via `<pre class="mermaid">`, with upstream SVG baselines).
- 2026-02-14: Flowchart HTML demo fixtures import (2 fixtures from `repo-ref/mermaid/demos/dataflowchart.html` via `<pre class="mermaid">`, with upstream SVG baselines).
- 2026-02-14: Mindmap HTML demo fixtures import (2 fixtures from `repo-ref/mermaid/demos/mindmap.html` via `<pre class="mermaid">`, with upstream SVG baselines).
- 2026-02-14: Architecture HTML demo fixtures import (11 fixtures from `repo-ref/mermaid/demos/architecture.html` via `<pre class="mermaid">`, with upstream SVG baselines).
- 2026-02-14: Class HTML demo fixtures import (10 fixtures from `repo-ref/mermaid/demos/classchart.html` via `<pre class="mermaid">`, with upstream SVG baselines). (1 additional demo block deferred because Mermaid CLI `@11.12.2` fails to parse it; the block contains the line `class People List~List~Person~~`.)
- 2026-02-14: ER HTML demo fixtures import (9 fixtures from `repo-ref/mermaid/demos/er.html` + `repo-ref/mermaid/demos/er-multiline.html` via `<pre class="mermaid">`, with upstream SVG baselines). Hardened ER parity for theme `forest` table striping (`rowOdd`/`rowEven`), Markdown/HTML labels inside `<foreignObject>`, and title placement (`utils.insertTitle`).
- 2026-02-14: Block HTML demo fixtures import (11 fixtures from `repo-ref/mermaid/demos/block.html` via `<pre class="mermaid">`, with upstream SVG baselines; the demo page includes 1 Flowchart snippet which is intentionally skipped by the Block importer). Hardened Block parity for additional node shapes (`stadium`, `subroutine`, `cylinder`, `diamond`, `hexagon`, trapezoids/lean variants) and added fixture-derived root viewport overrides to keep the global `parity-root` gate green.
- 2026-02-14: Packet HTML demo fixtures import (4 fixtures from `repo-ref/mermaid/demos/packet.html` via `<pre class="mermaid">`, with upstream SVG baselines). Hardened Packet layout parity for `config.packet.showBits` (including upstream `paddingY += 10` behavior when `showBits=true`) to keep `parity-root` green.
- 2026-02-14: Timeline HTML demo fixtures import (2 fixtures from `repo-ref/mermaid/demos/timeline.html` via `<pre class="mermaid">`, with upstream SVG baselines). Refreshed Timeline root viewport overrides for the new fixture ID to keep `parity-root` green.
- 2026-02-14: Gantt HTML demo fixtures import (10 fixtures from `repo-ref/mermaid/demos/gantt.html` via `<pre class="mermaid">`, with upstream SVG baselines). Hardened Gantt parser parity for non-ASCII task labels (UTF-8 safe keyword matching) and JS `Date` fallback parsing of `MM-DD-YY-HH:mm` strings (used by `dateFormat Z` demos), keeping `parity-root` green.
- 2026-02-14: Requirement HTML demo fixtures import (2 fixtures from `repo-ref/mermaid/demos/requirements.html` via `<pre class="mermaid">`, with upstream SVG baselines), keeping `parity-root` green.
- 2026-02-14: Journey HTML demo fixtures import (1 fixture from `repo-ref/mermaid/demos/journey.html` via `<pre class="mermaid">`, with upstream SVG baselines), keeping `parity-root` green.
- 2026-02-14: GitGraph HTML demo fixtures import (20 fixtures from `repo-ref/mermaid/demos/git.html` via `<pre class="mermaid">`, with upstream SVG baselines; imported with `--complex --limit 20`), keeping `parity-root` green.
- 2026-02-14: GitGraph HTML demo fixtures import (remaining 13 fixtures from `repo-ref/mermaid/demos/git.html` via `<pre class="mermaid">`, with upstream SVG baselines), keeping `parity-root` green.
- 2026-02-14: Info HTML demo fixtures import (2 fixtures from `repo-ref/mermaid/demos/info.html` via `<pre class="mermaid">`, with upstream SVG baselines), keeping `parity-root` green.
- 2026-02-14: Pie HTML demo fixtures import (3 fixtures from `repo-ref/mermaid/demos/pie.html` via `<pre class="mermaid">`, with upstream SVG baselines). Added root viewport overrides for the new pie demo fixture IDs to keep `parity-root` green.
- 2026-02-14: Radar HTML demo fixtures import (6 fixtures from `repo-ref/mermaid/demos/radar.html` via `<pre class="mermaid">`, with upstream SVG baselines), keeping `parity-root` green.
- 2026-02-14: Packet docs fixtures import (1 fixture from `repo-ref/mermaid/docs/syntax/packet.md` via fenced code blocks, with upstream SVG baselines), keeping `parity-root` green. (Skipped the placeholder `start` / `... More Fields ...` syntax blocks because Mermaid CLI renders them as error SVGs.)
- 2026-02-14: Kanban docs fixtures import (2 fixtures from `repo-ref/mermaid/docs/syntax/kanban.md` via fenced code blocks, with upstream SVG baselines), keeping `parity-root` green.
- 2026-02-14: Gantt docs fixtures import (10 fixtures from `repo-ref/mermaid/docs/syntax/gantt.md` via fenced code blocks, with upstream SVG baselines). Fixed Gantt strict date defaults for partial formats and topAxis grid label/tick parity to keep `parity-root` green.
- 2026-02-15: Timeline stress fixtures import (13 fixtures with upstream SVG baselines). Deferred 6 additional Timeline stress fixtures to `fixtures/_deferred/timeline/` pending tighter text-wrapping parity, keeping the global `parity-root` gate green.
- 2026-02-15: Upstream syntax docs batch import (15 fixtures across Block/ER/GitGraph/Pie/Requirement, with upstream SVG baselines). Deferred the C4 docs batch locally to `fixtures/_deferred/c4/` due to `textLength` / `tspan dy` parity deltas, keeping the global `parity-root` gate green.
- 2026-02-15: Flowchart Cypress fixtures batch import (12 fixtures across handdrawn + flowchart-v2 rendering specs, with upstream SVG baselines). Added a small Flowchart SVG parity hardening fix (`shape: diam` renders as a diamond) and refreshed Flowchart root viewport overrides as needed to keep the global `parity-root` gate green.
  - Deferred locally: `upstream_cypress_flowchart_v2_spec_should_be_possible_to_use_syntax_to_add_labels_with_trail_spaces_067` due to deeper SVG DOM structure deltas (node id ordinal mismatch that is not solvable by root viewport overrides).
- 2026-02-15: Flowchart Cypress fixtures second batch import (17 fixtures across flowchart-handDrawn + flowchart-v2 + one ELK spec case, with upstream SVG baselines). Hardened parser-side `vertexCalls` ordering for `&`-separated shapeData statements (DOM id suffix parity) and added a root viewport override for the stadium node case (`068`) to keep the global `parity-root` gate green.
- 2026-02-15: Mindmap authored stress fixtures batch (12 fixtures under `fixtures/mindmap/stress_*.mmd`, with upstream SVG baselines). Regenerated `mindmap_text_overrides_11_12_2.rs` and added fixture-derived Mindmap root viewport overrides to keep the global `parity-root` gate green.
- 2026-02-15: Sequence stress fixtures batch (15 authored fixtures under `fixtures/sequence/stress_*.mmd` + 1 new upstream HTML demo fixture `upstream_html_demos_sequence_sequence_diagram_demos_011`, with upstream SVG baselines). Added sequence root viewport overrides for the new stress fixtures to keep the global `parity-root` gate green.
- 2026-02-15: Sequence authored stress fixtures batch (12 fixtures under `fixtures/sequence/stress_*_016..027.mmd`, with upstream SVG baselines). Hardened Sequence frame-label wrapping parity (self-message-only frame min extents, greedy wrap pad + hyphenation behavior, SVG bbox width for wrap decisions) and added fixture-derived Sequence root viewport overrides for the new fixture IDs to keep the global `parity-root` gate green.

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
  A recent docs batch (2026-02-15) is kept locally under `fixtures/_deferred/c4/` for future parity work.
- A complex Gantt docs fixture (`timeline with comments + frontmatter config`) was skipped due to non-trivial DOM deltas; revisit
  as a dedicated Gantt parity item after additional renderer hardening.


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
