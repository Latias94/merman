# Phase 2 Parity Backlog (Mermaid@11.15.0)

Status: Active
Baseline: pinned Mermaid `11.15.0`
Pinned commit: `41646dfd43ac83f001b03c70605feb036afae46d`
Last updated: 2026-06-04

This backlog tracks the next hardening slice for the three diagram families that currently have
Phase 1 local support but are not yet admitted into the main SVG parity matrix:

- `treeView`
- `ishikawa`
- `eventmodeling`

Phase 1 means the local pipeline is complete for a source-backed minimum slice: detector, typed
parser and semantic model, render model, layout, SVG renderer, and semantic/layout snapshots. It
does not mean full upstream parity.

## Related Documents

- `docs/alignment/TREEVIEW_MINIMUM.md`
- `docs/alignment/TREEVIEW_UPSTREAM_TEST_COVERAGE.md`
- `docs/alignment/ISHIKAWA_MINIMUM.md`
- `docs/alignment/ISHIKAWA_UPSTREAM_TEST_COVERAGE.md`
- `docs/alignment/EVENTMODELING_MINIMUM.md`
- `docs/alignment/EVENTMODELING_UPSTREAM_TEST_COVERAGE.md`
- `docs/alignment/UNSUPPORTED_FAMILY_ADMISSION_RUBRIC.md`
- `docs/alignment/STATUS.md`

## Admission Target

Do not move these families into the main coverage matrix until each family has:

1. A minimal upstream SVG baseline corpus under `fixtures/upstream-svgs/<diagram>/`.
2. A dedicated `xtask compare-<diagram>-svgs` command, or an accepted shared compare path that
   includes the family explicitly.
3. Source-backed semantic and layout fixtures for the first upstream docs/Cypress batch.
4. Coverage documentation that lists covered upstream parser/rendering sources and deferred gaps.
5. A clear root viewport residual policy if `parity-root` differs from upstream.

## Cross-Family Work

| ID | Priority | Task | Exit Criteria |
|---|---:|---|---|
| P2C-001 | P0 | Add compare plumbing for the three Phase 1 families. | Done: `compare-tree-view-svgs`, `compare-ishikawa-svgs`, and `compare-eventmodeling-svgs` exist. |
| P2C-002 | P0 | Generate the first upstream SVG baseline batch from the pinned Mermaid commit. | Done: committed baselines exist under `fixtures/upstream-svgs/treeView`, `fixtures/upstream-svgs/ishikawa`, and `fixtures/upstream-svgs/eventmodeling`. |
| P2C-003 | P0 | Keep Phase 2 docs synchronized with the dashboard. | Done for Series 1-4; keep updating when DOM residuals change. |
| P2C-004 | P1 | Decide whether these families enter `compare-all-svgs` immediately or stay family-local until a broader gate is green. | The chosen policy is documented before any main matrix admission. |

## TreeView

Current Phase 1 coverage:

- Minimum doc fixture: `fixtures/treeView/upstream_docs_treeview_basic.mmd`
- Parser/source coverage: basic node rows, quoted multi-word names, child indentation, title,
  `accTitle`, and `accDescr`
- SVG smoke coverage: `crates/merman-render/tests/tree_view_svg_test.rs`

Upstream sources for the next fixture batch:

- `repo-ref/mermaid/packages/parser/tests/treeView.test.ts`
- `repo-ref/mermaid/cypress/integration/rendering/treeView/treeView.spec.ts`
- `repo-ref/mermaid/docs/syntax/treeView.md`

Backlog:

| ID | Priority | Task | Notes |
|---|---:|---|---|
| P2T-001 | P0 | Add upstream SVG baseline and compare command for the existing docs fixture. | Done. Family-local DOM parity mode now passes for the committed corpus. |
| P2T-002 | P0 | Import the Cypress simple, complex, multiple-root, and custom-config examples as fixtures. | Done: 4 Cypress fixtures with semantic/layout goldens and upstream SVG baselines. |
| P2T-003 | P1 | Audit title/accessibility DOM behavior against upstream renderer output. | Done: parser-source fixture covers `title` + `accTitle` + `accDescr`; root `<svg>` now emits upstream-shaped `aria-describedby` / `aria-labelledby` plus `<title>` / `<desc>` before `<style>`. |
| P2T-004 | P1 | Add root viewport notes after the first compare run. | Done: documented the current `parity-root` residual bucket as browser `getBBox()` / text-metric-derived `viewBox` and `max-width` drift; root `width` is aligned and no root `height` attr is emitted in the current corpus. |

Deferred:

- Exact Langium diagnostics and offsets.
- Strict float-level `getBBox()` parity for labels.

## Ishikawa

Current Phase 1 coverage:

- Minimum doc fixture: `fixtures/ishikawa/upstream_docs_ishikawa_basic.mmd`
- Parser/source coverage: basic hierarchy, unindented root, effect indented more than causes
- SVG smoke coverage: `crates/merman-render/tests/ishikawa_svg_test.rs`

Upstream sources for the next fixture batch:

- `repo-ref/mermaid/packages/mermaid/src/diagrams/ishikawa/ishikawa.spec.ts`
- `repo-ref/mermaid/cypress/integration/rendering/ishikawa/ishikawa.spec.ts`
- `repo-ref/mermaid/docs/syntax/ishikawa.md`

Backlog:

| ID | Priority | Task | Notes |
|---|---:|---|---|
| P2I-001 | P0 | Add upstream SVG baseline and compare command for the existing docs fixture. | Done. Family-local DOM parity mode now passes for the committed corpus. |
| P2I-002 | P0 | Import Cypress examples 1-5 and 12 as source-backed fixtures. | Done: 6 Cypress fixtures with semantic/layout goldens and upstream SVG baselines. |
| P2I-003 | P1 | Add config/theme fixtures for forest, dark, `diagramPadding`, and `useMaxWidth`. | Done: Cypress examples 7-11 have semantic/layout goldens, upstream SVG baselines, and green family-local DOM parity. |
| P2I-004 | P2 | Decide rough/handDrawn policy before importing the rough Cypress fixture. | Do not fake RoughJS parity with classic SVG branches. |

Deferred:

- RoughJS / hand-drawn renderer parity.
- Strict browser `getBBox()` float parity for labels and fish-head bounds.

## EventModeling

Current Phase 1 coverage:

- Minimum doc fixture: `fixtures/eventmodeling/upstream_docs_eventmodeling_minimum.mmd`
- Parser/source coverage: `tf`/`timeframe`, `rf`/`resetframe`, qualified entity identifiers,
  explicit and inferred relations, inline data, and `data` block references
- SVG smoke coverage: `crates/merman-render/tests/eventmodeling_svg_test.rs`

Upstream sources for the next fixture batch:

- `repo-ref/mermaid/packages/parser/tests/eventmodeling.test.ts`
- `repo-ref/mermaid/packages/mermaid/src/diagrams/eventmodeling/eventmodeling.spec.ts`
- `repo-ref/mermaid/cypress/integration/rendering/eventmodeling/eventmodeling.spec.ts`
- `repo-ref/mermaid/docs/syntax/eventmodeling.md`

Backlog:

| ID | Priority | Task | Notes |
|---|---:|---|---|
| P2E-001 | P0 | Add upstream SVG baseline and compare command for the existing minimum fixture. | Done. Family-local DOM parity mode now passes for the committed corpus. |
| P2E-002 | P0 | Import the six Cypress rendering examples as fixtures. | Done: 6 Cypress fixtures with semantic/layout goldens and upstream SVG baselines. |
| P2E-003 | P1 | Port parser fixture coverage for full syntax, qualified names, and reset frames. | These are already near the Phase 1 parser scope and should be cheap to snapshot. |
| P2E-004 | P1 | Decide the semantic policy for `entity`, `note`, and `gwt` before rendering them. | They are parsed upstream but explicitly outside Phase 1 render support. |
| P2E-005 | P2 | Audit data block HTML/foreignObject output beyond parity DOM structure. | Current local renderer emits upstream-shaped `foreignObject` HTML; strict sanitization and browser text metrics are still not claimed. |

Deferred:

- Full `note` and `gwt` rendering.
- Browser `foreignObject`, HTML sanitization, and text measurement parity.
- Exact upstream namespace runtime-state behavior; local swimlane reuse is intentionally stable.

## Suggested Execution Order

Progress as of 2026-06-04:

- Series 1-3 are complete at the admission-infrastructure level: family-local compare commands and
  upstream SVG baseline corpora exist for `treeView`, `ishikawa`, and `eventmodeling`.
- Series 4 first Cypress batch is imported: `treeView` has 4 Cypress fixtures, `ishikawa` has 6
  selected Cypress fixtures, and `eventmodeling` has 6 Cypress fixtures.
- `check-upstream-svgs --check-dom --dom-mode parity --dom-decimals 3` passes for all three
  families, so the upstream baseline corpora are reproducible.
- `compare-*-svgs` without `--check-dom` passes for all three families, proving parse/layout/render
  traversal works across the corpus.
- `compare-*-svgs --check-dom --dom-mode parity --dom-decimals 3` passes for all three family-local
  corpora after closing the treeView wrapper, ishikawa root/wrapper, and eventmodeling root/DOM
  shape residuals.

1. `P2T-001`: `treeView` baseline plus compare command for the existing fixture.
2. `P2I-001`: `ishikawa` baseline plus compare command for the existing fixture.
3. `P2E-001`: `eventmodeling` baseline plus compare command for the existing fixture.
4. Add the first Cypress fixture batch per family after each compare path exists.
5. Revisit matrix admission now that all three have passing family-local compare evidence; keep
   theme/config/rough/unsupported-statement policy work outside the main matrix until documented.

## Validation Gates

For documentation-only changes:

- `cargo run -p xtask -- check-alignment`

For each fixture or compare-tooling task:

- `cargo fmt --check`
- `cargo nextest run -p merman-core -p merman-render <diagram>`
- `cargo run -p xtask -- check-alignment`
- family-specific compare command once available
