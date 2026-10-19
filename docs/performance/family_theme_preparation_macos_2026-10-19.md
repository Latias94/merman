# Family Theme Preparation Ownership (macOS, 2026-10-19)

This checkpoint removes theme preparation that was immediately discarded or copied only to
validate a synchronous write. It records allocation/work evidence, not a latency claim.

## Changes

- Sankey's successful CSS emission receipt borrows three immutable prepared strings until its
  synchronous validation completes, avoiding three temporary owned copies.
- Unthemed Quadrant Chart no longer creates one empty expectation per point. Terminal point
  bindings and their work-meter charge remain intact; source and class style precedence is
  unchanged.
- Pie CSS reuses the inherited font already resolved for the diagram. Root CSS still resolves its
  separately configured font when typography was not requested.
- Class moves configured font-size preparation to its fallback branch, reuses the already
  normalized font, and moves owned normalized font data into common CSS without another clone.
  Typed typography and configured layout-font behavior remain unchanged.

## Evidence

The preregistered run is in the ignored directory
`target/bench/experiments/family-theme-preparation-20261019/`. It uses the frozen release library
build, complete public `Renderer.render` SVG operations, five observations per allocation row,
178 custom/configured controls, and the existing 38-scene catalog across 35 logical families and
10 presets plus default (418 cells). Candidate 1 was rejected because ER gained one allocation
and 37/40 bytes; candidate 2 corrected the owned-string transfer before admission.

All 178 controls preserve SVG, report, and ordered measurement traces exactly. Of 171 jointly
stable memory rows, none increases allocation count, allocated bytes, peak growth, or retained
growth; seven variable Flowchart rows remain inconclusive. Confirmed reductions include Class
(-2 allocations/-128 bytes), Pie (-2/-106), Sankey (-3/-47), and Quadrant Chart (-1/-192).

All 418 catalog cells are stable and preserve SVG, reports, effective theme metadata, and outcomes.
Twenty-four cells reduce allocations and allocated bytes: Class saves 2/128 bytes in default and
5/215 bytes in each preset; Sankey saves 3/47 bytes in default and 3/57 bytes in each preset; Pie
default saves 2/106 bytes; Quadrant Chart default saves 1/192 bytes. No cell increases any of the
four memory metrics. Retained growth is unchanged throughout.

No calibrated latency test was run for this candidate. Stage timing remains attribution only;
latency admission requires calibrated A/A and fixed AB/BA. This batch is not evidence that the
remaining v0.9.0 E2E gaps have closed.

Verification: release all-feature library build; 5,200 renderer tests passed and five were
skipped; renderer Clippy with warnings denied; format and diff checks. Strict verification passed
format, workspace check/Clippy, all 661 feature-matrix builds, and 36 artifact checks, then stopped
while materializing the Mermaid reference bundle because
`tools/mermaid-cli/node_modules/esbuild/package.json` is absent locally. The Rust matrix completed;
the final reference-bundle gate remains unverified in this environment.
