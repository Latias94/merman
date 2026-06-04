# HPD-050 - Architecture Final Node Edge Owner Diagnostics

Date: 2026-06-05
Task: HPD-050 Architecture-first layout engine audit

## Context

The child-group parent-input diagnostic clarified `stress_architecture_nested_groups_002`, but the
top residual rows still needed a sharper answer to a simpler question: which final node edges own
the X/Y span differences in the browser probe join?

This pass added a diagnostic-only owner table to `debug-architecture-delta`. It does not change
Architecture rendering, layout, root overrides, baselines, or final SVG output.

## Change

- Added a `Final node edge attribution` table to Architecture probe joins.
- The table compares browser final node `bb` edge owners with local final-frame service bboxes plus
  emitted group rects.
- It reports X/Y min owner, max owner, min/max deltas, and span deltas.
- The table is intentionally documented as boundary/frame evidence, not a root-width formula:
  nested group consumption, SVG text, and root padding can still add later root-bounds phases.

## Evidence

- `target/compare/architecture-delta-final-node-edge-owner-hpd050`
- `target/compare/architecture-report-parity-final-node-edge-owner-hpd050`
- `crates/xtask/src/cmd/debug/architecture.rs`

Commands:

- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_batch5_long_titles_and_punct_076 --fixture stress_architecture_html_titles_and_escapes_041 --fixture stress_architecture_unicode_and_xml_escapes_019 --fixture stress_architecture_batch6_init_fontsize_icon_size_wrap_093 --fixture stress_architecture_nested_groups_002 --probe-dir F:\SourceCodes\Rust\merman\target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050 --out target\compare\architecture-delta-final-node-edge-owner-hpd050`
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture-report-parity-final-node-edge-owner-hpd050`

## Findings

- Direct group-width rows are final group-edge owned:
  - `076`: `group-pipeline` X span delta `+5px`;
  - `041`: `group-ui` X span delta `+5px`;
  - `019`: `group-i` X span delta `+3px`.
- `093` is also final group-edge owned, but in the opposite direction:
  - `group-left` min delta `+44.463987`;
  - `group-right` max delta `+41.963987`;
  - X span delta `-2.5px`.
- `002` exposes a frame boundary rather than a direct root-width formula:
  - browser X min owner is `service-ingress`;
  - browser X max owner is `group-platform`;
  - the table's local/browser final-node X span delta is `+42.5px`, far larger than the SVG root
    width delta `+2.5px`.
  - This confirms that nested group rows still need render-path/source-frame evidence before a
    production change.

## Boundary

Do not use the new final-node owner table alone to tune nested group inset or root padding. It is
high-signal for direct final group-edge ownership (`076/041/019/093`) and a frame-mismatch sensor
for `002`.

## Verification

- `cargo fmt --check` - passed.
- `cargo nextest run -p xtask -E 'test(architecture_probe_join_reports_nested_group_aggregate_content) or test(architecture_probe_join_decomposes_group_and_service_bounds)'` - passed, `2` tests run.
- `cargo nextest run -p xtask` - passed, `112` tests run.
- `git diff --check` - passed.
- `cargo run -p xtask -- report-overrides --check-no-growth` - passed; Architecture root overrides remain at `0`.
- `cargo run -p xtask -- debug-architecture-delta ... --out target\compare\architecture-delta-final-node-edge-owner-hpd050` - passed for the top five post-strict rows.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture-report-parity-final-node-edge-owner-hpd050` - passed.
