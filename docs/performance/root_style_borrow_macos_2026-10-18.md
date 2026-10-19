# Root style borrowing and compiled preset coverage — macOS, 2026-10-18

## Decision

Root style composition now borrows a nonempty existing style when no custom properties are
requested. The deferred `px;` suffix is also borrowed instead of briefly allocated. Actual
properties still construct an owned string and run the same validation/formatting loop before
writing. Empty/absent style omission, XML attribute encoding, append/checkpoint boundaries,
marker tracking, root snapshots, family/id checks, exact byte preflight and cancellation remain.
No tests or assertions are removed or weakened; existing real-property/deferred marker tests cover
the changed private representation. No dependency, cache, public API or feature change.

The primary admission claim is exact public-operation allocation reduction, not calibrated latency.
Adjacent base is `aebb04a56`, runtime equal to `3bf737d25`.

## Public controls

The 178-input corpus retains all previous source/theme/site-configuration edge cases, all 76
rendering benchmark inputs and explicit canvas/fixed configurations. All SVG bytes, resource/theme
reports and ordered custom-measurer traces match; all 152 pipeline SVG identities also match.
Of 172 stable memory rows, 168 reduce allocation count/cumulative bytes and four remain unchanged.
Six variable Flowchart rows remain memory-inconclusive with all five samples preserved. Every
stable input has unchanged peak growth and retained bytes. The original 86 core rows all stay
stable and reduce allocation.

Default Zenuml changes from 467 to 466 allocations and 152,794 to 152,774 cumulative bytes; peak
95,912 and retained growth 9,555 bytes are unchanged. Unchanged stable cases are Usecase, whose
real properties still require composition, and the intrinsic Zenuml/ER/Timeline configurations.

The new catalog lane is frozen and fully captured before production edits: 38 existing cross-family
scenes covering all 35 logical family IDs, each with no compiled theme and all 10 public presets,
418 cells total. Each cell is a fresh process with one warm-up and five complete public rich SVG
operations. Preset compilation, file IO and report serialization are outside memory capture; the
compiled preset is reused within that cell. This measures warm rendering, not first compilation.

All 418 baseline and candidate cells succeed with matching emitted family, SVG bytes and complete
captured public reports. All five-repeat memory captures are stable and counters valid. 407 cells
remove one allocation and 3–32 cumulative bytes; 11 Usecase cells remain unchanged. All peak growth
and retained metrics are identical. The status distribution remains 37 NotApplicable, 229 Residual,
72 Verified and 80 Incomplete. Residual/Incomplete and diagnostics are preserved as actual partial
support, not hidden or treated as a successful full-theme application. This matrix does not prove
strict portability or v0.9.0 feature equivalence.

## Timing context

Forty fixed diagnostic calls cover prepared SVG emission and complete E2E for five families, with
two AB/BA pairs per row, 30 Criterion samples, 2 s warm-up and 3 s measurement. All preflight SVG
identities and postflight checks match. No selected public E2E row exceeds the registered material
regression screen of both 10% and 1 us. No A/A calibration or stable latency improvement is claimed.

| Family | Public E2E base → head (us) | Change |
| --- | ---: | ---: |
| Zenuml | 24.3060 → 24.0120 | -1.21% |
| Packet | 11.3165 → 11.1450 | -1.52% |
| Timeline | 44.0995 → 44.0480 | -0.12% |
| State | 442.3950 → 442.4050 | +0.00% |
| ER | 415.8000 → 419.1700 | +0.81% |

A separate four-call same-SVG v0.9.0 Zenuml diagnostic observes 20.5995 → 24.0605 us
(+16.80%). Error/resource contracts differ; this is one uncalibrated release-context row,
not an all-family result, theme-only cause or calibrated recovery. Keep its head estimate separate
from the adjacent lane.

## Verification and remaining owner

Focused root/SVG tests: 36 passed. Optimized native six-package tests: 8,253 passed, 13 skipped.
Five-package all-features/all-targets Clippy with warnings denied, 34 private theme tests, 47 WASM
host tests, minimal SVG features, optimized full WASM, 37-scene smoke, all 37 selected DOM groups
in three modes and formatting pass. Independent correctness and security reviews find no issues
or necessary new tests. No complete strict release/transport sweep or browser pixel parity is claimed.

The next independent candidate is root-theme report ownership: it clones required/applied key
inventories, residual capability sets and repeatedly reconstructs coverage sets from its own
immutable records. These are renderer-private, while public status/diagnostics/counts and actual
writer/capability outcomes must survive. Verify the construction invariant and output-mutation
counterexamples before deleting self-certification. Default no-mechanism paths must remain
unchanged; this candidate applies primarily to explicit root/canvas requests.

Production commit: `5d0b80e8b6cb5c5559309ca1c26bcdc5adf0f7e0`. Pipeline SHA-256: `cd890369cbf2e0b9c62ad1abf5f200fefefef06f4773e8a5b53b4a110f7d6458`.
Raw controls, registration, measurements, final source/artifact digests, reviews and validation
are under `target/bench/experiments/root-style-borrow-20261018/`. The all-preset probe, frozen
sources/catalog and both captures are under `target/bench/experiments/theme-catalog-coverage-20261018/`
(`base` and `rootstyle`). Cargo, memory capture and timing ran serially on Apple M4 Pro/48 GiB,
macOS aarch64, Rust 1.95.0. The persistent all-family goal remains active.
