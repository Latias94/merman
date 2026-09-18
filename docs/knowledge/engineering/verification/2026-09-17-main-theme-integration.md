# Main Theme Repair Integration

Date: 2026-09-17. Integration parents: theme branch `89d960dd3` and fetched `origin/main`
`9edd6d86a2d6ad2d1707aa0edb3488483240277f`. This records the merge's semantic decisions and
scoped verification; it is not C7a contract freeze or a full platform release check.

## Integrated Behavior

- Init and frontmatter share the incoming CSS component-value admission boundary. Safe source
  colors and fonts are admitted by default; stylesheets remain host-controlled, and explicit
  host secure locks still apply. Invalid values cannot replace trusted defaults.
- The incoming complete Base theme evaluator uses the existing staged assignment machinery.
  Constructor preparation, explicit overlay, one ordered update, and explicit replay share
  the same source-ownership tracking. The obsolete Base evaluator and its parallel dependency
  repair rows are removed. No second theme interpreter or proof framework was added.
- Falsy intermediate overrides must not claim ownership of later fields that read a calculated
  fallback. Regression tests cover this distinction, nested derived fields, and the existing
  transactional 64-iteration resource bound.
- Source filtering retains cancellation and error categories while preserving the branch's
  effective source config, typed theme selection, fallback overlay, and explicit ownership.
- The retired `presentation_layering.rs` target stays retired. Its incoming explicit secure-lock
  regression is applied to the current `diagram_theme_layering.rs` test instead.
- Android SDK setup uses `platform-tools`; patched cryptography dependencies are reconciled with
  this branch's lockfile. License reports and package projections are regenerated from that
  dependency closure.

## Deliberate Merge Resolutions

The incoming Forest test expected `primaryColor` to replace `mainBkg`. The pinned
`theme-forest.js` initializes them independently; the existing staged evaluator correctly
retains Forest's `#cde498` main background. The test now distinguishes source admission from
that independent default. Base dark-mode evidence now includes the derived assignments it
actually controls, rather than only the two explicit dark-mode paths. Assertions retain
conceptual-field reconciliation and require sorted, unique surviving paths.

WASM artifact budgets retain the product baseline measured at `f2adde25e`. Main's separately
measured ASCII increase is recorded as its own evidence, not current-branch validation. No
artifact byte ceiling is raised by this merge, and no current-branch size result is inferred
from main's measurements.

## Verification

- Core Release tests, including the new source-presentation integration target: **1,674/1,674**
  passed. This includes generated Mermaid value/stage oracles and ownership regressions.
- Android workflow and license-script regressions: **22/22** passed.
- Generated Rust license reports: **13 reports** refreshed successfully; release legal
  projections: **382 files** checked successfully.
- A separate Codex source review compared the staged Base assignments against the incoming
  evaluator and checked parse error/cancellation propagation; no actionable finding remained.

Full workspace, installed platform packages, browser suites, and final WASM/native size or
performance measurements are outside this merge validation. They remain part of the existing
product-boundary plan.
