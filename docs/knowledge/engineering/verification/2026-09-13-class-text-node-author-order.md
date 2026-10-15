# Class NodeLabel and generic Text author order

Date: 2026-09-13

Source commit: `00174dc2abe29630f6f298a019b64282faad237e`.

Class NodeLabel paint now resolves through the existing Text/role resolver in the static,
ordinal, and terminal fallback paths. A later generic Text rule suppresses an earlier
NodeLabel inline paint; a later NodeLabel rule still wins. Text ordinals participate in
the existing per-node cache, so binding node IDs does not repeat ordinal work.

This is a prerequisite correctness repair, not Class Text retirement. Static generic Text
retains compatibility residuals. The tested Text ordinal request remains
Incomplete(required=2, accounted=1), and RequirePortable returns the corresponding
IncompleteFamilyTheme error. No route, support revision, or qualification cell is promoted.
The inventory remains 31/33 bridge-free families and 36 legacy routes (Class 4, Block 32).

## Verification

The original two public SVG regressions failed on older NodeLabel inline paint remaining
active. The corrected implementation passed five focused Release tests, covering author
order and Default selectors, HTML/SVG labels, occurrence-local ordinals, Clear/Transparent,
source-owned paint, once-only binding accounting, and insufficient-work-budget rejection.

The private Release owner suite passed 3278/3278 with two existing skips. It includes the
renderer, facade, bindings-core and acceptance library suites, Class SVG with ELK/math,
support discovery, authoring, route authorization and Block/Class retirement tests.
Log: `/tmp/class-text-node-order-owner.log`.

Standards and Spec reviews both reported no actionable findings after boundary tests were
added. The reviews were read-only. They confirmed that incomplete generic Text accounting
must remain explicit until the actual Text terminals migrate.

The full SVG structure comparison, renderer Clippy (existing unrelated warnings only),
workspace formatting and diff checks passed in the main worktree. Logs:
`/tmp/class-text-node-order-structure.log` and `/tmp/class-text-node-order-clippy.log`.

```text
CARGO_BUILD_JOBS=2 python3 scripts/run_theme_acceptance.py nextest run --release --locked \
  -p merman-render -p merman-theme-acceptance -p merman-bindings-core -p merman \
  --features merman-render/layout-elk,merman-render/math,merman-bindings-core/svg --lib \
  --test class_svg_test --test theme_support_discovery_test --test theme_authoring \
  --test route_cutover_runtime --test legacy_projection_retirement \
  --test block_title_legacy_projection --test class_edge_label_background_legacy_projection \
  --no-fail-fast
```

## Clean-checkout confirmation

The exact source checkout `/tmp/merman-class-text-00174dc2a` passed the same owner command:
3278/3278, with two existing skips. Git status was empty before and after the run. It reused
`CARGO_TARGET_DIR` from the main worktree; 1572 tracked Rust/manifests were timestamp-refreshed
to force rebuilding, and the log confirms compilation from the clean source path. The Class
SVG test executable contains `/private/tmp/merman-class-text-00174dc2a/crates/merman-render`.

Executable SHA-256: `8b5107376b3dd73e8ef6b9aca2034bc271fa92bc617e622a8ddd43ffbd68f29c`.

Log: `/tmp/class-text-node-order-clean-owner.log`.
Log SHA-256: `f3acf4725ab2476a3a7e9183b52ef0fcac35a57c016865161e3d8beb98019749`.
The source/status/binary observations are in `/tmp/class-text-node-order-clean-state.json`.
This clean confirmation covers the owner suite; SVG structure and Clippy were checked in the
main worktree against the identical committed source.

## Limits and continuation

The SVG assertions prove winner selection and suppression of older inline paint. CSS
presence does not qualify generic Text terminal paint. Complete Class Text migration still
needs node, namespace, relation/cardinality, note and diagram-title writer coverage with
source ownership and missing-terminal rejection. Web/Typst artifact size budgets,
remaining profile qualification and C7a candidate/contract freeze are separate open gates.
This slice does not rerun the full workspace, browser suite or every installed package.
