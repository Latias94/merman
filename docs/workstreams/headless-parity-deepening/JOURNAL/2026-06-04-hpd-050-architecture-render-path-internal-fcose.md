# HPD-050 - Architecture Render-Path Internal FCoSE Phases

Date: 2026-06-04
Task: HPD-050 Architecture-first layout engine audit

## Context

`stress_architecture_junction_fork_join_026` was narrowed to an actual render-path
`cytoscape-fcose@2.2.0` / `cose-base@2.2.0` internal phase residual. The previous
`debug-architecture-delta --render-probe-dir` report showed local FCoSE compounds close to the
render-path first layoutstop shape, but it still compared local layout-base rectangles to
Cytoscape `node.boundingBox()` output rather than to bundled FCoSE's own internal node rectangles.

## Change

- Extended `tools/debug/arch_render_path_probe_fixture.js` so the actual `mermaid.render(...)`
  probe also captures bundled nested FCoSE/Cose internal phases in `probe.fcoseStages`.
- The in-memory Mermaid IIFE patch now records:
  - `coseLayout.start`
  - `coseLayout.after-process-children`
  - `coseLayout.after-process-edges-constraints`
  - `classicLayout.start`
  - `initConstraintVariables.start`
  - first tick start / after move
  - `classicLayout.end`
  - `coseLayout.after-runLayout`
  - `relocateComponent.before-shift`
- Extended the render-path probe Markdown summary with bundled FCoSE/Cose stage and compound-rect
  tables.
- Extended `debug-architecture-delta --render-probe-dir` to compare bundled FCoSE/Cose internal
  group rects with local FCoSE compound rectangles.
- No production renderer, layout formula, SVG fixture, or baseline changed.

## Focused Finding

The focused junction report was written to:

- `target\compare\architecture-delta-render-path-internal-join-hpd050\stress_architecture_junction_fork_join_026.md`

It confirms a sharper phase boundary:

- render-path `renderedFacts` still match `storedFacts`;
- the probe captures `22` bundled internal FCoSE/Cose stages and `0` probe errors;
- local FCoSE compound widths/heights match bundled run `0` `classicLayout.end` /
  `coseLayout.after-runLayout` exactly:
  - `left`: `dw=0`, `dh=0`
  - `right`: `dw=0`, `dh=0`
- bundled run `1` `classicLayout.end` / `coseLayout.after-runLayout` diverges by the same group
  width/height deltas seen in the local SVG residual:
  - `left`: `dw=+17.331122`, `dh=-18.609285`
  - `right`: `dw=-3.388269`, `dh=+6.107441`

The residual therefore points at the missing or mismatched second FCoSE rerun phase after segment
edge adjustment, not at root bounds, group padding, final group rect emission, or stale upstream SVG
baselines.

## Verification

- `node --check tools\debug\arch_render_path_probe_fixture.js` - passed.
- `cargo fmt -p xtask` - passed.
- `cargo nextest run -p xtask render_path_probe_markdown_summarizes_facts_and_stages architecture_render_path_join_reports_local_deltas` -
  passed, `2` tests run.
- `cargo run -p xtask -- debug-architecture-render-path-probe --fixture stress_architecture_junction_fork_join_026 --browser-exe "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe" --out target\compare\architecture-render-path-internal-probe-hpd050` -
  passed.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_junction_fork_join_026 --probe-dir target\compare\architecture-fcose-probe-active-residuals-hpd050 --render-probe-dir target\compare\architecture-render-path-internal-probe-hpd050 --out target\compare\architecture-delta-render-path-internal-join-hpd050` -
  passed.
- `cargo fmt --check -p xtask` - passed.
- JSONL validation for `docs\workstreams\headless-parity-deepening\CONTEXT.jsonl` - passed.
- `git diff --check` - passed; Git reported only the existing CRLF normalization warning for
  `CONTEXT.jsonl`.
- `cargo nextest run -p xtask` - passed, `106` tests run.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture_report_parity_after_internal_fcose_probe_hpd050.md` -
  passed.

## Residual Boundary

This remains evidence tooling only. The next production-capable junction slice should compare local
`manatee` run sequencing and segment-edge rerun behavior against bundled `cytoscape-fcose` run `1`.
Do not tune Architecture root width, group padding, or emitted group rectangles from this evidence.
