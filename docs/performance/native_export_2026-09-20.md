# Native export throughput checkpoint — 2026-09-20

## Decision and workload

Source `671ad89723a0364cffd8c3a5fb6c956b06c46d72` has a current native SVG/PNG/PDF throughput baseline for the
unchanged public Cyberpunk Flowchart, Sequence and XY Chart fixtures, under native default and
complete Cyberpunk themes. All 18 operations succeed with default resource limits. This is a
diagnostic workload observation, not a matched alpha.6 regression or an optimization admission.

The measured Cargo lane disables defaults and enables
`svg,png,jpeg,pdf,embedded-fonts,math,layout-elk,layout-cytoscape`. It is an advanced source-build
feature combination, not an installed-package or governed artifact-profile receipt. The host is
Apple M4 Pro, 48 GiB RAM, macOS 26.6.2 ARM64, Rust/Cargo 1.95.0 and Criterion 0.8.2, using the
optimized bench profile. Cargo and timing ran serially with ordinary desktop background load.

The public `Renderer` is reused. Each operation includes strict parsing, layout, resvg-safe SVG
emission, terminal encoding, request cloning and output destruction. `htmlLabels` is false and
diagram IDs are fixed. The compiled Cyberpunk theme is prepared before timing; default means no
explicit theme override. No font assets are embedded. Host font discovery is warmed by preflight.
PNG uses default 1x scale with no fit box; PDF requests the default filter scale of 4.0 under its
unchanged aggregate filter-image budget. No resource or quality limit was relaxed.

Each of 18 rows ran in one process with 2 seconds of warmup, a 3-second measurement target and
30 flat samples, totaling 540 samples. Expensive PDF cases exceed the requested target to collect
all 30 observations. The intervals below are Criterion mean-estimate 95% bootstrap intervals,
not independent paired-run confidence bounds. Throughput is the inverse mean latency, in complete
outputs per second. Source/output hashes and output files are retained.

## Measurements

| Family / theme / output | Mean ms [95% interval] | Outputs/s | Output bytes |
| --- | ---: | ---: | ---: |
| flowchart / cyberpunk / pdf | 484.220 [482.738, 485.874] | 2.07 | 489,421 |
| flowchart / cyberpunk / png | 45.207 [45.116, 45.299] | 22.12 | 77,940 |
| flowchart / cyberpunk / svg | 1.965 [1.955, 1.975] | 508.96 | 33,284 |
| flowchart / default / pdf | 4.313 [4.290, 4.338] | 231.87 | 19,953 |
| flowchart / default / png | 5.696 [5.646, 5.750] | 175.58 | 12,997 |
| flowchart / default / svg | 0.876 [0.868, 0.887] | 1141.38 | 16,725 |
| sequence / cyberpunk / pdf | 730.586 [728.700, 732.613] | 1.37 | 408,308 |
| sequence / cyberpunk / png | 58.990 [58.849, 59.137] | 16.95 | 84,196 |
| sequence / cyberpunk / svg | 2.401 [2.388, 2.414] | 416.55 | 49,247 |
| sequence / default / pdf | 3.650 [3.623, 3.681] | 273.97 | 17,637 |
| sequence / default / png | 5.059 [5.032, 5.089] | 197.65 | 11,040 |
| sequence / default / svg | 0.865 [0.859, 0.873] | 1155.67 | 24,233 |
| xychart / cyberpunk / pdf | 378.604 [377.756, 379.485] | 2.64 | 366,365 |
| xychart / cyberpunk / png | 42.598 [42.489, 42.703] | 23.48 | 86,398 |
| xychart / cyberpunk / svg | 1.941 [1.931, 1.952] | 515.32 | 33,907 |
| xychart / default / pdf | 3.473 [3.455, 3.493] | 287.93 | 18,982 |
| xychart / default / png | 5.794 [5.767, 5.826] | 172.58 | 15,377 |
| xychart / default / svg | 0.493 [0.492, 0.495] | 2026.40 | 9,244 |

Cyberpunk PNG costs 42.6–59.0 ms per output (17.0–23.5 outputs/s), while its PDF path costs
378.6–730.6 ms (1.37–2.64 outputs/s). The corresponding SVG generation costs 1.94–2.40 ms.
This identifies native PDF as a substantial cost surface for these complete effect-heavy designs.
Default-theme PDFs are 17.6–20.0 KB; Cyberpunk PDFs are 366–489 KB. Their different appearance,
geometry and filter work prevent interpreting the difference as a regression between equal outputs.

The export source documents a vector PDF path with localized rasterization of SVG filter regions,
and the default requested filter scale is higher than PNG's 1x raster scale. This supplies a
plausible source-backed explanation for the cost, not a measured attribution to any individual
function. The earlier [shadow checkpoint](zero_offset_shadow_2026-09-18.md) separately records
substantial PDF cumulative allocation. Together these observations justify retaining PDF cost
as a profiling target; they do not justify reducing output quality, raising budgets, bypassing
resource accounting or claiming a cache/algorithm improvement without another experiment.

## Validation and scope

Criterion test mode passes 18/18 cases. Every timing row parses its SVG, decodes its PNG with
positive dimensions, or checks PDF framing before measuring; after timing, output bytes must
match preflight exactly. All 18 retained artifacts match the emitted SHA-256 and byte-length
receipts. `cargo fmt --all -- --check` and `git diff --check` pass. No production code changed.

Independent macOS PDFKit verification opens all six PDFs as one-page documents with positive
page dimensions and rasterizes each to PNG. The three Cyberpunk previews were visually inspected:
labels, glow and complete scenes are visible. This is a reader/raster smoke and bounded visual
inspection, not quantitative PDF pixel qualification or catalog promotion. The temporary Swift
probe and its preview hashes are retained with the measurement; no new validation framework or
runtime dependency was added.

The evidence is warm-process and system-font-dependent. It does not measure process cold start,
first-touch font loading, transport overhead, export from a reused rendered document, concurrency,
allocation counts or peak memory. The basic `svg,png,pdf` feature combination is supported by the
benchmark entry but was not measured in this checkpoint. No alpha.6 binary-export comparison or
balanced independent revision confirmation was performed; those deltas remain Unverified.

## Reproduction

The benchmark and lifecycle contract are documented in
[BENCHMARKING.md](BENCHMARKING.md#native-svgpngpdf-throughput). For this checkpoint, use the advanced
feature list above and set separate `CRITERION_HOME` and `MERMAN_BENCH_ARTIFACT_DIR` directories.
The exact measured executable, command, source, lockfile, input digests and host are recorded in
`target/bench/experiments/native-export-20260920/experiment.yaml`. The executable was built from
the exact benchmark bytes in the source commit above; timing began after that commit was created.

The ignored local directory also retains `contract.log`, `timing.log`, `fmt.log`, `summary.json`,
`criterion/`, all 18 outputs under `artifacts/`, and the PDFKit probe, receipt and preview images.
Each summary row binds its Criterion raw JSON files and source/output identity. The PDFKit check
uses Apple Swift 6.3.2 and the host PDFKit framework.

The summary SHA-256 is `73c068f1e2654c08963c8e239c6e573da44ead65327d83fc3319629d0573b3a0`.
