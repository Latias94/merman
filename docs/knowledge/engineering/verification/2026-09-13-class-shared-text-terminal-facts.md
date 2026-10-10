# Class shared text terminal facts

Date: 2026-09-13

Source commit: `c16be4559c73a4e0e7a26825d3baf04bcd8b5867`.

Class now uses one `ClassTextThemePlan`, `ClassTextThemeReceipt` and
`ClassTextTerminalFacts` for its existing text-bearing terminal inventory. The internal
module moved from `class/theme/typography.rs` to `class/theme/text.rs`; no public API changed.

The receipt preserves inherited-color run counts and unknown color ownership alongside
independent font-family and font-size counts. NodeLabel paint decisions and namespace Title
consume those same facts. Namespace inheritance requires nonempty visible text, every run
inheriting color, and no unknown owner; source-owned but unambiguous color does not qualify.
The separate namespace inheritance boolean is gone. Existing CSS font evidence remains
property-specific, and no second terminal inventory or SVG parser was introduced.

This is a prerequisite for the remaining Class Text cutover. Receipt activation still follows
existing typography requests. Generic Text paint has not acquired full terminal coverage;
its four legacy routes remain, and unsupported Text ordinal accounting remains incomplete.
No support revision, authorization route, qualification cell or C7a gate changed.

## Verification

Three new unit tests cover color/font independence, aggregation across node/note/namespace,
edge label/cardinalities and diagram title, and the distinction between empty, inherited,
source-owned and unknown/math text. The existing projection test now verifies preservation
of color as well as font counts. Existing public NodeLabel source-ownership, mixed text,
namespace math and strict-rejection cases remain covered.

The final main-worktree private Release suite passed **3281/3281**, with two existing skips.
The earlier 3280-pass run preceded the final explicit inheritance predicate and is not the
final source evidence. Final log: `/tmp/class-text-terminal-facts-owner-final.log`.

```text
CARGO_BUILD_JOBS=2 python3 scripts/run_theme_acceptance.py nextest run --release --locked \
  -p merman-render -p merman-theme-acceptance -p merman-bindings-core -p merman \
  --features merman-render/layout-elk,merman-render/math,merman-bindings-core/svg --lib \
  --test class_svg_test --test theme_support_discovery_test --test theme_authoring \
  --test route_cutover_runtime --test legacy_projection_retirement \
  --test block_title_legacy_projection --test class_edge_label_background_legacy_projection \
  --no-fail-fast
```

Class SVG structure comparison passed, retaining the existing reviewed font-row segmentation
policy and the exact browser-layout residual for
`stress_class_svg_font_size_px_string_precedence_026`. The existing two upstream fixture
exclusions remain. No new comparator waiver was added. Logs/report:
`/tmp/class-text-terminal-facts-structure.log` and
`/tmp/class-text-terminal-facts-structure-report.md`.

Renderer Clippy passed with existing unrelated warnings; workspace formatting and diff checks
passed. Log: `/tmp/class-text-terminal-facts-clippy.log`. Standards and Spec reviews found no
actionable issues after the explicit inherited-paint predicate and empty/source/math tests
were added. Reviews were read-only and do not substitute for these executable checks.

## Clean-checkout confirmation

`/tmp/merman-class-text-facts` checked out the exact source above and passed the same owner
suite: **3281 passed, two existing skips**. Git status was empty before and after. Dependencies
used the shared `CARGO_TARGET_DIR`; 1572 tracked Rust/manifests were timestamp-refreshed to
force compilation from the clean source. The test executable contains the clean renderer path.

Log: `/tmp/class-text-terminal-facts-clean-owner.log`.
Log SHA-256: `b79769a962765095a2af8b3ff2e9175fe5c01a3a318c31d1784cdf2d2d9f4c4e`.

Class test executable SHA-256: `bf9f929c13d6477f37801aed462c99e344fc097542f9a18f97919185d5dbd03a`.
The executable was preserved at `/tmp/class-text-terminal-facts-clean-class-svg-test` before
returning the shared target to main-worktree compilation. Source, status and binary observations:
`/tmp/class-text-terminal-facts-clean-state.json`.

The clean run covers the owner suite; structure/Clippy checks used the main worktree at the
identical source. This does not claim full-workspace, complete browser-suite, installed-package
or artifact-profile qualification. The next all-channel Text cutover test was added afterward
and is not part of this committed source or its passing count.
