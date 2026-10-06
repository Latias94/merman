# Theme integration native checkpoint — 2026-10-06

## Scope

This diagnostic compares remote main `733fd2fa711cb25cf6d1d94f5797fff424f52d51`
with integrated theme source `5c40e66679755fd0339653737cf5e55f596092cd`.
Both sources were built from clean detached worktrees on the same Apple M4 Pro
(48 GiB RAM), macOS 27.0 ARM64, with Rust 1.95.0. Each revision keeps its lockfile.
These are whole-product revision differences, not isolated causal estimates of theme overhead.
No production optimization was applied.

Cargo ran serially in a shared target directory. Each benchmark executable was copied,
hashed and verified before sampling, so a later build could not replace either measured binary.
Builds and timed measurements did not overlap. Ordinary desktop background load was present.

## End-to-end latency

The operation is a deterministic native SVG render with trusted-native resource limits and
features `all-diagrams,svg,layout-cytoscape,layout-elk,math`, with defaults disabled.
Each row has two independent AB/BA process pairs, 30 Criterion samples per process,
2 seconds of warmup and 3 seconds of measurement. Values below are diagnostic point estimates.

| Fixture | Main | Theme integration | Difference |
| --- | ---: | ---: | ---: |
| Class medium | 620.595 µs | 1,318.000 µs | +697.405 µs (+112.37%) |
| Mindmap medium | 246.955 µs | 379.775 µs | +132.820 µs (+53.78%) |
| Info medium | 7.448 µs | 18.436 µs | +10.988 µs (+147.53%) |

All three rows have byte-identical SVG output across revisions and verified pre/postflight
receipts. Class and Mindmap exceed the registered diagnostic screening thresholds of both
10% and 50 µs. Info does not exceed the absolute screening threshold; that does not establish
non-regression or waive the dedicated low-latency contract.

The complete seven-fixture run returns `contract_failure` (exit 2): both Flowchart fixtures,
Sequence and Treemap have different SVG hashes and were not timed. Element counts remain
683, 269, 129 and 56 respectively; equal element counts do not prove equivalent output.
These four rows remain coverage exclusions. No comparator rule or fixture was relaxed.

This checkpoint does not establish a confirmed regression, non-regression or candidate
admission. Such claims require clean comparable selections, balanced A/A calibration and
fresh power-sized confirmation with at least eight pairs. Two-pair confidence intervals are
not used as decision-grade evidence here.

## Stage attribution

A diagnostic follow-up reuses the same frozen executables and original discovery receipts.
All 12 stage/fixture output identities match across revisions and pass before/after checks
in each of 48 processes. Each row uses the same two AB/BA pairs and long settings as above.
No Cargo build ran during these samples.

| Stage / fixture | Main µs | Theme integration µs | Difference µs |
| --- | ---: | ---: | ---: |
| `parse/class_medium` | 145.945 | 376.375 | +230.430 |
| `parse/mindmap_medium` | 130.285 | 112.085 | -18.200 |
| `parse/info_medium` | 1.339 | 1.861 | +0.521 |
| `parse_cold_engine/class_medium` | 151.845 | 388.345 | +236.500 |
| `parse_cold_engine/mindmap_medium` | 134.630 | 117.505 | -17.125 |
| `parse_cold_engine/info_medium` | 4.670 | 5.470 | +0.800 |
| `layout/class_medium` | 250.105 | 297.490 | +47.385 |
| `layout/mindmap_medium` | 55.275 | 57.286 | +2.011 |
| `layout/info_medium` | 0.409 | 1.735 | +1.326 |
| `render/class_medium` | 230.625 | 321.395 | +90.770 |
| `render/mindmap_medium` | 60.382 | 98.221 | +37.839 |
| `render/info_medium` | 3.868 | 6.095 | +2.227 |

These are attribution observations, not separately confirmed regressions. Class shows increased
cost in parsing as well as layout and SVG emission. Mindmap parsing is lower in this run,
layout is close, and SVG emission is higher. The next profiling targets are therefore Class
parse/model construction, Class/Mindmap SVG emission, and the facade's full SVG pipeline.
No individual function has been established as the cause.

Stage values must not be summed to reconstruct end-to-end latency: the batched layout/render
benchmarks exclude different setup work and the public end-to-end path includes additional
request, postprocessing and artifact lifecycle work. The parse receipt is the canonical
compatibility projection of the typed model; it does not assert equality of private render
context or theme evidence introduced on the branch. Correctness checks remain necessary.

## CLI artifact size

Both builds use the identical `cli-release` artifact recipe: `dist` (release with thin LTO),
20 explicit features, defaults disabled, native `aarch64-apple-darwin`. These are local unsigned
builds, not release archives or installed-package sizes. No cross-platform size claim is made.
Gzip uses level 9 and timestamp zero.

| Artifact | Main bytes | Theme integration bytes | Difference |
| --- | ---: | ---: | ---: |
| Raw executable | 45,511,408 | 53,462,736 | +7,951,328 (+17.47%) |
| Raw executable, gzip | 18,803,233 | 22,054,346 | +3,251,113 (+17.29%) |
| `strip -S` executable | 45,512,032 | 53,463,344 | +7,951,312 (+17.47%) |
| `strip -S` executable, gzip | 18,405,059 | 21,558,691 | +3,153,632 (+17.13%) |

On this Mach-O output, `strip -S` slightly increases file size; it is a measured debug-symbol
stripping variant, not a claimed size reduction or an all-symbol-stripped release artifact.
The `__TEXT` segment grows by 6,488,064 bytes, including 5,466,808 additional `__text` bytes.
`__LINKEDIT` grows by 1,376,256 bytes. The normal dependency closure grows from 338 to 341
package identities: `merman-bindings-core`, `merman-theme-contract`, and
`serde_json_canonicalizer` are added. The dynamic-library set is unchanged. This localizes most
of the size growth to code and link metadata but does not attribute it to individual functions.
`__PAGEZERO` is a virtual zero-fill reservation and is not counted as on-disk artifact size.

Both binaries pass `--version`, `capabilities --json`, and valid SVG renders for Flowchart,
Sequence, Class and Info, with no stderr. Class and Info CLI SVG bytes match between revisions;
Flowchart and Sequence differ, as in the pipeline coverage exclusions. The capability response
changes intentionally at the product surface: CLI contract version 5 becomes 6, the descriptor
digest changes, and `theme_presets` is added. Both report package version 0.8.0. Identical build
features therefore do not mean identical product behavior.

The head release build succeeds with 80 compiler warnings: 62 in `merman-render`, 15 in
`merman-export`, 1 in `merman`, and 2 in `merman-cli`. The baseline build log has no warnings.
Unused imports, variables and dead-code diagnostics need a separate feature-aware cleanup;
this checkpoint does not alter the measured source or run automatic fixes.

## Correctness and outstanding work

The measured head previously passed 5,722 Rust tests (8 skipped), the scoped feature matrix,
and the 37-family SVG structure/parity/root policies. Those checks do not close the browser
viewport gate: the macOS audit retains one blocking Flowchart title residual. Native Linux
Chromium evidence is still needed; the failed emulated browser attempts do not substitute for it.

The ignored local evidence lives under
`target/bench/experiments/theme-main-2026-10-06/` in the integration-repair worktree.
`experiment.yaml` records source/toolchain/fixture/build policies; `latency.json` preserves
all seven rows, output identities, executable identities, commands and raw process estimates.
`latency.md` is the unmodified comparator report, including its aggregate contract failure.

The `cli-base/` and `cli-head/` directories retain executable copies, gzip/strip variants,
`receipt.json`, build logs, four rendered controls, capability responses, section sizes,
dynamic libraries, and normal dependency trees. `measure_cli.py` is a local experiment script;
it is not added to the repository's build tooling.

`stages.json` and `stage-logs/` retain the follow-up process estimates, commands and raw logs;
`measure_stages.py` reuses the checked-in benchmark command/receipt/estimate helpers.
The checked-in [compact receipt](evidence/theme_main_integration_2026-10-06.json) preserves
source/artifact identities, observations and local raw-artifact hashes without requiring
local absolute paths. Keep both detached experiment worktrees and frozen binaries while
continuing profiling; the original user worktrees and untracked notes remain untouched.
