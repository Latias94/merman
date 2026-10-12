# Default CLI System-Font Discovery Checkpoint

> Current policy (2026-10-12): ordinary CLI Mermaid graphical output defaults to
> deterministic measurement without a native font context or system discovery for
> layout. PNG/JPEG/PDF painting can still load system fonts through the existing codec
> path. Default/release profiles still compile native/system context support. Explicit
> files/directories enable native measurement with a closed font set;
> `--font-measurement native` without sources or
> `--font-system on` explicitly requests system discovery when available. The original
> executable defaults, measurements, and results below are historical and unchanged;
> this note does not claim a new timing run. See the [current CLI font guide](../../crates/merman-cli/README.md#explicit-fonts).

Date: 2026-10-10, Asia/Shanghai. This is a same-host comparison of two
unpublished, uncommitted native-font implementation snapshots based on
`292826512`. It is not a comparison with published 0.8.0. Both executables
already enable actual-font measurement and system discovery by default.

## Result

For the [user's Flowchart](../assets/native-font-measurement/flowchart.mmd),
with **no font/config options**, 90 fresh processes per executable produced
identical SVG text after stdout decoding and UTF-8 encoding. Three alternating A/B repetitions, each with 30 samples
per executable, confirmed lower median latency in every repetition. Filesystem
caches were warm, not flushed; this is not cold physical-I/O evidence.

| Repetition | Baseline median, ms | Candidate median, ms | Baseline p95, ms | Candidate p95, ms |
| --- | ---: | ---: | ---: | ---: |
| 1 | 2010.25 | 1350.71 | 2291.40 | 1552.15 |
| 2 | 2127.54 | 1489.91 | 2268.41 | 1593.06 |
| 3 | 2094.92 | 1485.48 | 2348.03 | 1600.17 |
| All 90 samples | 2091.91 | 1468.03 | 2284.75 | 1593.06 |

The aggregate median decreased **623.87 ms (29.82%)**. Every repetition passes
the preregistered joint threshold of more than 10% and 50 microseconds.
These repeated same-host observations are not a formal cross-host or
distribution-free statistical guarantee. p95 uses nearest-rank quantiles.
Executable size is unchanged at **49,643,008 bytes** in the `release` profile;
neither packaged size nor thin-LTO `dist` size was measured.

## Cause And Preserved Policy

The only performance change is in the bounded system-font reader: reserve the
already-admitted metadata length before reading, and use a 64 KiB block instead
of a 16 KiB block. The baseline grew its Vec incrementally. This removes repeated
growth and copying without removing full SHA-256, source/ancestor checks,
actual-read byte limits, or per-block cancellation checkpoints. Allocation
failure from the initial reserve is fallible. Selected files are still verified
and materialized as shared binary snapshots for measurement and painting.

An isolated diagnostic admitted 541 unique files / 564 faces, read 779,458,188
font bytes and retained 6,552,878 cmap bytes on this host. A separate second pass
attributed roughly 1,269 ms to baseline reads and 505 ms to hashing, versus
560 ms and 438 ms with the 64 KiB candidate. Metadata parsing and cmap extraction
were each about 2-4 ms. Those diagnostic pass times do not add up to the public
CLI timing and are not independent admission evidence. A 256 KiB block did not
meaningfully improve catalogue time and was not retained.

A separate three-round live-process diagnostic sampled Windows
`Process.PeakWorkingSet64` after `Refresh()` every 10 ms. Observed peaks were
81,829,888 / 81,793,024 / 81,793,024 bytes for the baseline and
51,073,024 / 51,093,504 / 51,089,408 for the candidate. These are observed
process working-set high-water marks, not Rust allocated bytes or guaranteed
exit-time peaks. They show no observed memory regression, not a portable memory
improvement admission. See [raw memory observations](evidence/native-system-discovery-2026-10-10/memory-samples.json).

## Reproduction And Identity

Host: Windows 11, OS build 26100, Intel i7-11700 (8 cores / 16 logical threads).
Toolchain: Rust 1.95.0 (`59807616e`), Cargo 1.95.0 (`f2d3ce0bd`),
`x86_64-pc-windows-msvc`. Build each snapshot with
`cargo build --locked --release -p merman-cli`, using the CLI's source defaults:
`all-diagrams,analysis,ascii,icons,jpeg,layout-cytoscape,layout-elk,markdown,math,native-fonts,network-icons,parallel-markdown,pdf,png,rustdoc,shell-completions,svg,system-clock,system-fonts,system-random,system-timezone,system-timing`.
This recipe uses usvg 0.47 and fontdb 0.23. Source files differ from the bare
base commit; a bare checkout of `292826512` does not reproduce these binaries.

| Artifact | SHA-256 |
| --- | --- |
| Baseline executable | `3aa3cca698afd6bd2101ebea65f6f29f8bdd58fd82dc49688993d66f1c36d2f2` |
| Candidate executable | `961b06fabb138bd1e84f1ce3242069b75df3f4eb71c6f761bad3e3a4a6a8086e` |
| Lockfile | `77c66b401c6e1400b3e4868b1dd6e2f8092abecbb1447df440f6a93912c90631` |
| Input | `072a84880cd2df110285ff106a8caa6cf6873082750296e005e988e85df45944` |
| Every measured SVG | `97a476b792e1c1773ae7893cb7f1ce0278cc766c88faec874463a5ccd8d7cd84` |

[Versioned raw samples](evidence/native-system-discovery-2026-10-10/samples.json)
contain all 180 successful process measurements and identities. The checked-in
[PowerShell harness](evidence/native-system-discovery-2026-10-10/measure.ps1)
owns three warmups per executable, AB/BA sample order, stdout/stderr draining,
and the UTF-8 text-identity gate. The timing harness does not hash the original
stdout byte stream. Separate raw-byte controls cover Flowchart, ER and Sequence;
all three match between the preserved executables. Their
[raw stdout hashes](evidence/native-system-discovery-2026-10-10/raw-output-controls.json)
come from `StandardOutput.BaseStream.CopyToAsync(MemoryStream)` without text
decoding; they are correctness controls, not additional timing samples.
Run it without concurrent builds or other benchmarks:

```powershell
pwsh -NoProfile -File docs/performance/evidence/native-system-discovery-2026-10-10/measure.ps1 -Baseline path/to/baseline.exe -Candidate path/to/candidate.exe -InputPath docs/assets/native-font-measurement/flowchart.mmd -OutputPath target/bench/system-font-samples.json
```

The baseline reader is reconstructed by removing the initial
`try_reserve_exact(metadata.len())` and changing its block back to 16 KiB in an
isolated copy of the native-font implementation; leave all other policy and
capabilities identical. Installed font binaries are not redistributed. Their
set, storage, host load and OS cache affect timing and default font selection;
exact timings or SVG hashes on another machine are not promised.

The candidate was measured after the native-only warning fix. The later CLI
capability-report correction affects slim feature-union reports, not the full
default's effective flags or font reader; it is not included in the measured
executable identity above.

## Quality Evidence And Remaining Cost

The [same-Fira-Code Flowchart, ER and Sequence comparisons](../assets/native-font-measurement/README.md)
demonstrate why actual-font measurement is enabled: labels fit their containers
and wrap using the painted font. They are a separate controlled explicit-font
lane, not the unspecified-font system-discovery benchmark above.

The [earlier explicit-font checkpoint](native_font_measurement_2026-10-10.md)
separately records compiled/inactive versus explicit active cost. Its few-ms
delta does **not** describe the new default's system discovery. Library/FFI
measurement defaults remain host-independent; raw-SVG fonts affect painting
only, and deterministic CLI layout retains its compatibility route.

Default discovery still costs roughly 1.5 seconds on this font-heavy host.
This change does not solve full-catalogue acquisition. Future work should
investigate a content-correct catalogue/snapshot design; it must not replace
font-byte identity with path/mtime, discard resource checks, or silently change
fallback selection. Explicit files remain the bounded, reproducible alternative.
