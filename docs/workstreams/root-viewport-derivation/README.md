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
- Current State root viewport overrides: `33` entries after the style-directive border,
  Mermaid entity-placeholder edge-label, multiline note-label, transition edge-label, and shared
  alias/styled node-label derivation passes. The latest disabled-root State retained-root recheck
  still maps all `33` generated keys to exact root-delta rows, with `32` snapped `parity-root` DOM
  mismatches and one exact-only root guard. The retained table spans noteGroup bounds, RTL/scale
  roots, edge-label wrapping, style/font precedence, and small browser-float guards rather than one
  safe shared rule.
- Current Mindmap root viewport overrides: `39` entries after deriving the single-line delimiter
  label bounds for the Cypress square/rounded-rect/circle fixtures, the docs circle plain-label
  measurement path, the docs cloud emitted path bbox, plain wrapping-label container bounds, and
  the stale retained pins exposed by the post-wrapping disabled-root sweep. The separate
  hand-written Mindmap profile calibration block is now gone; its remaining cases were replaced by
  Mindmap-owned plain HTML label metrics rather than fixture, glyph, or root lookup data.
- Current Sequence root viewport overrides: `58` entries after the follow-up Sequence
  message-width metric, note/message/frame, actor/root-bounds, and SVG metric-table cleanup
  passes plus the latest stale-pin cross-check and the participant lifecycle-height derivation.
  The participant creation/destruction v2 fixture now derives from Mermaid's pre-render actor
  layout-height lifecycle cursor rule instead of a root pin. The remaining
  table is split into retained message/note text measurement, nested frame/rect vertical geometry,
  and typed participant width/spacing debt rather than stale table debt. This supersedes the
  earlier pending revisit TODO that waited on message-width inference before reopening the bucket.
  The narrower text escaping / line-break subfamily is retained too: the focused disabled-root
  slice over the line-break, colon, escaping, wrapped-message, whitespace-semicolon, and note-with-
  br fixtures still shows `6` positive width drifts, `0` negative width drifts, `0` height
  changes, and one exact match. The narrower nested frame / rect vertical subfamily is retained
  as well: `stress_deep_nested_frames_018`, `stress_nested_frames_001`, and
  `stress_nested_rect_par_029` remain height-only root guards, but element probes split the drift
  across footer placement, nested frame internals, rect/par cursor movement, activation bounds, and
  note/loop bounds rather than one safe shared vertical boundary rule. The typed participant
  width/spacing subfamily is retained too: focused disabled-root checks still show mixed root
  width signs across typed Cypress fixtures (`+12`, `+35`, `+14`) and the adjacent quoted/typed
  stress fixture (`-7`), with actor-column, message-center, and note-width deltas rather than one
  shared actor visual-width rule.
- Current Journey root viewport overrides: `0` entries. The remaining long-label Cypress roots now
  derive from Journey actor legend single-run SVG computed text length, floored to the 1/32px
  browser lattice used by the emitted `<text><tspan>line</tspan></text>` labels.
- Current Requirement root viewport overrides: `7` entries after deriving the styled
  `test_req`/`test_entity` repeated Cypress roots from final CSS `font-weight` label measurement.
  The remaining Requirement pins still cover mixed root drift: font-size precedence, prototype/
  frontmatter offsets, long requirement/element name width and height drift, the docs combined
  `font-weight:bold` 1px browser lattice residual, and the large HTML demo stack.
- Current Timeline root viewport overrides: `8` entries after deriving the empty Timeline root
  from typed layout bounds. Empty Timeline diagrams no longer invent a synthetic 100px pre-title
  content box, so `upstream_pkgtests_diagram_orchestration_spec_046` derives its upstream `400px`
  root naturally. The remaining Timeline pins still cover title/label browser bbox width drift,
  CJK/emoji text-height drift, and Fira Sans vertical-line height accumulation rather than a clean
  shared rule.
- Current ER root viewport overrides: `7` entries after deriving the simple frontmatter-title
  root from emitted title bounds and moving the shared `DELIVERY-ADDRESS`,
  `PRODUCT-CATEGORY`, `Customer Account Tertiary`, `CATEGORY`, and `This **is** _Markdown_`
  entity-label browser widths into ER-owned HTML label metrics. The five 16px entity-label metrics replace thirteen fixture-scoped root pins
  across the package/docs/accessibility, not-so-simple/theme/syntax-reference, and
  relationship-line-break/html-demo/cardinality-alias/markdown-formatting variants. The remaining
  ER pins still cover recursive relationship geometry, edge-label bounds, docs layout, large HTML
  demo, multiline demo, and error-demo residuals rather than one safe shared rule.
- The global generated root override audit is currently clean on stale pins after the ER title and
  entity-label cleanups. The latest `audit-root-overrides --fail-on-stale` report covers `287`
  inventory entries, `293` fixture keys, `293` retained root-delta keys, `280` disabled-root
  DOM mismatches, `0` stale entries, and the same three accepted Mindmap outside-table DOM
  residuals, so the current baseline is stable rather than stale.
- Current GitGraph root viewport overrides: `23` entries after deriving GitGraph title text
  bounds, branch line endpoints, horizontal branch-label widths, commit/tag label computed-length
  widths, vertical branch-label centered SVG bbox widths, upstream seeded auto-id warm-up
  behavior, and the `BT` + `parallelCommits` compact-axis mirror, then honoring commit/tag label
  theme-variable styles and pruning the now-derived pins while retaining the remaining table as real
  exact root-drift guards. The latest GitGraph retained-root recheck found `23` high-precision
  generated root-delta keys, `15` snapped `parity-root` DOM mismatches, and no clean shared
  branch/commit/tag measurement rule to remove another pin without causing outside-table drift.
- Current Flowchart root viewport overrides: `43` inventory entries covering `49` fixture keys
  after deriving imageSquare
  image-plus-label layout bounds, anchor dot layout bounds, C1 replacement-glyph HTML label
  measurement, SVG-like subgraph-title/root bounds, Unicode/entities HTML title bounds, HTML-label
  font-size precedence, iconSquare outer layout bounds, the unregistered custom FontAwesome
  fallback advance, and LR fork/join direction-sensitive layout bounds, then pruning the now-derived docs
  parameters, old-shape set5, courier long-name/class-definition, stage2 long-word title,
  Unicode/entities title, stale subgraph title-margin pins, numeric-vs-px-string font-size root,
  docs icon-shape root, custom-icon fallback roots, eight old-shape set3 LR fork roots, and the
  quoted-numeric `rankSpacing: '100'` root. The latest chained-statement pass derives
  `upstream_cypress_flowchart_spec_20_multiple_nodes_and_chaining_in_one_statement_020` by
  matching Mermaid's split htmlLabels semantics for node labels versus edge/subgraph/CSS label
  behavior. The latest icon multiline pass derives `stress_flowchart_icons_multiline_br_054` by
  preserving FontAwesome icon-only HTML lines as measured DOM line boxes.
  The latest FontAwesome boundary pass derives `stress_flowchart_icons_unicode_and_wrap_056`
  without adding per-icon glyph-width data; the remaining icon root guards stay retained because
  exact parity would require real FontAwesome per-icon advance widths.
  The latest table-only cleanup collapses exact-duplicate Flowchart match arms with Rust
  or-patterns; it reduces inventory rows without changing fixture-key coverage or rendering
  behavior.
- Current root viewport override budget: `287` entries.
- Current text metric lookup budget: `489` entries after adding the ER-owned
  `DELIVERY-ADDRESS`, `PRODUCT-CATEGORY`, `Customer Account Tertiary`, `CATEGORY`, and
  `This **is** _Markdown_` browser width facts.
- Current SVG text metric table budget: `186` rows after adding two Sequence message-width facts
  for the docs boundary root pin and correcting existing default message/actor text facts for the
  title/accessibility, simple Cypress, arrow variant, package sequence, and docs/control sequence
  clusters.
- Closeout status: complete with an explicit root-parity residual policy. Full
  `cargo run -p xtask -- verify --strict` passes and prints the five accepted root residuals:
  two Class `different_text_labels_037` roots and three Mindmap docs/example roots. The policy is
  exact: changed values, missing residuals, or additional residuals fail the gate.
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
cargo run -p xtask -- compare-gitgraph-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all
cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all
cargo run -p xtask -- compare-er-svgs --check-dom --dom-mode parity-root --dom-decimals 3
```

Use disabled-root sweeps only as diagnostic input. They are expected to fail until each bucket has
typed bounds coverage:

```pwsh
$env:MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES='1'
cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all
cargo run -p xtask -- compare-mindmap-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all
cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all
cargo run -p xtask -- compare-gitgraph-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all
cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all
cargo run -p xtask -- compare-er-svgs --check-dom --dom-mode parity-root --dom-decimals 3
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

Start with smaller, better-bounded buckets before broad table pruning in GitGraph, Sequence, or
Flowchart. Disabled-root cross-checks can still remove stale retained pins, but broader reductions
should come from typed bounds or shared measurement rules rather than blind deletion.
