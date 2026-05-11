# Root Viewport Derivation Milestones

## Goal Statement

The next root viewport cleanup stage should reduce fixture-scoped root pins by replacing them with
local derivation rules that are easier to maintain and easier to reason about.

Success means:

- State and Mindmap no longer rely on any root pin that can be derived from typed layout or emitted
  SVG bounds.
- Remaining pins are documented as browser-measurement or model gaps.
- Strict parity gates stay green after each deletion.

## M0: Baseline and Tooling

Status: in progress.

Scope:

- Create the workstream docs.
- Reuse existing root audit tooling from `xtask`.
- Capture current State/Mindmap root override counts and drift families.

Exit criteria:

- `README.md`, `TODO.md`, `MILESTONES.md`, `AUDIT.md`, and `CHANGELOG.md` exist.
- State and Mindmap baseline counts are recorded.
- Focused audit commands are documented.
- `clippy`, `nextest`, `parity-root`, and strict gate expectations are explicit.

## M1: State First Pass

Status: in progress.

Scope:

- Classify State root viewport drift families.
- Replace at least one practical fixture group with typed or emitted-bounds derivation.
- Remove only entries that stay green under both State DOM parity modes.

Progress:

- Classified the then-current 42 retained State root pins with a disabled-root `parity-root` sweep.
  The largest drift families are HTML-sanitized notes, right-to-left scale bounds with long IDs,
  wrapping edge-label bounds, markdown labels, unicode/RTL text metrics, style/font precedence, and
  small browser float/lattice guards.
- Removed `upstream_cypress_statediagram_v2_spec_can_have_styles_applied_034` after narrowing the
  72px border-label height inflation rule to classDef-compiled styles. Direct `style` directives no
  longer receive classDef-only height derivation.
- Removed the two `test({ foo: 'far' })` State root pins after decoding Mermaid
  `encodeEntities` placeholders before layout measurement and moving the remaining browser width
  fact into a shared State edge-label text metric.
- Removed the two shared multiline note State root pins after moving the browser-measured note
  label width into State-owned note metrics and applying it consistently in layout and render.
- Removed the two simple State transition-label root pins after extending the existing
  `Transition 1/2/3` edge-label metric to the matching `Transition 4/5` labels without growing the
  text lookup budget.
- Removed the docs `A transition` State root pin by moving its browser-measured edge-label width
  into State edge-label metrics.
- Removed the shared `Your state with spaces in it` State root pins by moving its browser-measured
  node-label width into State node-label metrics.
- Removed the package style `id1/id2` State root pin by extending the existing bold-italic
  `id3/id4` node-label metric family without growing text lookup debt.
- Retained the `state_with_a_note_together_with_another_state` v1/v2 pair for now because the
  disabled-root drift is in note-cluster rect bounds, not a direct text width mismatch.
- Retained the next compound-title, style-precedence, and choice candidates for now because their
  disabled-root drift does not collapse to a single reusable typed metric.

Exit criteria:

- State root override count shrinks or the attempted candidate is documented as retained.
- `compare-state-svgs --check-dom --dom-mode parity-root --dom-decimals 3` passes.
- `report-overrides --check-no-growth` passes.
- `cargo clippy -p merman-render --all-targets --all-features -- -D warnings` passes if render code
  changed.

## M2: Mindmap First Pass

Status: in progress.

Scope:

- Classify Mindmap root viewport drift families.
- Replace at least one practical fixture group with typed or emitted-bounds derivation.
- Remove only entries that stay green under Mindmap parity gates.

Progress:

- Removed the three Cypress single-root shape pins (`square_shape_011`, `rounded_rect_shape_012`,
  and `circle_shape_013`) after Mindmap layout measurement started trimming delimiter-created
  labels with exactly one non-empty text line. SVG text emission still preserves the raw upstream
  whitespace, so this is a layout/bounds derivation rather than a DOM rewrite.
- Removed `upstream_docs_mindmap_circle_011` after Mindmap plain label measurement stopped using
  global fixture-derived HTML width overrides that belong to other diagram families. The remaining
  docs bang/cloud shape entries still guard emitted-bounds drift and stay pinned for now.

Exit criteria:

- Mindmap root override count shrinks or the attempted candidate is documented as retained.
- `compare-mindmap-svgs --check-dom --dom-mode parity-root --dom-decimals 3` passes.
- `report-overrides --check-no-growth` passes.
- `cargo clippy -p merman-render --all-targets --all-features -- -D warnings` passes if render code
  changed.

## M3: Broader Root-Debt Plan

Scope:

- Decide whether the State/Mindmap derivation patterns apply to Architecture, Flowchart, Sequence,
  or GitGraph.
- Record the next bucket order using evidence from the first passes.

Exit criteria:

- `AUDIT.md` maps each remaining root bucket to a derivation plan or retention reason.
- Strict release gate passes.
- `cargo nextest run` is green if shared layout or renderer contracts changed.
