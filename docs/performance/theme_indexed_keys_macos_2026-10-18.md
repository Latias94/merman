# Theme Indexed-Key Allocation Checkpoint

This checkpoint removes repeated `format!` calls for fixed indexed Mermaid theme-variable names
on non-exact theme materialization paths. The change keeps the existing loops, insertion order,
explicit-value checks, color operations and ownership propagation; a static key table is borrowed
for lookups and converted to an owned `String` only when a JSON map insertion actually needs it.

The comparison uses base revision `dd4c1bbdafb079cb081189c2e6e260dfdca57bd9` on macOS 27.0,
Apple M4 Pro, arm64, rustc 1.95.0. The public `Renderer::render` probe puts a root `theme` and
`themeVariables` object in each diagram's `init` directive, so every sample exercises the
per-operation `Materialize` path. It covers the default theme and all six Extended themes with
empty, unrelated, scale and Git overrides, plus palette-array cases for Redux Color themes.
Each row has five allocation observations after a warm-up render.

All 30 non-empty, non-palette rows reduce allocations and allocated bytes. The default theme saves
134 allocations and 2,233 bytes per render. Other themes save 15–55 allocations and 137–1,024
bytes. Empty exact-snapshot controls and explicit palette controls are unchanged. The 60 SVG and
theme-report artifacts (30 rows, two artifacts each) compare byte-for-byte equal between base and
candidate; the focused theme suite passes 139/139 and the public `merman` suite passes 396/396
with two configured skips. Clippy, formatting and the all-feature release build pass.

This is an allocation/work reduction only. No calibrated latency claim is made; the change has no
retained cache or new state. Raw allocation metrics and SVG/report artifacts are retained under
`target/bench/experiments/theme-indexed-keys-20261018/` for the local audit.
