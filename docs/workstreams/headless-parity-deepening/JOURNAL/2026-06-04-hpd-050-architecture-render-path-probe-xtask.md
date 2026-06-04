# HPD-050 - Architecture Render-Path Probe Xtask Wrapper

Date: 2026-06-04
Task: HPD-050 Architecture-first layout engine audit

## Context

The previous render-path probe proved that `stress_architecture_junction_fork_join_026` reproduces
the stored upstream SVG through the actual installed Mermaid `mermaid.render(...)` Architecture
path. That made the older manual ArchitectureDB/FCoSE probe diagnostic-only, but the new source of
truth still required hand-running a Node script and manually inspecting JSON.

## Change

Added `xtask debug-architecture-render-path-probe`.

The command wraps `tools/debug/arch_render_path_probe_fixture.js` without changing the probe's
browser instrumentation. It adds the same operational affordances as the existing manual FCoSE
probe wrapper:

- repeated `--fixture` filters
- `--out` / `--out-dir`
- optional `--browser-exe`
- stable `<fixture>.render-path-probe.json`
- stable `<fixture>.render-path-probe.md`
- `architecture-render-path-probe-batch.md` for multi-fixture runs

The Markdown summary records the bundled Mermaid/Cytoscape/FCoSE versions, rendered-vs-stored root
facts, SVG group rectangles, SVG service positions, captured graph bboxes per render stage, and
group bounds by stage.

## Findings

The focused wrapper run for `stress_architecture_junction_fork_join_026` wrote:

- `target\compare\architecture-render-path-probe-xtask-hpd050\stress_architecture_junction_fork_join_026.render-path-probe.json`
- `target\compare\architecture-render-path-probe-xtask-hpd050\stress_architecture_junction_fork_join_026.render-path-probe.md`

It reported:

- `facts match: true`
- `6` captured stages
- `2` SVG groups
- `5` SVG services

The summary confirms the same render-path authority as the earlier hand-run JSON:

- viewBox matches stored exactly:
  `-1362.063232421875 -1213.2674560546875 2808.126708984375 2557.534912109375`
- max-width matches stored exactly: `2808.126708984375`
- the `draw-after-layout-before-svg-emission` stage is the post-rerun state and matches final
  stored group rectangles.

## Outcome

No renderer behavior, layout formula, SVG fixture, or baseline changed.

Future `junction_fork_join_026` evidence should use the xtask wrapper before any source-backed
claim about the actual Mermaid render path. If the row needs deeper investigation, the next step is
a bundled `cytoscape-fcose` / `cose-base` internal-phase harness, not a renderer/root-bounds tune
against the older manual probe.

## Verification

- `cargo fmt -p xtask`
- `cargo nextest run -p xtask render_path_probe fcose_probe_args_accept_out_dir_aliases fcose_probe_batch_markdown_links_per_fixture_artifacts`
- `cargo run -p xtask -- debug-architecture-render-path-probe --fixture stress_architecture_junction_fork_join_026 --browser-exe "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe" --out target\compare\architecture-render-path-probe-xtask-hpd050`
