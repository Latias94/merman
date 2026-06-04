# HPD-050 - Architecture Child Group Parent-Input Diagnostics

Date: 2026-06-05
Task: HPD-050 Architecture-first layout engine audit

## Context

The post-strict Architecture root queue classification split the current `20` mismatch rows into
separate residual families. `stress_architecture_nested_groups_002` was classified as a nested
aggregate / child-group phase row, but the existing `debug-architecture-delta` report only showed
the parent aggregate as a union of local emitted child group rects. That made it too easy to miss
the renderer phase that actually feeds parent groups: emitted child group rects are consumed after
the local `1px` child-group inset.

This pass tightened the diagnostic seam only. It did not change Architecture rendering, FCoSE
input, group padding, root overrides, or baselines.

## Change

- Added a `Child group parent-input phase` table to `debug-architecture-delta` probe joins.
- The table compares, per parent/child group:
  - browser child group final `node.boundingBox()`;
  - local emitted child group rect;
  - local parent-input rect after the renderer's `1px` child-group inset;
  - raw and parent-input `dx/dy/dw/dh` against browser.
- Added focused xtask test coverage for the nested group report path.

## Evidence

- `target/compare/architecture-delta-child-group-parent-input-hpd050`
- `target/compare/architecture-report-parity-child-group-parent-input-hpd050`
- `crates/xtask/src/cmd/debug/architecture.rs`

Commands:

- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_batch5_long_titles_and_punct_076 --fixture stress_architecture_html_titles_and_escapes_041 --fixture stress_architecture_unicode_and_xml_escapes_019 --fixture stress_architecture_batch6_init_fontsize_icon_size_wrap_093 --fixture stress_architecture_nested_groups_002 --probe-dir F:\SourceCodes\Rust\merman\target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050 --out target\compare\architecture-delta-child-group-parent-input-hpd050`
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture-report-parity-child-group-parent-input-hpd050`

## Findings

- The direct-width rows (`076` / `041` / `019`) and the source-shaped service row (`093`) have no
  child-group parent-input rows, so they remain outside this nested aggregate seam.
- In `stress_architecture_nested_groups_002`, the raw local child group widths are almost aligned
  with browser final child group bboxes:
  - `platform -> data`: raw `dw=-0.5`, `dh=0.0`;
  - `platform -> runtime`: raw `dw=0.0`, `dh=0.0`.
- The parent-input phase exposes the current compensation explicitly:
  - `platform -> data`: input `dw=-2.5`, `dh=-2.0`, `dx=45.25`, `dy=41.0`;
  - `platform -> runtime`: input `dw=-2.0`, `dh=-2.0`, `dx=42.25`, `dy=41.0`.
- The existing `Group aggregate child attribution` table still shows the raw emitted child group
  aggregate because that is useful for comparing final SVG child rects. The new table shows the
  renderer input actually used when computing a parent group.

## Boundary

This is evidence for a nested child-group / parent-input phase, not a production fix. Do not treat
`002` as another direct service label-width residual, and do not globally remove or retune the
child-group inset without a source-backed focused test plus full Architecture root verification.

## Verification

- `cargo fmt --check` - passed.
- `cargo nextest run -p xtask -E 'test(architecture_probe_join_reports_nested_group_aggregate_content) or test(architecture_probe_join_decomposes_group_and_service_bounds)'` - passed, `2` tests run.
- `cargo nextest run -p xtask` - passed, `112` tests run.
- `git diff --check` - passed.
- `cargo run -p xtask -- report-overrides --check-no-growth` - passed; Architecture root overrides remain at `0`.
- `cargo run -p xtask -- debug-architecture-delta ... --out target\compare\architecture-delta-child-group-parent-input-hpd050` - passed for the top five post-strict rows.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture-report-parity-child-group-parent-input-hpd050` - passed.
