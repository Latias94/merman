# Lazy staged empty themes — macOS, 2026-10-18

## Decision

Commit `cc3c7f67e` caches the exact result of each staged Mermaid theme's empty, normalized
compatibility execution. The cache is entered only when normalized `themeVariables` is empty and
the configuration has no theme-variable calculation owners. Nonempty overrides, owner-bearing
configurations, invalid inputs, resource limits, cancellation, and every non-staged theme retain
the original executor.

The cache is populated by the existing staged executor and keeps its `serde_json` insertion order,
including nested objects. It does not use the generated snapshot, whose sorted object keys would
change public `effective_config` bytes and could be observed by a custom detector. Each request
clones the prepared ordered map and uses the existing single compatibility-preserving setter;
normalization still commits only after successful preparation. Each of the four staged programs
has one process-wide `OnceLock<Result<...>>`, so successful results and original errors are bounded
and deterministic.

This is an exact complete-public-operation allocation and work-removal admission. It makes no
calibrated latency claim. The primary cell is `zenuml_medium--spotless`; the control matrix covers
all public staged themes and all diagram families.

## Public controls

The 178 public SVG, report, and ordered custom-measurement controls are byte-for-byte identical.
All stable memory controls are non-increasing. Five variable Flowchart controls remain
inconclusive and are not used as wins.

The independent catalog contains 38 scenes, 35 logical families, no theme plus all 10 compiled
presets, for 418 cells. Every cell emits the same SVG and report, with the same family, diagnostics,
theme status, admission result, and stable counters:

- 380 preset cells reduce allocation count and allocated bytes;
- 114 cells reduce by 1,070 allocations and 74,871 bytes, with peak growth down 872 bytes;
- 152 cells reduce by 851 allocations and 56,564 bytes, with peak growth down 1,076 bytes;
- 114 cells reduce by 1,050 allocations and 69,135 bytes;
- all 38 no-theme cells remain unchanged in all four memory metrics;
- retained growth is unchanged in every cell.

The catalog status counts remain 37 `NotApplicable`, 229 `Residual`, 72 `Verified`, and 80
`Incomplete`. Partial support and fallback diagnostics remain visible.

For the primary Zenuml/Spotless cell, allocation count changes from 3,265 to 2,215 and allocated
bytes from 612,793 to 543,658. Peak growth remains 270,788 bytes and retained growth remains
9,792 bytes.

## Bounded cache cost

After initializing the generated theme runtime through the original full path, a fresh process
measures the first empty staged preparation as follows:

| Theme | Retained growth on first preparation |
| --- | ---: |
| Base | 70,327 bytes |
| Dark | 70,467 bytes |
| Forest | 73,547 bytes |
| Neutral | 67,064 bytes |
| Sum of four fixed entries | 281,405 bytes |

The second preparation for each theme has zero retained growth. The cache has exactly four fixed
entries and does not grow with requests, diagrams, or configuration values. This bounded cost is
accepted because it removes roughly one thousand allocations and tens of kilobytes from every
eligible render while preserving the public operation. First-request wall time remains outside
this admission and must be measured separately if it becomes a product concern.

## Verification

The focused theme suite passes 139 tests. Optimized native tests for the six-package surface pass
8,254 tests with 13 skipped; Clippy passes with warnings denied. Theme qualification, WASM host,
minimal features, full WASM build, WASM smoke, all-family DOM structure/parity/parity-root, and
format checks pass.

Raw registration, source/build receipts, 178 controls, cache retention, and the 418-cell capture
are under `target/bench/experiments/theme-staged-empty-lazy-20261018/`; the catalog comparison is
`target/bench/experiments/theme-catalog-coverage-20261018/stagedemptylazy-comparison.json`.
Host: Apple M4 Pro/48 GiB, macOS aarch64, Rust 1.95.0. Cargo, probes, and validation ran serially.

This change does not claim to close the v0.9.0 latency gap. The 19-scenario same-SVG diagnostic
and its eight rows above the 10%/1 µs screen remain the separate release-context evidence in
[the v0.9.0 refresh](theme_v090_same_svg_refresh_macos_2026-10-18.md). The next candidate must
attribute one remaining consumer path before changing another shared theme boundary.
