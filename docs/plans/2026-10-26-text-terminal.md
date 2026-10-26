---
title: Preserve terminal text semantics across measurement and emission
type: fix
date: 2026-10-26
---

# Terminal text contracts

## Goal

Repair the confirmed font/text review findings on the main PR, then adapt the
applicable fixes to the 0.9 maintenance PR. Measurement, wrapping, bounds, and
emission must agree on visible text and effective terminal typography. Preserve
the pinned Mermaid DOM/CSS behavior rather than tuning geometry to fixtures.

## Decisions

- Extend existing label artifacts and styled word/line representations; do not
  create a second CSS engine or a universal diagram layout abstraction.
- SVG createText words explicitly reset inherited weight/style. Project these
  resets into SVG measurement while retaining Markdown strong/em runs. HTML
  inheritance remains independent.
- Preserve styled State runs when retaining measured rows. Do not flatten them
  to strings and attempt to recover their styles during emission.
- Resolve source declarations against the declarations that actually reach the
  terminal. Preserve pinned style compilation behavior, including its handling
  of invalid duplicate values, unless upstream evidence requires changing it.
- Keep wrapping-budget probes separate from final terminal bbox measurement.
- Keep native glyph-fallback ordering outside this change: it is an unresolved
  backend contract, not a confirmed measurement/painting divergence.
- No padding/overflow workaround, broad comparator normalization, relaxed
  residual tolerances, public ABI change, or new font-loading behavior.

## Implementation units

### U1. Family-local font sizes

Use the emitted root font for Sequence title geometry and Mindmap HTML labels.
Keep actor/message fonts and the SVG/HTML label paths distinct. Cover configured
large fonts through normal parser/layout/render entry points.

### U2. State styled line artifacts

Retain strong/em word types through native row geometry, bbox preparation, and
SVG emission. Preserve effective inline theme/source overrides on inner words.
Cover plain text, mixed emphasis, multiple rows, and existing edge padding.

### U3. Block and Usecase terminal typography

Apply Block's emitted source font longhands during label measurement, including
relative inheritance where the same declaration reaches multiple descendants.
Derive Usecase SVG measurement/wrapping from the actual terminal typography,
not HTML-only role selectors. Preserve HTML role fonts and source overrides.

### U4. Shared text projection and Class notes

Match SVG Markdown word measurement to terminal resets; keep HTML inheritance.
Make the lightweight Markdown path agree with the pinned full parser for code
spans. Measure Class SVG notes using the same Markdown projection as the writer.
Align Flowchart duplicate-declaration measurement with compiled terminal winners.

### U5. Integration and maintenance adaptation

Inspect the combined diff, run independent reviews, repair source-owned test or
snapshot drift with the existing generators, and adapt applicable fixes to 0.9.
Do not import main-only theme/evidence architecture into the maintenance branch.

## Verification

- Add or strengthen focused regression tests before implementation and record
  their expected failures. Workers do not run Cargo concurrently; the parent
  owns serial nextest runs, formatting, generated outputs, and canonical commits.
- Use style-sensitive measurement controls and independently inspect terminal
  text/attributes, not only parent styles or calls to the same adapter.
- Recheck current-source default SVGs in the browser for final computed font
  styles and Sequence title containment.
- Run affected renderer suites, layout snapshots, strict renderer Clippy, and
  cargo fmt. Run feature-leaf checks when shared feature boundaries change.
- Run the exact full SVG structure owner on each changed release line; retain
  source-backed semantic changes and do not weaken comparison policies.
- Keep main and maintenance verification records separate. Remote CI is only
  green when the current pushed SHA's checks actually finish successfully.

## Delivery

Use focused Conventional Commits on the existing PR branches. Preserve all
unrelated root-worktree Flutter changes. Record evidence and any remaining
limits in a separate validation document; the plan itself is not a progress log.
