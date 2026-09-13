---
type: Verification Evidence
title: Class edge-label background projection retirement
timestamp: 2026-09-13
related_plan: docs/plans/2026-08-09-001-portable-theme-architecture-convergence-addendum.md
git_branch: refactor/presentation-theme-model
tags: theme,class,retirement,verification
---

# Boundary

KTD23 v9 removes only Class's static unqualified/Default `EdgeLabelBackground.fill`
projection into `themeVariables.edgeLabelBackground`. The independent pre-removal checkout is
`9babbfa248ea505053d24d8314b09d8e25a529b6`; see the
[baseline record](2026-09-13-class-background-baseline.md). Historical SVG witnesses and all
previous retirement batches remain unchanged. Two historical identities correspond to four
solid/transparent matrix routes. The remaining executable inventory is 58: Block 36, Class 22.
The cumulative retirement authority contains 80 identities and 160 value probes.

The unused CSS selectors remain for Mermaid structural parity. Class no longer projects the
requested color into them. Block's assignment remains executable. This does not introduce a typed
background writer: public support revision 82 reports this Class surface as Unsupported, with a
new shared support golden case.

# Runtime evidence

The completed label writer reports whether it emitted a background with positive dimensions,
using the same decoded text as rendering. Background receipt events are allocated only when a
background mechanism is requested. Missing, duplicate, extra, or out-of-order checkpoints cannot
complete the background domain. Receipt work accounts for the additional events.

Visible winning unsupported requests remain residuals. Absent labels, ordinal misses, shadowed
rules, and source-owned fills become NotApplicable only after completed terminal reconciliation.
Source fill ownership does not hide unsupported sibling stroke facets. Dagre and ELK use the same
receipt path. Relation IDs use a one-based DOM index; receipt recording converts to the zero-based
semantic index.

The native post-removal test requires exact SVG byte equality and decoded PNG dimension/RGBA
equality with the unthemed scene. It covers 120 requested-paint comparisons across five schemes,
three looks, both input HTML-label modes, both selectors, and solid/transparent paint. Node and
NodeLabel controls must still alter visible pixels in each scene. The former normalization helper
is removed; the independent pre-removal commit retains it for historical comparison.

# Verification

On macOS ARM64, with Cargo builds serialized and `CARGO_BUILD_JOBS=2`:

- Renderer Class, bridge/retirement, and support discovery checks: 276/276 passed, including
  both layout adapters and the source-owned fill/unsupported stroke negative.
- Private Release theme acceptance: 144/144 passed, zero skipped. Nextest reported two leaky
  test processes (`paint_route_svg_admission_allows_only_the_exact_font_seal_residual` and
  `gantt_warning_today_and_vertical_have_independent_visible_png_evidence`); assertions passed
  and the runner exited successfully. This is not a memory-leak measurement.
- Full SVG structure comparison: successful exit across all 35 comparator sections.
- Native C ABI, UniFFI, and Typst shared support goldens plus binding-core authoring tests:
  15/15 passed. These are compiled Rust transport tests, not installed-package witnesses.
- CI/release workflow guards: 33/33 passed, including explicit Block/Class PNG witness selection.
- Renderer Clippy, formatting, and whitespace checks passed. Clippy still reports existing
  warnings, including the Class label helper argument-count warning (now nine arguments).

The full acceptance run initially exposed a Flowchart mutation-test mismatch: it renamed the
chart title while expecting a cluster-title error. Commit `af28833a3` fixes the mutation and also
requires the actual cluster-title group and edge-label parent owner before qualification. This
is a separate acceptance repair, not a Class rendering change. The results above are the
subsequent successful run.

```text
CARGO_BUILD_JOBS=2 cargo nextest run --locked -p merman-render --features layout-elk,layout-cytoscape --lib --test class_svg_test --test theme_support_discovery_test -E 'test(class) | test(support_manifest) | test(legacy_compatibility) | test(legacy_projection_retirement) | test(legacy_tombstones) | binary(theme_support_discovery_test)'
CARGO_BUILD_JOBS=2 python3 scripts/run_theme_acceptance.py nextest run --release --locked -p merman-theme-acceptance --no-fail-fast
CARGO_BUILD_JOBS=2 cargo run --locked --release -p xtask -- compare-all-svgs --check-dom --dom-mode structure --dom-decimals 3 --diagnostic-browser-text-layout
CARGO_BUILD_JOBS=2 cargo nextest run --locked -p merman-bindings-core -p merman-typst-plugin -p merman-ffi -p merman-uniffi --features svg --lib -E 'test(theme_support_matches_shared_golden) | test(theme_definition)'
python3 scripts/test_release_workflow_security.py
CARGO_BUILD_JOBS=2 cargo clippy --locked -p merman-render --lib --features layout-elk,layout-cytoscape --no-deps
cargo fmt --all -- --check
git diff --check
```

Logs are `/tmp/class-background-render-final.log`, `/tmp/class-background-acceptance-all.log`,
`/tmp/class-background-structure.log`, `/tmp/class-background-transports.log`, and
`/tmp/class-background-workflows-final.log`.

# Clean-checkout confirmation

A detached checkout of `f1046a9cfa5fa505ac4082c92f3277ecf0fcde97` at
`/tmp/merman-class-retired-f1046a9cf` passed 21/21 focused Release acceptance tests and 33/33
workflow guards. `git status --short` was empty before and after both checks. The 21 tests include
KTD23 authorization and its negative cases, Block title/cluster-label retirement, Class background
SVG/PNG comparison, and Flowchart qualification mutation/pixel checks. This targeted replay does
not repeat or replace the full 144-test run recorded above.

Cargo reused the primary worktree's target directory with two build jobs. Both this checkout and
the independent pre-removal checkout remain available for comparison.

```text
CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/Users/frankorz/Documents/projects/rust/merman/.worktrees/presentation-theme-model/target python3 scripts/run_theme_acceptance.py nextest run --release --locked -p merman-theme-acceptance --lib --test legacy_projection_retirement --test class_edge_label_background_legacy_projection --test block_title_legacy_projection -E 'test(route_retirement_manifest) | test(preset_qualification::flowchart_proof) | binary(legacy_projection_retirement) | binary(class_edge_label_background_legacy_projection) | binary(block_title_legacy_projection)'
python3 scripts/test_release_workflow_security.py
```

Logs: `/tmp/class-background-clean-final.log` and
`/tmp/class-background-clean-workflows-final.log`.

# Delivery limits

These checks concern the retired Class projection and its public support response. They do not
close C5, C7a public rollout, final artifact/profile qualification, or contract freeze. Installed
Web, Node, Python, and WASM packages require fresh builds to exercise support revision 82; older
revision-81 package evidence does not cover the changed query. Full workspace and browser suites
remain owned by the repository CI lanes.
