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

- Removed `upstream_cypress_statediagram_v2_spec_can_have_styles_applied_034` after narrowing the
  72px border-label height inflation rule to classDef-compiled styles. Direct `style` directives no
  longer receive classDef-only height derivation.

Exit criteria:

- State root override count shrinks or the attempted candidate is documented as retained.
- `compare-state-svgs --check-dom --dom-mode parity-root --dom-decimals 3` passes.
- `report-overrides --check-no-growth` passes.
- `cargo clippy -p merman-render --all-targets --all-features -- -D warnings` passes if render code
  changed.

## M2: Mindmap First Pass

Scope:

- Classify Mindmap root viewport drift families.
- Replace at least one practical fixture group with typed or emitted-bounds derivation.
- Remove only entries that stay green under Mindmap parity gates.

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
