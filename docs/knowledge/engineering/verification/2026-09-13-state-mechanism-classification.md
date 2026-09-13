---
type: Verification Evidence
title: State mechanism classification from terminal consumers
timestamp: 2026-09-13
related_plan: docs/plans/2026-08-09-001-portable-theme-architecture-convergence-addendum.md
git_branch: refactor/presentation-theme-model
tags: theme,state,classification,verification
---

# C5 finding and change

The current-source C5 audit started at `91656b072`. State's family program classified every
rule facet, palette target, and effect binding as `TypedAdapter`. A separate
`StateStaticThemeSupport` table supplied narrower facts for checking the independent public
support manifest. The terminal adapter already rejected unsupported requests; the finding was inconsistent static
classification authority, not a newly reproduced portable-admission or rendering failure.

State now derives rule, palette, and binding classifications in `family_mechanism_matrix.rs`.
The separate table and enum in `state.rs` are removed. Generic Text still reaches State labels.
Simple typography belongs to text surfaces, geometry belongs to State/Composite/Note surfaces,
and paint requires a scalar solid or transparent value on a supported text or shape surface.
Clear and non-scalar paint, shape typography, and other mechanisms with no consumer are
`Unsupported`. Palette routes are restricted to the nine targets that call `series_color`.

Effect binding and rule effect have different domains. Only State can generate a bound effect
graph. Rules also retain the existing Clear consumer on Composite, Note, and SpecialState:
`consume_effect` suppresses their binding before checking graph/surface support. Their non-clear
graphs still produce runtime residuals. The matrix does not encode Clear/value for non-paint
facets; the terminal adapter continues to own value- and surface-dependent admission.

The summary used to validate the independent public support manifest now derives from the same
matrix, retaining the conservative partial State value domain. Its effect summary intersects
rule and binding support, so the internal Clear
routes do not introduce new positive effect claims. Public support revision 85 and the shared
fourteen-case golden are unchanged. KTD17 v83 (466 routes / 694 native witnesses), KTD23 v9,
and the 40 remaining legacy routes (Block 32 / Class 8) are unchanged.

# Evidence

- The initial focused baseline passed 711/711 before edits.
- Two new matrix regressions failed against the old implementation: State Clear fill and Title
  palette returned `TypedAdapter` instead of `Unsupported`. The final tests also cover target,
  typography, geometry, effect, and selector boundaries.
- Independent source review identified the effect-Clear exception before it could be removed.
  The final SVG test renders Note, Composite, and SpecialState using binding+Clear under strict
  portability. Each output is byte-for-byte equal to an empty-theme render of the same source;
  evidence requires two accounted mechanisms, one Applied Clear, one NotApplicable binding,
  and zero residuals. Thus silently ignoring both requests cannot satisfy the test.
- The initial SVG oracle incorrectly rejected State's two default shadow filter definitions.
  The complete-output comparison preserves those definitions while checking that binding+Clear
  changes nothing. It replaces the incorrect assertion that no filter definition may exist.
- The final owner suite passed **882/882** on macOS ARM64 in private Release mode with ELK.
  It includes core catalog and detector/config-layer tests, binding catalog projection, bounded
  program/provenance tests, public support-manifest reconciliation, primary-family SVG/terminal
  cases, C6 runtime, and the existing route/retirement acceptance checks. The first 799-test run
  did not select core `catalog_tests`; the final filter explicitly includes that module and all
  core detector tests. Counts here describe executed tests, not full workspace coverage.
- The full SVG structure gate passed. Formatting and `git diff --check` passed.
- Correctness and simplicity reviews found no remaining blocking issues. The mixed Clear/Radius
  regression compares semantic facets rather than relying on route enumeration order.

The exact final owner command was:

```text
CARGO_BUILD_JOBS=2 python3 scripts/run_theme_acceptance.py nextest run --release --locked -p merman-core -p merman-render -p merman-bindings-core -p merman-theme-acceptance --features merman-render/layout-elk --lib --test diagram_theme_test --test theme_resolution_svg_test --test typed_family_api_test --test theme_support_discovery_test --test flowchart_cluster_theme_test --test swimlane_cluster_theme_test --test state_svg_test --test sequence_svg_test --test c6_runtime --test route_cutover_runtime --test legacy_projection_retirement --test block_title_legacy_projection --test class_edge_label_background_legacy_projection -E '(package(merman-core) & (test(family::catalog_tests) | test(tests::detect))) | (package(merman-bindings-core) & (test(diagram_family_capabilities) | test(theme_catalog))) | (package(merman-render) & (test(diagram_theme::) | test(flowchart::theme_evidence::) | test(state::theme_evidence::) | test(state::style_plan::) | test(sequence::theme_evidence::) | test(sequence::typography::) | binary(diagram_theme_test) | binary(theme_resolution_svg_test) | binary(typed_family_api_test) | binary(theme_support_discovery_test) | binary(flowchart_cluster_theme_test) | binary(swimlane_cluster_theme_test) | binary(state_svg_test) | binary(sequence_svg_test))) | package(merman-theme-acceptance)' --test-threads 2 --no-fail-fast
```

The mandatory default-output check was:

```text
CARGO_BUILD_JOBS=2 cargo run --locked --release -p xtask -- compare-all-svgs \
  --check-dom --dom-mode structure --dom-decimals 3 --diagnostic-browser-text-layout
cargo fmt --all --check
git diff --check
```

Raw logs are `/tmp/theme-c5-current-source-acceptance.log` (baseline),
`/tmp/theme-c5-state-classification-red.log` (matrix regressions),
`/tmp/theme-c5-state-classification-green.log` (first SVG oracle),
`/tmp/theme-c5-state-classification-final.log` (799/799),
`/tmp/theme-c5-complete-owner-scope.log` (882/882), and
`/tmp/theme-c5-state-full-structure.log` (full structure gate).
`/tmp/theme-c5-final-command.json` retains the final command arguments for clean-checkout replay.

# Clean-checkout confirmation

The committed implementation at `4fc8f4532a8eb2bd607de78937fb256ae75a6125` was checked out at
`/tmp/merman-state-classification-4fc8f4532`. The same final owner command passed **882/882**;
`git status --porcelain` was empty before and after the run.

With approximately 5.5 GiB free disk space, this same-source checkout reused the active worktree's
`target` through `CARGO_TARGET_DIR`. The new checkout timestamps forced recompilation; the log
identifies core, renderer, binding core, and acceptance sources under the clean checkout.
The resulting `state_svg_test-f66e607ba44a9308` binary has SHA-256
`ceb76e91b54f28a0e33840d14febf5da64710ec631e8aa2c151116b48ea383aa`. It contains eight clean-checkout core path references
and zero references to the stale pre-Class baseline that affected an earlier verification.
This is a clean-source replay, not a cold build with an empty dependency cache.

The full SVG structure comparison was run on the same implementation before commit in the active
worktree; it was not repeated in the clean checkout. Logs and provenance are
`/tmp/theme-c5-4fc8f4532-clean.log`, `/tmp/theme-c5-4fc8f4532-clean-state.json`, and
`/tmp/theme-c5-4fc8f4532-clean-linkage.json`.

# Remaining scope

This slice closes State's duplicate static classification. The complete C5 requirement-by-
requirement closure audit, C7a artifact-bound qualification/public rollout, contract freeze, and
C7b bridge deletion remain open. No native consumer package was rebuilt in this slice, no package
was published, and the full workspace/browser/platform suites were not run. Earlier installed
consumer records remain evidence for their explicitly named source revisions.
