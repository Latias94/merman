# Playground startup investigation (2026-10-01)

## Decision

The initial-render scheduling candidate is **inconclusive and not integrated**. It moved the first
facade-ready render ahead of the normal 300 ms debounce, but neither registered calibration passed.
There is no confirmed cold-start speedup or editor-readiness non-regression claim.

The baseline is `bd5d64689a9f6bb32791c08d79f70fb2358fb50b`; the unadmitted candidate is
`e03e21dc038603d22e2e6cde05346687dafcde58`. Both were built from clean isolated worktrees with
identical dependency locks, WASM, opaque-realm inputs, default source, and output SVG. The local
ledger, raw samples, profiles, and analysis commands remain under
`target/bench/experiments/playground-startup/`.

## Workload and evidence

Windows 11 x64, Intel i9-13900KF, Node 24.21.0, Chromium Headless Shell 151.0.7922.34; production
Playground on localhost at 1440 x 900, English locale, light theme, default Flowchart. Every raw
sample used a fresh browser process and context. Timings start at navigation, exclude browser
process launch, and do not model a real network or an empty operating-system file cache.

The primary metric was `max(editorReadyMs, previewReadyMs)`: an attached, writable Monaco input
and the existing first-preview mark after SVG mounting with nonzero dimensions. These are DOM
readiness proxies, not proof of completed screen painting or measured first-keystroke latency.

Both protocols required improvement above 10% and 50 ms, with separate editor/preview controls,
eight A/A pairs per revision, and a fixed power-derived A/B budget of 8-32 pairs. Analysis reused
`tools/bench/compare_self.py`, 10,000 bootstrap resamples, seed 20261001, and simultaneous 95%
intervals for six comparisons. No profiler ran during calibration. Every raw sample retained
source and exact serialized-SVG identity checks; SVG SHA-256 was
`74dc0c9f48342d934992204a6ce915c8ebfc708b6e0021e36bb52b5cbae95b9f`.

- Protocol 1 collected 32 raw starts. A/A equivalence failed; the required confirmation count
  reached 442 pairs, above the registered cap. One initial main-thread task lasted 1,241 ms
  and ended immediately before the editor-readiness mark; that sample was not CPU-profiled.
- Protocol 2 was registered before collecting new samples. It warmed each server with two visits,
  then used the median of exactly three fresh-process starts per observation: 96 measured starts
  plus four warmups. It retained every raw value without trimming. Baseline A/A still failed the
  fixed margins: the primary metric's identity interval was approximately -114 to +4 ms, and its
  order interval -111 to +11 ms. The candidate calibrated stably, which does not repair the
  unstable baseline or establish a between-version benefit.

Neither protocol proceeded to A/B confirmation. Calibration medians must not be quoted as a
speedup, and no additional protocol was used to seek acceptance.

## Attribution and next boundary

Three separate profiled starts attributed long tasks to React mounting (53-55 ms), Monaco
construction (61-99 ms), and the first SVG render (115-121 ms). Profiling adds overhead; these
numbers are attribution evidence only. The observed facade-ready-to-render gap was 299-301 ms.
Removing that wait can move SVG computation into Monaco initialization, so a preview-only metric
cannot establish a workbench improvement.

Language workers already start after the initial desktop preview. Their separate execution realm
and SVG admission boundaries remain necessary. Monaco's additional font measurement after mount
accounted for about 1.6 ms of self time; this does not justify claiming that its entire first
layout can be removed. A future startup candidate needs stable calibration and both editor and
preview controls before integration.

The investigation also exposed an independent correctness defect: an explicit immediate request
queued behind active Compare work could lose its scheduling intent and wait for the ordinary
editing debounce. Fixing that request deadline does not require changing initial-render scheduling
and is not evidence that the rejected cold-start candidate improves latency.

## Separate request-deadline repair

Commit `7e8908d21` stores an execution deadline on each queued render request. Explicit refresh,
feature changes, resume, and re-enable keep their immediate intent while Compare settles; a later
edit replaces it with its own full debounce deadline. Nested pauses release only the latest request.
Initial facade readiness still uses the original debounce, and executions remain serialized.

Four deterministic regressions fail against the original coordinator and pass with this repair.
The final coordinator suite passes 42 tests; the complete affected runtime suite passes 124.
TypeScript, scoped ESLint, production build, license checks, and artifact-graph verification pass.
Thirteen browser regressions pass across Chromium desktop/mobile, Firefox, and WebKit desktop/mobile,
covering real editor input and undo, blocked-preview startup, Compare, hidden mobile previews,
workspace resizing, gallery state, and lifecycle cleanup. Independent source and mock-clock review
found no remaining correctness issue.

These checks validate the scheduling contract, not a cold-start speedup. No Rust source, WASM,
dependency, worker-activation policy, or SVG admission boundary changed. The earlier continuous
scrollbar flicker remains unreproduced; the bounded narrow-window stability regression passes.
