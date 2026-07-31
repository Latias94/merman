# Cached Baseline Registries Performance Decision (2026-07-31)

Status: accepted for repeated fresh-`Engine` construction in one running process.

## Question

Can Merman initialize the immutable pinned detector, semantic-parser, and render-parser registries once and return copy-on-write clones instead of rebuilding one vector and two maps for every fresh `Engine`?

## Revisions

- Base: `d9c8e9a9d84e4166a4502187f094094c4e1cd91e`
- Candidate: `b56e1ae819a157906f11f4b8759bc214c16b7b76`
- Candidate commit: `perf(core): cache pinned registry baselines`

The candidate changes only `DetectorRegistry::pinned_mermaid_baseline`, `DiagramRegistry::pinned_mermaid_baseline`, and `RenderDiagramRegistry::pinned_mermaid_baseline`. Each baseline is stored in `OnceLock`; returned registries still use the existing `Arc::make_mut` copy-on-write path, so per-engine custom detectors and parsers remain isolated.

## Registered evidence contract

The primary public lane was `typed-render-model-parse-cold`, selected as `parse_cold_engine/info_medium`. It constructs a fresh default `Engine` for every measured logical operation. The low-latency admission thresholds were frozen before confirmation at 10% relative improvement and 1,000 ns absolute improvement, with eight base A/A pairs, eight candidate A/A pairs, a maximum of 64 AB/BA pairs, 30 Criterion samples, one second of warm-up, two seconds of measurement, bootstrap seed `2026073104`, and 10,000 bootstrap resamples.

`parse/info_medium`, which reuses one `Engine`, was registered as a non-regression control. `parse_cold_engine/flowchart_medium` was also attempted as a broader control, but its independent A/A absolute intervals exceeded the 1,000 ns low-latency stability margin. The harness therefore stopped that row before A/B measurement, and it is not used for a performance claim.

## Results

The initial same-revision upper-bound check measured reused-engine parsing at 865.68 ns and cold-engine parsing at 4,226.0 ns, leaving a 3,360.32 ns construction ceiling.

| Evidence | Base | Candidate | Change | Decision |
| --- | ---: | ---: | ---: | --- |
| Two-pair diagnostic, `parse_cold_engine/info_medium` | 4.36 us | 959.50 ns | -3.40 us / -77.99% | Advanced to confirmation |
| Two-pair diagnostic, `parse/info_medium` control | 821.41 ns | 837.68 ns | +16.27 ns / +1.98% | Below the registered gate |
| Clean confirmation, `parse_cold_engine/info_medium` | 4.233 us | 937.97 ns | -3.295 us / -77.84% | Confirmed improvement |
| Combined-family confirmation, Info row | 4.291 us | 956.74 ns | -3.334 us / -77.70% | Confirmed improvement |
| Combined-family confirmation, Flowchart row | unavailable | unavailable | A/A unstable | Inconclusive; no A/B interpretation |

The clean single-row confirmation used eight final AB/BA pairs. Its simultaneous one-sided 95% bounds were `-77.93%..-77.73%` and `-3,306.04..-3,283.01 ns`, clearing both frozen improvement thresholds.

The strict low-latency stability checks also passed. The largest base A/A absolute endpoint was 50.0 ns and the largest candidate endpoint was 14.47 ns, both below `min(1,000 ns, 5% of baseline)`. The largest absolute log-ratio endpoint was 0.01559, below `-ln(0.95)`, and three times every noise endpoint remained below the registered improvement thresholds.

## Semantic and quality gates

- `cargo fmt --all -- --check`
- `cargo nextest run --locked -p merman-core`: 1,364 passed
- Baseline-constructor tests assert that separately requested pinned registries share storage before mutation and detach through copy-on-write when customized.

## Decision and claim boundary

Retain the candidate. It removes a fixed allocation and population cost from repeated default `Engine` construction without changing parser selection, registry contents, detector order, public APIs, or custom-registry mutation semantics.

The measured claim applies to repeated fresh engines inside an already-running process. The first process-local baseline initialization still performs the original construction work. No end-to-end render improvement, first-call startup improvement, or `flowchart_medium` timing improvement is claimed from this experiment.

## Raw evidence digests

- Diagnostic JSON: `d89298dd07bf08528f17a6b12ee5b994df3f6f45bc786434e7f43c4b6ee4fc2c`
- Reused-engine control JSON: `d458b8704b95b14f31b452fae9d1f3b46cfa4a0bf75e77e07e71619cd9b46952`
- Clean confirmation JSON: `706a6a7211fe2b4bc806786e9644ae15ff63ee30a74a59d995059a331793def5`
- Combined-family confirmation JSON: `15da06323bf47734fc424633a0e933060330433a7c2de8d1389137eb96754a91`

Raw reports remain under `target/bench/experiments/cached-baseline-registries/` and are intentionally excluded from source control.
