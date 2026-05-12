# Root Viewport Derivation Workstream

This workstream follows the fearless-refactor closeout. The remaining root viewport override
entries are no longer treated as blind-pruning debt; they are tracked as typed bounds and
measurement derivation targets.

## Objective

Replace fixture-scoped root viewport overrides with typed layout bounds, emitted SVG bounds, or
shared browser-measurement derivation where practical, starting with State and Mindmap and then
revisiting Sequence once the first derivation patterns are proven, while keeping `parity-root` and
strict release gates green.

## Initial Scope

- State root viewport overrides: `45` entries.
- Mindmap root viewport overrides: `52` entries.
- Current State root viewport overrides: `34` entries after the style-directive border,
  Mermaid entity-placeholder edge-label, multiline note-label, transition edge-label, and shared
  alias/styled node-label derivation passes.
- Current Mindmap root viewport overrides: `39` entries after deriving the single-line delimiter
  label bounds for the Cypress square/rounded-rect/circle fixtures, the docs circle plain-label
  measurement path, the docs cloud emitted path bbox, plain wrapping-label container bounds, and
  the stale retained pins exposed by the post-wrapping disabled-root sweep.
- Current Sequence root viewport overrides: `179` entries after deriving the small-font Sequence
  note/message height path, the docs boundary actor-spacing path, and the title/accessibility
  default-message width path, including the residual default-title pair and simple note-right
  clusters.
- Current root viewport override budget: `717` entries.
- Current SVG text metric table budget: `186` rows after adding two Sequence message-width facts
  for the docs boundary root pin and correcting two existing default-message facts for the
  title/accessibility cluster.
- Keep the existing strict gate green:

```sh
cargo run -p xtask -- verify --strict
```

## Focused Audit Commands

Use focused parity-root audits before and after each candidate deletion:

```sh
cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all
cargo run -p xtask -- compare-mindmap-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all
cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all
```

Use disabled-root sweeps only as diagnostic input. They are expected to fail until each bucket has
typed bounds coverage:

```pwsh
$env:MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES='1'
cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all
cargo run -p xtask -- compare-mindmap-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all
Remove-Item Env:\MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES
```

## Success Criteria

- Each removed root viewport entry is replaced by a deterministic derivation rule, not by another
  fixture-specific pin.
- Each retained entry has current evidence explaining the drift source.
- `cargo run -p xtask -- compare-<diagram>-svgs --check-dom --dom-mode parity-root --dom-decimals 3`
  passes for each touched diagram family.
- `cargo run -p xtask -- report-overrides --check-no-growth` passes and budgets only shrink unless
  growth is explicitly justified.
- `cargo clippy -p merman-render --all-targets --all-features -- -D warnings` passes after render
  code changes.
- `cargo nextest run` passes before release closeout when the blast radius reaches shared rendering
  or layout code.
- `TODO.md`, `MILESTONES.md`, `AUDIT.md`, and `CHANGELOG.md` stay current.

## Strategy

Start with smaller, better-bounded buckets before revisiting GitGraph, Sequence, or Flowchart.
State is first because the remaining drift clusters around scale/direction, edge-label bounds,
notes, and small text/shape float differences. Mindmap follows because the remaining drift clusters
around wrapping text, icons, shapes, and long-label bounds.
