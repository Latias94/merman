# Theme refactor PR checkpoint (macOS, 2026-10-19)

The first review batch contains the accumulated theme runtime responsibility cleanup, demand-driven
family preparation, shared XML output changes, data-only facade/transport execution, staged empty
theme preparation and allocation reductions. It does not close the all-family v0.9.0 convergence
goal. The [performance plan](PERF_PLAN.md) links the independent experiments and their boundaries.

## Integration

The branch incorporates `origin/main` at `252752f70` through merge `f77147795`. ER emission keeps
the upstream prepared geometry cache and the refactor's consumer-owned text styles. Resource-policy
documentation retains both SVG ownership and native-font budget guidance. The resulting unused
ER emission attribute-font copy is removed in `af243067a` and `08688eaf5`; layout measurement styles
remain. The tested product state is `08688eaf5`.

The upstream ER cache intentionally changes the number and stage of measurement callbacks.
Earlier incremental exact callback/output/allocation receipts remain tied to their recorded
revisions; they are not silently reassigned to the merged product. Release diagnostic timing is
likewise still attached to its frozen executables.

## Validation of the merged product

| Gate | Result |
| --- | --- |
| Release nextest: facade, core, renderer, export, bindings-core, theme-contract; all features | 8,275 passed, 13 skipped |
| Release Clippy for those crates, all features, warnings denied | Passed |
| Private theme acceptance | 34 passed |
| WASM host tests | 47 passed |
| Optimized full WASM build and 37-diagram smoke | Passed |
| Full SVG structure, parity and parity-root comparison with reviewed browser-text residuals | Passed |
| Representative dependency closures and feature graph | Passed, including 45 curated builds and isolated consumers |
| Criterion registration/output discovery | 459 benches and 459 receipts across seven lane groups |
| Native-memory compiled boundary smoke and zero-work verification | Passed |
| Performance Python contracts | 16 baseline, 51 memory, 29 memory-driver and 128 aggregate tests passed |
| Node package/transport tests with pinned npm 12.0.2 | 115 passed |
| Affected Python syntax and qualification/archive tests | Passed |
| Dependency license generation/projections, third-party checks and generator tests | Passed |
| `cargo deny` advisories, bans, licenses and sources | Passed |
| Formatting, path-length and diff checks | Passed |

Default local npm 11.20.0 initially fails the unchanged footprint test because its `npm pack --json`
shape differs from the repository's npm 12 contract. Running the complete suite with the already
installed pinned npm 12.0.2 passes; no package parser or test assertion is changed.

Three independent read-only reviews cover the ER merge, XML/resource/cancellation boundaries and
rich/data-only API/transport paths. They find no concrete additional defect; they are static reviews,
not substitutes for dynamic tests or external review.

Raw commands and logs are under `target/bench/experiments/theme-pr-20261019/`. Broad Linux workspace,
platform artifact and painted browser-containment owners remain in CI. Full strict release
preflight is not certified: the preceding run completed 661 feature builds and 36 artifact checks
before the missing local Mermaid CLI `esbuild` dependency stopped its final bundle gate.

Native ABI and shared binding-contract verification also pass. The broader platform verifier passes
Android target Clippy, the Flutter native build, generated ABI freshness and Flutter analysis, then
stops when the local Dart formatter changes an existing smoke-test expression. Its formatter and
generated workspace files remain excluded; full platform verification is not claimed.

After PR creation, merge `a7c3a006d` incorporates remote main `8a80b908b`, including the 0.9.1
workspace versions and fixture-provenance corrections. Only generated license reports conflict;
the existing generators rebuild all 13 source reports and 424 projections against the merged
lockfile. License, projection, release-version, 27 related Python contracts and formatting checks
pass. The independent fuzz lockfile additionally drops the removed facade `sha2` dependency without
changing any resolved package version; `cargo +nightly-2026-07-01 check --manifest-path
fuzz/Cargo.toml --locked` passes.

## Remaining performance work

The [latest same-SVG release diagnostic](theme_v090_same_svg_refresh_macos_2026-10-19.md) retains all
19 selected E2E scenes. Seven exceed both 10% and 1 us; the 57 changed-SVG fixtures and three
preprocessing fixtures remain separate. Those timing observations are uncalibrated and precede
the final color serializer and main integration. Allocation admissions establish only their
registered work/memory reductions. Continue attribution and high-confidence refactoring, then
calibrate A/A and run the fixed AB/BA procedure before claiming latency convergence.
