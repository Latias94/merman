# Theme branch main resync — October 20, 2026

This checkpoint records a second normal merge into the theme performance branch, from
`d1a300b980c5568b4284b8e8a98622ee43f829ef` and upstream
`daa7c812d`, committed as `6badd5444e44bacef807d086a0e3e09eee32e89e`. The upstream change adds prepared redraw, persistent mounting, borrowed family
models and public parse-metadata projection. It retains its upstream API and resource contracts.

The sole textual conflict is the Radar series-paint call in family preparation. The merged call
uses the performance branch's `RadarSeriesPaintPlan::resolve` and upstream's borrowed `model`.
It does not reintroduce the discarded CSS-binding parameter or clone the model. The automatic
config, parse-pipeline and Kanban merges retain both sides' behavior; independent source review finds no blocking interaction.

## Validation

The validation freezes indexed tree `e1707685fb8c9f587d0139899a5c5bc8cc148bf0` before execution.
Commands run serially against the existing target directory; full release nextest uses four
compile jobs and test threads, with one Cargo job for other native stages except the separate
acceptance test build. There is no new performance sampling. Full source rendering and prepared redraw retain separate
operation identities; this merge does not substitute one for the other in historical evidence.

| Check | Result |
| --- | --- |
| All-features release nextest: core, facade, renderer and bindings-core | 8100 pass, seven skipped |
| Matching strict release Clippy | Pass with `-D warnings` |
| Full DOM matrix, original structure/parity/parity-root policies | 37 entries pass |
| Optimized full WASM, including wasm-opt | Pass; full package transaction published |
| Release theme acceptance, PNG/all-diagrams/Cytoscape | 34 pass |
| Format, staged/unstaged diff, indexed path length | Pass; 160-unit indexed path budget |
| Four Python performance owner suites | 245 pass (16 + 51 + 29 + 149) |

All 13 serial command stages reach terminal success. The final source-hash check matches all
19,448 frozen non-documentation files; the index still equals the tested tree. The 37 fresh
parity-root report files are archived in the experiment directory with digests. WASM build
inputs and seven published package files have recorded digests; its input digest is
`55cd3263572d415537287896d66a756f5573e19fb06ca71e3af20c23bfa220de`.

Exact commands, source hashes, logs and terminal receipts remain in the ignored
`target/bench/experiments/theme-main-resync-20261020/` directory. Platform consumers, Linux
workspace coverage and browser containment keep their existing CI owners. Build completion does
not claim browser runtime smoke or cross-platform qualification.

## Performance identity

All adjacent allocation reports and the three formal release confirmations retain their original
frozen revisions. In particular, the latest v0.9.0 result remains at `a6742779c`, with 15 confirmed
non-regressions, three regressions and one inconclusive result across 19 same-SVG scenes. It does
not measure this merge, prepared redraw or persistent mounting. The earlier
[main-integration checkpoint](theme_main_integration_macos_2026-10-20.md) remains the historical
validation of `9b4ff208f`; it is not reused as proof for the new source tree.

Subsequent measurements must freeze the current API, parse metadata, output contract and inputs.
Previously prepared private probe migrations are drafts against the preceding merge and require
source revalidation before use. The all-family theme convergence goal remains incomplete.
