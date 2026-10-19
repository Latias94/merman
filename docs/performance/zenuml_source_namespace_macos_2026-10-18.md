# Zenuml source configuration ownership — macOS, 2026-10-18

This checkpoint resolves Zenuml source configuration once instead of repeating full root paths
for every role. Its adjacent base is `0ca9b4145` (runtime equal to `0ebf21cb2`). Admission is exact
duplicate-work removal and reduced unused configuration storage, with semantic and allocation
controls. No calibrated latency improvement or theme-only release-gap attribution is claimed.

## Configuration ownership

`ZenumlSourceStyle` already owns the source values used by both layout and drawing. It now locates
`themeVariables.zenuml` once and requires an object before reading members. Missing/non-object
namespaces produce the same default typography and empty color map. Present namespaces preserve
all authored values and fallback priorities; absence of a compiled theme does not skip source
configuration. The private typography constructor accepts the same resolved object rather than
independently walking the complete path again. Existing layout tests use this owning constructor
or the equivalent default typography; no old assertion is removed or weakened.

For a missing namespace, the previous constructor issues 19 direct color, 15 fallback and two
font/size path requests. The new constructor makes one namespace lookup and no local role reads.
For a present empty object, old paths perform 108 `Value::get` calls; the new path performs two
namespace gets and 35 local gets. These are source-level counts, not instrumented CPU timings.
The role count is fixed, and both implementations retain the same asymptotic behavior with
respect to authored configuration. No general JSON lookup complexity improvement is claimed.

The private color map also stops storing the standalone `surfaceColor` String: it has no terminal
consumer. That raw key remains a fallback input for actor/note/group/muted/activation backgrounds.
`activationBackgroundColor` keeps its original priority: direct value, raw `mutedSurfaceColor`,
raw `surfaceColor`, then the writer's terminal fallback. It does not read a partially populated
resolved map. Empty strings remain explicit overrides; non-string values fall back. An empty
font family remains authored, retaining the 14px message measurement branch. Font size retains
the existing numeric/CSS-pixel parser and positive-value filter, including existing large-size
geometry rejection. No shared writer, resource/cancellation/error boundary, cache, dependency,
unsafe code or persistent state changes.

## Public controls

Before production edits, the preceding 74-input corpus is extended with 12 direct public
site-config inputs: null/array/string/number/empty namespaces, surface and muted inheritance,
empty/non-string role overrides, font/size and explicit activation priority. Both frozen sides
render all 86 cases successfully. The older source-init controls remain recorded without being
counted as effective authored-font coverage; actual site-config cases supply that coverage.

All 86 SVGs and theme reports match byte for byte. All ordered custom text-measurement callbacks
and output match. Five-repeat allocation metrics are stable on both sides: six inputs reduce
allocation count and cumulative bytes, 80 remain unchanged, no peak growth increases and every
retained-byte count matches. All 152 default benchmark SVG identities also match.

| Public source configuration | Allocation count, base → candidate | Allocated bytes saved |
| --- | ---: | ---: |
| Default medium Zenuml | 471 → 471 | 0 |
| Surface only | 525 → 524 | 7 |
| Surface plus muted | 525 → 524 | 7 |
| Empty role overrides | 536 → 534 | 1,287 |
| Non-string role overrides | 540 → 538 | 1,287 |
| Authored font/size plus surface | 529 → 528 | 7 |
| Explicit activation priority | 525 → 524 | 7 |

This does not remove default allocations: default configuration already produced an empty map.
Its primary benefit is the exact repeated-path work removal. Configured `surfaceColor` removes
one unused String; the two larger byte reductions are consistent with avoiding a BTreeMap node
allocation at its entry-count boundary, an inference from the representation and metric deltas
rather than an allocation-stack capture. This secondary effect is observed on the
registered inputs, not a general per-request memory guarantee.

## Latency and release context

The fixed adjacent diagnostic uses six rows, two balanced AB/BA pairs, 30 Criterion samples,
two-second warmup and three-second measurement: 24 invocations. Preflight/postflight output
identities match for every row. Values below are medians of two estimates per side.

| Operation | Adjacent base, us | Candidate, us | Observed change |
| --- | ---: | ---: | ---: |
| layout/zenuml_medium | 4.6143 | 3.8969 | -15.55% |
| end_to_end/zenuml_medium | 24.7250 | 24.1535 | -2.31% |
| layout/packet_medium | 1.4923 | 1.5111 | +1.27% |
| end_to_end/packet_medium | 11.4145 | 11.3715 | -0.38% |
| layout/timeline_medium | 20.8745 | 20.8070 | -0.32% |
| end_to_end/timeline_medium | 44.0730 | 43.7930 | -0.64% |

No public E2E row triggers the registered greater-than-10% AND greater-than-1-us regression
screen. The default preparation change is consistent with the earlier source-style hotspot and
exact duplicate-path removal, but stage medians are not additive. These measurements are
diagnostic: there is no A/A calibration or confirmed latency claim.

Separate same-SVG v0.9.0 public E2E context measures Zenuml 20.1550 → 24.2200 us
(+20.17%). That four-invocation release-range comparison retains different
error/resource contracts and no A/A calibration; it is not an all-family ratio or a theme-only
causal attribution.

## Verification and remaining work

Three independent read-only reviews cover source fallback contracts, correctness and
maintainability/static work counting. They report no required changes. The existing inheritance
test gains empty/non-string and activation-priority cases. Two tests cover malformed/isolated
namespaces and authored-empty/invalid typography; existing CSS encoding, theme, custom measurement
and resource tests remain. The focused Zenuml suite passes all 38 tests.

Validation passes:

- Six-package all-features release nextest: 8,252 passed, 13 skipped.
- Five-package all-features/all-targets Clippy with warnings denied.
- Private theme qualification: 34 passed; WASM host tests: 47 passed.
- Minimal SVG/Zenuml/Treemap/Venn feature compilation.
- Optimized full Web WASM and 37-diagram runtime smoke.
- DOM comparisons across 37 family groups in structure/parity/parity-root modes.
- Formatting and diff checks.

Root viewport state retaining unused fields after emission and the extra intermediate Engine clone
at the public facade remain separate candidates. They are not implemented or credited with this
batch's measured work removal. Choose the next batch by fresh remaining-owner evidence rather
than subtracting independent stage medians or inclusive sample shares.

Production commit: `d4b495cea5488b0cebe03ea1de61bec98af462ba` (`perf(zenuml): resolve source style namespace once`). Pipeline SHA-256: `9afb5e5b39a890712d430f006f349589964e3d9d7e636adf40370cc0db7a2199`.
Raw preregistration, frozen corpus/source/artifact digests, static work proof, five-repeat allocator
captures, custom measurement traces, SVG/report controls, diagnostic samples and verification logs
are under `target/bench/experiments/zenuml-source-namespace-20261018/`.
Cargo, allocator capture and timing processes run serially on Apple M4 Pro, 48 GiB, macOS aarch64,
Rust 1.95.0. Previous XML A/A remains inconclusive and is not retried. No full strict
release/transport-matrix certification is claimed.
