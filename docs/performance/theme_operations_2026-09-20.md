# Theme compilation and discovery checkpoint — 2026-09-20

## Decision and scope

Source `bb411e505a33fd03503d8d7747d4ed323c0622fe` now has a reproducible native diagnostic baseline for theme
compilation, materialization, preset export, support queries and catalog construction. Both Cargo
feature lanes pass all 13 benchmark contract checks. This is not candidate admission or a
performance improvement claim; U10's matched revision and transport cost gates remain open.

The SVG-only lane enables `svg`. The advanced lane enables
`svg,png,jpeg,pdf,embedded-fonts,math,layout-elk,layout-cytoscape`, with default features disabled in
both. These are explicit Cargo feature combinations, not governed packaged-artifact receipts.
The host is an Apple M4 Pro with 48 GiB RAM, macOS 26.6.2 ARM64, Rust/Cargo 1.95.0, Criterion
0.8.2 and the optimized bench profile. Cargo builds and timing processes ran serially; ordinary
desktop background load was present. The benchmark reuses the existing target directory.

Each lane ran once in a separate process, SVG first, after contract preflight. Every row uses
2 seconds of warmup, 3 seconds of measurement and 30 internal Criterion samples: 780 samples
across 26 rows. The intervals below are Criterion's mean-estimate 95% bootstrap intervals, not
independent repeated-process or paired-revision confirmation intervals. No cross-profile speedup
or regression is inferred from these numbers.

## Observations

All values are **microseconds**, shown as mean [95% interval]. The support row measures the full
22-query shared batch; it is not a single query's latency.

| Operation | SVG only | Advanced |
| --- | ---: | ---: |
| `theme_catalog/fresh_engine` | 1505.931 [1498.906, 1511.585] | 1537.735 [1533.734, 1541.510] |
| `theme_catalog/reused_engine` | 1494.846 [1488.847, 1502.332] | 1514.842 [1507.500, 1522.753] |
| `theme_compile_definition/dark` | 48.165 [48.000, 48.332] | 48.151 [47.971, 48.315] |
| `theme_compile_definition/default` | 45.005 [44.800, 45.244] | 45.218 [44.901, 45.671] |
| `theme_compile_definition/light` | 48.002 [47.812, 48.230] | 47.942 [47.752, 48.122] |
| `theme_compile_preset/cyberpunk` | 222.539 [221.874, 223.171] | 226.832 [225.514, 228.201] |
| `theme_compile_preset/editor-light` | 138.022 [137.375, 138.923] | 140.282 [139.777, 140.815] |
| `theme_export_preset/cyberpunk` | 122.105 [121.552, 122.751] | 127.724 [127.299, 128.183] |
| `theme_export_preset/editor-light` | 67.042 [66.799, 67.273] | 70.609 [70.341, 70.897] |
| `theme_materialize/dark` | 10.834 [10.802, 10.866] | 11.247 [11.212, 11.282] |
| `theme_materialize/default` | 7.735 [7.698, 7.769] | 8.039 [8.002, 8.075] |
| `theme_materialize/light` | 10.805 [10.764, 10.845] | 11.131 [11.084, 11.176] |
| `theme_support/shared_vectors` | 28.198 [28.112, 28.274] | 29.112 [29.001, 29.216] |

Default definition means an empty `tokens` object in a valid version-one authoring definition.
Light/dark inputs are the shared authoring goldens. Compile rows include admission and complete
recipe compilation; materialize rows stop at the public materialized JSON result. Preset export
returns the public self-contained recipe JSON. Timings include result allocation and destruction.
The resource policy is explicitly `interactive`, with deterministic engines. No input embeds fonts.

Catalog construction costs about 1.5 ms in this workload. The source-backed explanation is that
`ThemePresetDescriptor::describe` invokes `compile_preset` to assess resource-policy availability
for each of the ten descriptors on every request. A reused engine does not cache that result.
The measurements identify recurring work, but do not isolate its full cost with a profiler or
prove a safe cache optimization. Any future reuse must preserve effective resource policy and
artifact capability projection; a process-global catalog cannot simply ignore those inputs.
No cache or production behavior changed in this checkpoint.

## Correctness and limits

Preflight replays canonical complete-spec bytes for both authoring goldens, checks compiled
fingerprints against their materialized complete specs, compares all 22 support responses and
all preset descriptors against shared goldens, checks fresh/reused catalog byte equality, and
checks export/recompile fingerprints for Editor Light and Cyberpunk. Both feature lanes pass
13/13 Criterion test-mode cases; `cargo fmt --all -- --check` and `git diff --check` pass.
An initial benchmark-only setup error omitted the required `tokens` field. It failed before any
samples were collected, was corrected, and its failed log remains beside the passing logs.

Both catalog lanes run after preflight in a warm process. `fresh_engine` includes engine creation
and destruction; it does not measure process startup or first-touch initialization. Discovery is
a static support bound, not rendered qualification. This benchmark neither renders diagrams nor
measures allocation counts, peak memory, font embedding, transport overhead or packaged sizes.

Alpha.6's presentation catalog is not the current authoring/discovery contract. There is no
same-contract alpha.6 baseline in this checkpoint, so those historical deltas remain Unverified.
Do not combine old presentation-catalog latency with these results or use these samples to
relax budgets. Native memory, binary-export throughput and cross-transport costs need their
separate evidence lanes.

## Reproduction and receipt

The commands and lifecycle definitions are in
[BENCHMARKING.md](BENCHMARKING.md#theme-compilation-and-discovery). The measured executables were
built from the exact benchmark bytes committed in the source above. Their hashes, the lockfile,
fixture hashes, commands and setup history are recorded in the ignored local evidence directory
`target/bench/experiments/theme-operations-20260920/`:

- `experiment.yaml`: preregistration, source and executable identities, host, gates and outcome.
- `summary.json`: mean estimates, intervals and hashes of each row's raw JSON files.
- `svg/criterion/` and `advanced/criterion/`: separate raw Criterion sample directories.
- `svg-contract-fixed.log`, `advanced-contract.log`, `svg-timing.log` and `advanced-timing.log`:
  successful contract and timing logs; `svg-contract.log` retains the failed setup attempt.

The summary SHA-256 is `837469582a05f2b6028f525bbd289534b457f545722b4f4a1b9cec05f7375ca6`.
