# Packet static style ownership (macOS, 2026-10-19)

Packet stylesheet fallback values now borrow their immutable literals. Configured values keep the
same JSON projection, delimiter normalization and CSS safety predicate; selected typed paints
still own their replacements. This removes allocations without introducing a cache or changing
validation, emission, measurement, work accounting or cancellation boundaries.

The registered allocation experiment is under
`target/bench/experiments/packet-static-style-20261019/`, based on product `4db8cb3f4`. All 197 public
controls preserve exact SVG/report bytes and ordered custom-measurer traces. Of 190 jointly stable
memory rows, 16 reduce allocations and bytes; seven variable Flowchart rows remain inconclusive.
Configured strings/numbers that are accepted remain unchanged. Missing, null, non-string and unsafe
values still fall back as before, now without allocating the fallback literal.

Default Packet changes from 127 to 117 allocations and 19,207 to 19,162 allocated bytes per complete
public render. Peak growth changes from 10,193 to 10,148 bytes; retained growth remains 3,855 bytes.
All 418 catalog cells across 35 logical families and 10 presets plus no-theme preserve exact SVG,
report, configuration, status and catalog metadata. All cells are stable. The 11 Packet cells each
save ten allocations and 45 allocated bytes; other cells stay unchanged, with no peak or retained
growth regressions. The catalog's different diagram ID/report retention gives different absolute
byte metrics from the control probe; compare each lane only with its own baseline.

The first catalog attempt stopped before collecting samples because a worktree-external input path
could not be made relative. A subsequent shared-target baseline accidentally reused candidate
libraries; equal library digests exposed that invalid comparison. Both attempts remain recorded,
and neither supports admission. The accepted catalog comparison uses a clean detached baseline and
an isolated target directory. The original pre-edit public baseline probes remain valid.

Renderer nextest passes 5,201 tests with five skipped; release all-feature Clippy, formatting and
diff checks pass. Existing public Packet theme/resource tests are also run. No latency or v0.9.0
recovery claim is made. These literal allocations existed in v0.9.0; their removal reduces shared
cost rather than explaining a newly introduced theme regression.
