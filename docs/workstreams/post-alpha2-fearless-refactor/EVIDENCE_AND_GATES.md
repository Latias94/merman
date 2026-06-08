# Post Alpha.2 Fearless Refactor — Evidence And Gates

Status: Active
Last updated: 2026-06-08

## Planned Gates

- `git diff --check -- docs/workstreams/post-alpha2-fearless-refactor`
- `cargo fmt --all --check`
- `cargo check -p merman --features render`
- `cargo nextest run -p merman --features render svg_pipeline_tests`
- `cargo nextest run -p merman --features render render_svg`
- `cargo check -p xtask`
- `cargo nextest run -p xtask admission`
- `cargo nextest run -p xtask compare_adapter_registry`
- `cargo nextest run -p xtask compare_invocation`
- `cargo nextest run -p xtask diagram_filter_key`
- `cargo nextest run -p xtask root_parity_policy`
- `cargo nextest run -p merman-bindings-core`
- `cargo nextest run -p merman-ffi render_svg`
- `cargo run -p xtask -- check-alignment`
- `cargo nextest run -p merman-render presentation_theme`
- `cargo nextest run -p merman-render timeline`

## Evidence Log

- 2026-06-08: Created this follow-on lane after `0.7.0-alpha.2` release. The prior `docs/workstreams/merman-0-7-architecture-deepening` lane remains closed and is referenced as history, not reopened.
- 2026-06-08: PA2R-020 moved binding render request construction and execution behind `RenderRequestPlan`. `cargo nextest run -p merman-bindings-core` passed 31/31 tests. `cargo nextest run -p merman-ffi render_svg` passed 2/2 focused tests. `cargo fmt --all --check` passed.
- 2026-06-08: PA2R-030 derived supported diagram metadata from render parser facts. `cargo nextest run -p merman-core registry` passed 10/10 focused tests. `cargo nextest run -p merman-core detect` passed 22/22 focused tests. `cargo run -p xtask -- check-alignment` passed. `cargo fmt --all --check` passed.
- 2026-06-08: PA2R-040 moved Timeline theme color, palette, disabled color, root color, and redux mode roles behind `PresentationTheme::timeline`. `timeline.rs` no longer walks raw `themeVariables` paths. `cargo nextest run -p merman-render presentation_theme` passed 10/10 focused tests. `cargo nextest run -p merman-render timeline` passed 9/9 focused tests. `cargo fmt --all --check` passed.
- 2026-06-08: PA2R-050 replaced the narrow private `HeadlessRenderOperation` helper with `HeadlessOperation`. The internal Module now owns semantic `layout_diagram` and typed-render SVG stages, while public free functions and `HeadlessRenderer` remain adapters. `cargo check -p merman --features render` passed. `cargo nextest run -p merman --features render svg_pipeline_tests` passed 9/9 focused tests. `cargo nextest run -p merman --features render render_svg` passed 7/7 focused tests. `cargo fmt --all --check` passed.
- 2026-06-08: PA2R-060 moved admission status combinations behind `DiagramAdmissionRecord` and status helper methods. Primary SVG, root-deferred, compare-command, defer-reason, and fixture evidence projections now read from the inventory Module instead of duplicating enum combinations in the alignment check. `cargo nextest run -p xtask admission` passed 4/4 focused tests. `cargo run -p xtask -- check-alignment` passed. `cargo check -p xtask` passed. `cargo fmt --all --check` passed.
- 2026-06-08: PA2R-070 moved `compare-all-svgs` diagram dispatch behind the per-diagram compare adapter registry. The all-diagram harness no longer carries a duplicate match over every primary SVG matrix family. `cargo nextest run -p xtask compare_adapter_registry` passed 2/2 focused tests. `cargo nextest run -p xtask diagram_filter_key` passed 1/1 focused test. `cargo run -p xtask -- compare-all-svgs --diagram info --filter upstream_info_spec --check-dom --dom-mode parity --dom-decimals 3` passed. `cargo check -p xtask` passed. `cargo fmt --all --check` passed.
- 2026-06-08: PA2R-080 moved `compare-all-svgs` common per-diagram command argument construction behind `CompareAllInvocationOptions`. The main loop no longer owns DOM args, mode-suffixed report path naming, flowchart-only text measurement, or optional root-report flag construction. `cargo nextest run -p xtask compare_invocation` passed 4/4 focused tests. `cargo nextest run -p xtask root_parity_policy` passed 4/4 focused tests. `cargo run -p xtask -- compare-all-svgs --diagram info --filter upstream_info_spec --check-dom --dom-mode parity --dom-decimals 3` passed. `cargo check -p xtask` passed. `cargo fmt --all --check` passed.
- 2026-06-08: PA2R-090 moved fixture-specific root parity residual acceptance out of `compare/all.rs` and into `compare/root_parity.rs`. `compare-all-svgs` now asks `RootParityResidualPolicy` to accept or summarize failures instead of owning acceptance fragments, remaining mismatch summaries, and missing-residual failures. `cargo nextest run -p xtask root_parity_policy` passed 4/4 focused tests. `cargo nextest run -p xtask compare_invocation` passed 4/4 focused tests. `cargo run -p xtask -- compare-all-svgs --diagram info --filter upstream_info_spec --check-dom --dom-mode parity --dom-decimals 3` passed. `cargo check -p xtask` passed. `cargo fmt --all --check` passed.
