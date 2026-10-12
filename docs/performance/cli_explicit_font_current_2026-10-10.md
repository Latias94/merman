# Current Explicit-Font CLI Cost

> Current policy (2026-10-12): ordinary CLI Mermaid graphical output defaults to
> deterministic measurement without a native font context or system discovery for
> layout. PNG/JPEG/PDF painting can still load system fonts through the existing codec
> path. Default/release profiles still compile native/system context support. Explicit
> files/directories enable native measurement with a closed font set;
> `--font-measurement native` without sources or
> `--font-system on` explicitly requests system discovery when available. The original
> executable defaults, measurements, and results below are historical and unchanged;
> this note does not claim a new timing run. See the [current CLI font guide](../../crates/merman-cli/README.md#explicit-fonts).

Date: 2026-10-10, Asia/Shanghai. Windows 11 build 26100, Intel i7-11700,
Rust 1.95.0, Cargo 1.95.0, x86_64-pc-windows-msvc. This checkpoint uses the
current uncommitted implementation, not published 0.8.0. Build with
`cargo build --locked --release -p merman-cli` and its source defaults;
this is not the thin-LTO `dist` profile. The executable is 49,643,008 bytes,
SHA-256 `9399abf8e75e7d0317f85724ee6bdb11759d6177a6e0bcc9e00bddccaacd3c72`.

## Results

One executable renders the saved user Flowchart, ER and Sequence inputs using
either deterministic measurement or explicit native Cousine-Regular measurement.
Both use the same versioned configuration with HTML labels disabled. Deterministic
mode opts out of automatic discovery; native mode uses the checked-in font and
system discovery off. Timing includes startup, acquisition, rendering and output
capture, not merely shaping. The two policies intentionally produce different
geometry. No painting/font embedding is performed for SVG output.

| Input | Deterministic median, ms | Native median, ms | Delta, ms | Delta | Deterministic p95, ms | Native p95, ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Flowchart | 58.74 | 64.75 | +6.01 | +10.24% | 76.58 | 72.48 |
| ER | 58.08 | 61.23 | +3.15 | +5.43% | 70.72 | 78.51 |
| Sequence | 57.41 | 60.21 | +2.80 | +4.88% | 73.78 | 70.56 |

Each cell aggregates 90 fresh processes per policy: three repetitions of 30
alternating AB/BA samples after three warmups per input/policy. All 540 measured
processes exited successfully with nonempty stdout. Each input/policy has one
stable raw stdout SHA-256 across warmups and measurements. p95 is nearest-rank.
Filesystem caches were warm, not flushed. Families were measured in separate
blocks; cross-family times are not paired controls.

| Repetition | Flowchart deterministic/native, ms | ER deterministic/native, ms | Sequence deterministic/native, ms |
| --- | ---: | ---: | ---: |
| 1 | 59.23 / 65.86 | 59.02 / 61.78 | 57.82 / 60.31 |
| 2 | 59.34 / 64.60 | 57.93 / 61.54 | 57.35 / 60.27 |
| 3 | 57.92 / 64.32 | 57.75 / 60.76 | 57.31 / 60.21 |

The native median is higher in each repetition. Mixed p95 directions are retained,
not interpreted as tail improvements. These descriptive same-host results do not
prove portable overhead or a statistically admitted speedup. They do not measure
compiled-feature footprint, reused-context library throughput or system discovery.

## Reproduction

The [versioned harness](evidence/cli-explicit-font-current-2026-10-10/measure.ps1),
[configuration](evidence/cli-explicit-font-current-2026-10-10/config.json) and
[540 raw observations](evidence/cli-explicit-font-current-2026-10-10/samples.json)
retain executable/font/lockfile/config/input/harness hashes, each sample's
timestamp, exit code, byte count and raw output hash. The
[harness documentation](evidence/cli-explicit-font-current-2026-10-10/README.md)
describes the exact lane commands and controls. Run without concurrent builds:

```powershell
cargo build --locked --release -p merman-cli
pwsh -NoProfile -File docs/performance/evidence/cli-explicit-font-current-2026-10-10/measure.ps1 -Cli target/release/merman-cli.exe -OutputPath target/bench/cli-explicit-font-current-samples.json
```

The output parent directory must exist. The user inputs are versioned under
`docs/assets/native-font-measurement/`; Cousine-Regular is the existing test font,
SHA-256 `1da22250675fc4c42fcf3a9736c44bc0570516105331443b663fd5cfbd1412fe`.
These are Cousine timing controls, not the Fira Code visual comparison. New builds
can change executable hashes and timings; the dirty source snapshot has no release
identity. Exact historical binary reconstruction is not promised.

See the separate [system-discovery optimization](native_system_discovery_2026-10-10.md)
for the default no-font path, the
[historical compiled-capability checkpoint](native_font_measurement_2026-10-10.md)
for older footprint data, and the
[same-Fira-Code visual evidence](../assets/native-font-measurement/README.md)
for label containment. None of these lanes substitutes for another.
