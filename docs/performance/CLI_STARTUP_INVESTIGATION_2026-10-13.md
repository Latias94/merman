# CLI startup and configuration investigation, 2026-10-13

The full CLI's small-diagram latency needs work on process startup, file publication, and font
discovery. Deferring Clap argument construction did not produce a material improvement and was
discarded. A separate comparison of the existing theme ownership PR investigates native parsing;
it must not be presented as a fresh-process CLI improvement.

This follows the local Merman/mmdr diagnostic comparison of the same date. All figures here are
diagnostics on one Windows host, not calibrated production admission results or cross-platform
latency budgets. No production change is retained by this investigation.

## Frozen inputs and measurement boundaries

- Host: Windows 11 x86_64, Intel Core i7-11700, Rust 1.95.0.
- Main revision: `3a4b8c393939b69372a8f05a2b971f7cdf6ae529`.
- Theme ownership [PR #178](https://github.com/Latias94/merman/pull/178) revision:
  `f47abded02b540d7fe563af7bc75f2fa1bb90c51`.
- Main working directory and unrelated files were left untouched. Experiments used separate
  worktrees; the theme PR was read and measured without editing it.
- Cargo builds were serialized. No Cargo build or profiler ran during the reported timed batches.
- Current full CLI builds used `scripts/artifact_profile_recipe.py cli-release --build-host
  --locked` with `CARGO_BUILD_JOBS=1`. These local Windows builds use dynamic CRT; the official
  historical Windows release binaries use static CRT. Compare local base/candidate with each
  other, rather than treating them as identical builds of the official release binary.
- Fresh-process measurements include process launch and complete exit, with warm OS caches.
  Background activity and Windows security scanning were not independently controlled.
- Native pipeline measurements use the existing Criterion harness, default features plus `svg`,
  locked dependencies, 30 samples, 2 seconds warmup and 3 seconds measurement per invocation.
  The main and theme PR manifests differ, including additional theme contract dependencies.
  This is a broad PR comparison, not causal attribution to one isolated patch.

## Clap construction is too small to explain the regression

The startup path constructs the Clap command before processing `--version`. Version handling
exits before diagram operations, file preflight, font discovery, or rendering. Temporary optimized
test instrumentation measured command construction and construction plus version parsing in
eight rounds of 2,000 iterations, following 100 warmups.

- Command construction: round averages from **0.120 to 0.164 ms**.
- Construction plus version parsing: **0.122 to 0.165 ms**.
- These are warm, process-local measurements, not a CPU profile of the first cold invocation.

A candidate used Clap's [deferred subcommand construction](https://docs.rs/clap/latest/clap/_derive/index.html),
preserving derive-owned argument schemas for root mmdc flag reflection and explicit construction
for manual generation. Its 91 CLI contract invocations had identical stdout, stderr and exit
status, covering all five completion shells, help, version, capability metadata and error paths.
These contracts do not constitute full successful-operation or manual-output coverage. SVG/PNG timing
outputs also matched byte for byte.

Each row below interleaved 24 fresh processes per executable, after warmups, reversing paired
order on alternate observations. The candidate was uncommitted; this was a diagnostic screen.

| Complete operation | Main median | Deferred candidate | Change |
| --- | ---: | ---: | ---: |
| Tiny native SVG to stdout | 61.16 ms | 63.49 ms | +3.80% |
| Tiny mmdc-compatible SVG to stdout | 61.09 ms | 58.47 ms | -4.29% |
| `--version` | 38.33 ms | 38.45 ms | +0.30% |
| `--help` | 38.56 ms | 37.80 ms | -1.96% |
| Medium sequence SVG to stdout | 45.36 ms | 44.86 ms | -1.09% |
| Tiny flowchart Mermaid input to PNG file | 137.68 ms | 139.86 ms | +1.59% |

There was no consistent material win, so the candidate and temporary Rust instrumentation were
removed. A calibrated, power-sized confirmation run was not attempted. This does not prove zero
possible benefit; it rejects this candidate for the registered CLI latency objective. Saving
approximately 0.1 ms cannot explain the earlier approximately 12 ms historical startup gap.

## Windows image size and imported DLLs deserve attention

PE inspection of the official Windows releases found these changes:

| Region | v0.7.0 bytes | v0.9.0 bytes |
| --- | ---: | ---: |
| Executable | 30,224,384 | 50,822,656 |
| `.text` | 22,153,216 | 38,896,640 |
| `.rdata` | 7,270,400 | 10,595,328 |
| `.pdata` | 683,008 | 1,177,600 |

Most growth is executable code. Compressing embedded assets alone is not supported as the main
remedy. New direct imports include `OLEAUT32.dll`, `CRYPT32.dll`, and `IPHLPAPI.dll`, with string,
certificate-store and network-adapter APIs. This establishes dependency growth, not which feature
or Windows subsystem accounts for every millisecond of actual Merman startup.

A separate 2x2 Rust control experiment varied only three retained DLL imports and a retained,
unread 20 MB constant. All four programs printed the same line and exited successfully; none
called the added DLL functions or traversed the constant. They used Rust 1.95.0, optimization
level 3, static CRT and stripped symbols. Each retained batch has 64 fresh processes per case,
with rotating and reversed order after three warmups.

| Control | Executable bytes | Retained batch A median | Retained batch B median |
| --- | ---: | ---: | ---: |
| Small program | 222,720 | 12.13 ms | 12.48 ms |
| Three extra DLL imports | 223,232 | 15.33 ms | 15.69 ms |
| Unread 20 MB constant | 20,222,976 | 20.40 ms | 19.99 ms |
| Imports and constant | 20,222,976 | 23.20 ms | 23.03 ms |

In these controls, added imports cost approximately 3.2 ms and added size approximately 7.5-8.3 ms.
Process cycle counts also increased with imports but changed little with padding. This supports
investigating image loading and dependency initialization ahead of Clap construction. It is not
an additive decomposition of Merman's measured startup gap; actual code pages, relocation,
security scanning, and cache behavior can differ from this artificial control.

WPR CPU tracing was unavailable because Windows rejected enabling the tracing policy
(`0xc5585011`). `GetProcessTimes` was too coarse for these short processes. There is no CPU stack
profile identifying a particular DLL initializer or security product as the cause.

## Theme ownership PR

The frozen theme PR build completed. Each row is the median of two Criterion round estimates,
with main/theme order reversed in the second round. Both parse projections and SVG outputs were
stable within each revision before and after timed work. All six parse compatibility projections
matched between revisions. Five SVG outputs changed; identical parse projections do not prove
that the full effective configuration, theme state or rendered contract remained equivalent.

| Fixture | Main parse | Theme PR parse | Main end-to-end | Theme PR end-to-end | SVG identity |
| --- | ---: | ---: | ---: | ---: | --- |
| `flowchart_tiny` | 185.06 us | 380.74 us | 369.69 us | 826.07 us | Different bytes |
| `class_tiny` | 183.26 us | 361.10 us | 348.05 us | 755.92 us | Same bytes |
| `sequence_tiny` | 166.18 us | 391.12 us | 359.56 us | 904.37 us | Different bytes |
| `flowchart_medium` | 536.25 us | 745.98 us | 2460.80 us | 3660.25 us | Different bytes |
| `sequence_medium` | 229.88 us | 450.17 us | 653.67 us | 1300.80 us | Different bytes |
| `mindmap_medium` | 306.35 us | 237.09 us | 599.37 us | 1017.23 us | Different bytes |

The PR does not establish a general small-diagram improvement. Five parse rows are slower;
mindmap parsing improves by approximately 23%. The strongest output-preserving diagnostic is
`class_tiny`: the parse projection and SVG bytes match, but parse time rises approximately 97%
and end-to-end time approximately 117%, consistently in both runner orders. These are large
diagnostic changes warranting investigation before a performance claim, not statistically
calibrated admission or causal attribution to a single change in this broad PR.

The source explains why an apparent configuration cache improvement is not enough. The initialized
global theme is `default`, while ordinary flowchart, class and sequence family defaults select
`redux-color`. Those requests still enter `OperationConfigBuilder::materialize_operation_theme`.
The inherited initialized fast path applies when the selected theme already matches. Family
paint ownership freezing only applies to ER/gitGraph, and ordinary unmodified detector config
can finish its checkpoint without a full value comparison. Those guards do not justify blaming
the entire regression on a new checkpoint or paint-freeze call.

Additional existing layout/render stage diagnostics used two reversed-order rounds with the
same sampling settings:

| Fixture | Stage | Main | Theme PR | Prepared-stage output identity |
| --- | --- | ---: | ---: | --- |
| `class_tiny` | layout | 52.71 us | 86.08 us | Same layout projection |
| `class_tiny` | render | 56.88 us | 73.48 us | Same SVG bytes |
| `mindmap_medium` | layout | 114.66 us | 140.07 us | Same layout projection |
| `mindmap_medium` | render | 135.54 us | 258.52 us | Different SVG bytes |

Class preparation and later stages both warrant attention; the end-to-end change is not isolated
to graph layout. Stage timings use different prepared inputs and must not be added or subtracted
as an exact end-to-end profile. A layout projection also does not hash every internal resource.

Source-backed attribution candidates, rather than measured CPU fractions, are:

- PR theme materialization clones the configuration transactionally, then writes theme variables;
  Arc-backed JSON can incur another full copy-on-write. The operation builder also resets those
  variables and reapplies family appearance when the initialized and selected themes differ.
- Derived-ownership propagation constructs dependency collections, maps and formatted config
  paths even in ordinary requests with no typed compatibility binding or explicit ownership.
  Configuration setters also maintain additional ownership/mutation state.
- Mindmap uses the initialized global default rather than the scoped `redux-color` choice. Its
  improved parse timing is consistent with avoiding the materialization work, but it does not
  prove the individual cost of those functions.
- The complete renderer now always finalizes the standalone SVG for target admission, even when
  no explicit pipeline is requested. Main could return the emitted SVG directly in that case.
  The default parity path fingerprints SVG resources, traverses XML for resource budgets and
  reference expansion, and computes document/target evidence hashes. It does not run the entire
  resvg compatibility/CSS closure validation by default. The stage `render` bench calls the
  lower-level SVG emitter without this finalization. Identical SVG bytes do not remove this
  additional work. Its guarantees need an explicit cost budget, rather than being deleted based
  only on a latency comparison.

The narrow follow-up is an instrumented optimized build of the existing theme branch that
counts materializations/COW and times selected-theme application, derived-ownership propagation,
and render finalization. Run the same class and mindmap public operations, preserve detector,
explicit-ownership and null-theme contracts, then remove instrumentation before any candidate
admission. Do not add a parallel configuration cache implementation to main based only on this
broad comparison.

Frozen source anchors: [operation theme selection](https://github.com/Latias94/merman/blob/f47abded02b540d7fe563af7bc75f2fa1bb90c51/crates/merman-core/src/parse_pipeline.rs#L99),
[transactional theme application](https://github.com/Latias94/merman/blob/f47abded02b540d7fe563af7bc75f2fa1bb90c51/crates/merman-core/src/theme.rs#L1851),
[derived ownership](https://github.com/Latias94/merman/blob/f47abded02b540d7fe563af7bc75f2fa1bb90c51/crates/merman-core/src/theme.rs#L822),
[complete-render finalization](https://github.com/Latias94/merman/blob/f47abded02b540d7fe563af7bc75f2fa1bb90c51/crates/merman/src/render.rs#L1265),
[standalone resource scanning](https://github.com/Latias94/merman/blob/f47abded02b540d7fe563af7bc75f2fa1bb90c51/crates/merman-render/src/svg/pipeline/standalone.rs#L83),
and [document evidence](https://github.com/Latias94/merman/blob/f47abded02b540d7fe563af7bc75f2fa1bb90c51/crates/merman/src/render/document.rs#L498).

## Fonts and publication remain separate costs

The preceding fixed-SVG raster control measured about 149.17 ms with the default system catalog
and 59.91 ms with one explicit Arial file. Its PNG hashes differed. This is evidence about font
catalog cost and a supported user configuration, not an equivalent-output optimization.

The default raster catalog uses a process-local `OnceLock`; each fresh CLI process discovers
fonts again. The canonical full CLI already enables `fontdb` memory mapping through its PDF
dependency closure. Adding `memmap` would not change that build. Mmdr's persisted font index is
used by its text measurement path, lacks an explicit font/file-change invalidation contract, and
does not eliminate every native PNG catalog load. Copying that cache is not justified without
addressing stale fonts, fallback order and invalidation.

For callers with known font requirements, explicit font files/directories can reduce discovery.
Callers requiring arbitrary fallback should reuse a rendering context when possible. A catalog
restriction or persistent cache needs its own output/fallback contract; it must not silently
replace the default system catalog.

The earlier within-batch v0.9 SVG file-versus-stdout difference was approximately 12 ms, with
identical SVG bytes. Current file publication includes identity checks, atomic rename and
`sync_all`; v0.7 and mmdr use ordinary writes. That differential covers the entire publication
path, not fsync alone. Keep these guarantees unless a separately defined API policy changes
them. Stdout is already available for integrations that own storage.

## Recommended next steps

1. Investigate the existing theme PR's class and sequence cost before describing it as a native
   performance improvement. Retain per-row output checks and distinguish intentional API/output
   changes from equivalent-work improvements; avoid an overlapping cache implementation.
2. For Windows CLI startup, test full-capability binary size and linker import policies with
   frozen production artifacts. A narrower diagnostic could test delayed loading of optional
   Windows DLLs, then exercise the affected certificate, networking and font paths. Do not replace
   platform certificate roots or remove features merely to claim a faster equivalent CLI.
3. For editor PNG preview, measure known-font configuration or context reuse in the actual
   integration. Font catalog work is much larger than the measured Clap construction cost here.
4. Measure Linux and Apple Silicon before changing portable architecture based on Windows image
   loading. The inkmd commit measured Apple Silicon, and its normal PNG route used librsvg.

## Local evidence and reproduction

The local evidence directory is
`target/bench/experiments/cli-startup-2026-10-13` in the isolated `perf/cli-startup` worktree.
It contains the ledger, rejected patch, optimized phase probe, CLI contract samples, fresh-process
samples, loader control source and compiler reproduction commands, native logs and Criterion raw samples. Evidence
is diagnostic; an implementation admission requires the calibrated confirmation workflow in
`docs/performance/BENCHMARKING.md` and approved clean revisions.

Full CLI source build:

```powershell
$env:CARGO_BUILD_JOBS = '1'
python scripts/artifact_profile_recipe.py cli-release --build-host --locked
```

Native source build, separately in each frozen worktree:

```powershell
cargo bench --locked -p merman --features svg --bench pipeline --no-run --message-format=json-render-diagnostics
```

During timing, invoke the emitted executable directly with isolated `CRITERION_HOME`, rather than
running Cargo alongside measurements:

```text
pipeline.exe --bench --noplot --sample-size 30 --warm-up-time 2 --measurement-time 3 --exact parse/flowchart_tiny
```

The full CLI base SHA-256 is
`07ef44e50ef488ce3e4954944a6b11fe5c10a13e6609dc547697bd707b8d3487`;
the discarded deferred candidate SHA-256 is
`68a5984086a066ca0f2b3ade7c0876d4c394153e64056ee1e8986fc50833266d`.
The frozen theme pipeline SHA-256 is
`85148b47f89d7c6a772c5d848cb26bfeb4c8466bf135ea776ef2b189d2ddf099`.
Loader executable hashes and raw sample hashes are included in the local evidence manifest.
An early exploratory loader batch was overwritten during a repeat; only the two separately
retained batches appear in the table. No formal cross-platform correctness suite or calibrated
optimization admission was performed because no production patch is retained.
