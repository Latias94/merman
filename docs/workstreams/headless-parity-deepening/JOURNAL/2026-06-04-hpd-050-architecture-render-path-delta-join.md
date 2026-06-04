# HPD-050 - Architecture Render-Path Delta Join

Date: 2026-06-04
Task: HPD-050 Architecture-first layout engine audit

## Context

`stress_architecture_junction_fork_join_026` remains the largest Architecture root residual after
HPD-090 closeout. Earlier HPD-050 work established that the manual ArchitectureDB/FCoSE probe is
diagnostic-only for this fixture, while the actual installed Mermaid `mermaid.render(...)` path
reproduces the stored upstream SVG exactly. The local delta report still needed a way to put that
authoritative render-path probe beside the local SVG and FCoSE compound evidence.

## Change

- Added optional `--render-probe-dir` to `xtask debug-architecture-delta`.
- The delta batch report now has separate `probe json` and `render-path probe json` columns.
- Per-fixture delta reports now include `Render-path probe join` when a matching
  `<fixture>.render-path-probe.json` is present.
- The new join records render-path root facts, SVG group facts, SVG service facts, and group stage
  `bb` values against local emitted output and local FCoSE compound rectangles.
- No production renderer, layout formula, SVG fixture, or baseline changed.

## Focused Finding

The focused junction report was written to:

- `target\compare\architecture-delta-render-path-join-hpd050\stress_architecture_junction_fork_join_026.md`

It confirms:

- render-path `renderedFacts` still match `storedFacts`;
- stored max-width is `2808.126709`;
- local max-width is `2822.102295`;
- local root delta is `+13.975586px`;
- local emitted `left` group delta is `dx=-6.954918`, `dy=+6.250922`, `dw=+17.331122`,
  `dh=-18.609285`;
- local emitted `right` group delta is `dx=+10.376204`, `dy=-12.358363`, `dw=-3.388269`,
  `dh=+6.107441`.

The render-path stage table narrows the source boundary:

- local FCoSE compounds are close to render-path `layoutstop-run1-before-segments` group `bb`
  shapes: `dx=+3.25`, `dy=+11`, `dw=-5`, `dh=-22` for both groups;
- local FCoSE compounds diverge from the post-rerun `draw-after-layout-before-svg-emission` group
  `bb` state that the stored SVG consumes.

## Verification

- `cargo fmt --check -p xtask` - passed.
- `cargo nextest run -p xtask` - passed, `106` tests run.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_junction_fork_join_026 --probe-dir target\compare\architecture-fcose-probe-active-residuals-hpd050 --render-probe-dir target\compare\architecture-render-path-probe-xtask-hpd050 --out target\compare\architecture-delta-render-path-join-hpd050` -
  passed.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture_report_parity_after_render_path_join_hpd050.md` -
  passed.

## Residual Boundary

This is evidence tooling only. It does not justify root-bounds tuning, group padding changes, or
final SVG group rect rewrites. The next source-backed junction slice should compare local `manatee`
against the bundled `cytoscape-fcose@2.2.0` / `cose-base@2.2.0` internal phases from the actual
render path.
