# Canonical root theme outcomes — macOS, 2026-10-18

## Decision

Commit `fb2823f02809bc279fd0f4f58e7f45ea96d11cab` keeps one renderer-private sequence of root
mechanism outcomes: identity, requested capabilities and capabilities actually emitted by the SVG
writer. Terminal mechanism counts and the applied-capability union are computed once. Residual
diagnostics borrow identities from that sequence; an operation-wide output-mutation flag supplies
their reason after an untrusted transformation.

This removes the evaluation flag, two cloned key inventories, owned residual records, duplicated
residual capability sets/reasons and repeated coverage-set reconstruction. The report no longer
revalidates representations derived from its own immutable records. Construction owns unique
mechanism identities and nonempty requirements; the actual writer still rejects unknown keys and
capabilities outside a mechanism's requirements. Partial emission receives no complete-mechanism
credit, but still contributes its actual applied capabilities. Repeated marks replace prior
emission. An untrusted transformation invalidates every applicable mechanism, while an empty plan
remains NotApplicable. Public verification variants, diagnostics/source/order, strict rejection
counts, capability union, resource limits, cancellation and SVG output policy remain.

Tests that manually assembled contradictory private report fields are removed with those fields.
Their replacements exercise rejected/partial/repeated writer marks and output mutation through the
real application builder. The equal-capability two-layer test now renders and parses actual SVG,
checking both layer identities, colors and order. No dependency, cache or public API is added.

Admission is an exact complete-public-operation allocation improvement for explicit root mechanisms.
No calibrated latency improvement is claimed. Adjacent base is `d423394f4760286a237829b13a2cbce6f9afcab4`,
whose runtime is unchanged from root-style commit `5d0b80e8b6cb5c5559309ca1c26bcdc5adf0f7e0`.

## Public controls

All 178 inputs preserve SVG bytes, captured public resource/theme reports and ordered custom
measurement callbacks. All 152 prepared-render/E2E benchmark SVG identities match. Five complete
memory samples per input remain recorded on each side. Of 173 inputs stable on both sides, 30
remove 12 allocations and 672 cumulative bytes, and 143 remain unchanged. Five stable inputs also
reduce peak growth by 192 bytes; retained bytes match for every stable input. Five variable
Flowchart inputs remain memory-inconclusive; their medians are not admitted as improvements.
The original 86 core inputs are all stable: 18 reduce allocation and 68 remain unchanged.

For example, explicit solid-canvas Zenuml changes from 3,263 to 3,251 allocations and 611,212 to
610,540 cumulative bytes; peak growth of 270,785 and retained growth of 9,780 bytes remain equal.
Default Zenuml, with no applicable root mechanism, remains at 466 allocations and 152,774 cumulative
bytes, with unchanged peak/retained growth of 95,912/9,555 bytes. This batch does not remove the
remaining default-request release gap.

The independent warm compiled-preset lane covers 38 scenes across all 35 logical families, each
with no compiled theme and all 10 public presets: 418 cells. Each cell uses a fresh process, one
warm-up and five complete public rich SVG operations; preset compilation, file IO and report
serialization stay outside capture. The compiled preset is reused within its cell, so this does
not measure first compilation.

All 418 cells succeed, with matching emitted family, SVG bytes, public reports and outcomes.
All captures are stable with valid counters. The 380 preset cells remove 12–16 allocations and
672–1,048 cumulative bytes per operation; all 38 no-theme cells remain unchanged in all four memory
metrics. Peak growth decreases in 25 cells and remains equal in 393; retained growth always matches.
Public theme statuses remain 37 NotApplicable, 229 Residual, 72 Verified and 80 Incomplete.
Partial support, fallback, diagnostics and rejected admission reasons remain visible; these cells
do not establish strict portability or feature equivalence with v0.9.0.

## Timing context

The fixed adjacent diagnostic runs 40 calls: prepared SVG emission and complete E2E for five
families, two AB/BA pairs per row, 30 Criterion samples, 2 s warm-up and 3 s measurement. All
preflight identities and postflight checks match. No selected public E2E row exceeds both the
registered 10% and 1 us regression screen. There is no A/A calibration or latency admission.

| Family | Public E2E base → head (us) | Change |
| --- | ---: | ---: |
| Zenuml | 23.2805 → 23.4470 | +0.72% |
| Packet | 10.8745 → 11.2915 | +3.83% |
| Timeline | 42.4650 → 42.8130 | +0.82% |
| State | 429.2700 → 426.9350 | -0.54% |
| ER | 406.5700 → 406.1600 | -0.10% |

A separate four-call same-SVG v0.9.0 Zenuml diagnostic observes 20.1670 → 23.5470 us (+16.76%).
Keep that head estimate separate from the adjacent lane and the later
[19-scene release-context refresh](theme_v090_same_svg_refresh_macos_2026-10-18.md). The tag has
different error/resource contracts; none of these diagnostics isolates theme costs or proves
release convergence.

## Verification and remaining owner

Focused root/SVG/application tests: 62 passed. Optimized native six-package tests: 8,254 passed,
13 skipped. Five-package all-features/all-targets Clippy with warnings denied, 34 private theme
qualification tests, 47 WASM host tests, minimal SVG features, optimized full WASM, 37-diagram smoke,
all 37 selected DOM groups in structure/parity/parity-root modes, and formatting pass. Independent
correctness, security and maintainability reviews report no findings or necessary test gaps.
This is not a complete strict release/transport sweep or browser pixel-parity claim.

The root-report inventory/self-coverage candidate is complete. Continue with separately attributed
consumer work on the remaining same-SVG release gaps; Packet and Kanban retain large relative
diagnostic gaps, while Class tiny has a larger absolute gap. Configured duplicate CSS normalization
remains a separate candidate requiring actual policy/order counterexamples. Do not assign the
remaining default-path latency to this explicit-root allocation repair. The all-family goal remains
active, including changed-output inputs, partial preset support and the need for calibrated evidence.

Pipeline SHA-256: `d1cad7c938b5d9057960f09dd30aa92d91b8eeb36d63c0cbf5d81177d7689be4`.
Raw registration, both-side allocation samples, callback receipts, discovery, artifact/source
digests, timing logs, reviews and validation are under
`target/bench/experiments/root-theme-outcomes-20261018/`. The catalog comparison is
`target/bench/experiments/theme-catalog-coverage-20261018/rootoutcomes-comparison.json`, with full
captures under `rootstyle/` and `rootoutcomes/`. Host: Apple M4 Pro/48 GiB, macOS aarch64,
Rust 1.95.0. Cargo, allocation capture and timed processes ran serially.
