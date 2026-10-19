# Packet and Kanban preparation — macOS, 2026-10-17

The adjacent baseline is `c0609d418828a88019ca33566e6a955b62979949`. Runtime changes are
`1151b41cc` (Packet) and `feefbf006` (Kanban/shared typography). This batch removes
unused preparation work while preserving the public SVG and theme-report contracts. Its
preregistered admission criterion is exact allocation reduction on complete public operations,
not calibrated latency improvement. No dependency, cache, unsafe code, production instrumentation
or new rendering policy is introduced.

## Ownership changes

Packet freezes compatibility evidence only when residual fields exist and a resolved theme can
consume them. Terminal reconciliation still examines the original metadata residual count:
preparation can provisionally defer fields that the actual writer must subsequently confirm or
restore. Portable-theme admission and writer-owned consumption evidence remain intact.

Packet's configured style values already have code-owned property names. They now reuse the
owned JSON-projected value, perform the existing second trailing-delimiter normalization in
place, and call the unchanged core CSS value predicate directly. This removes declaration
assembly, fixed-property parsing and comparison, and a second owned-value copy. User-provided
values retain validation, fallback behavior, and the existing structural safety policy. Default
fields already returned their literal fallback without declaration parsing; this change does
not explain a large default-Packet latency gap.

Kanban passes its resolved diagram font to the common CSS preparation instead of resolving it
again. Ordinary root-font CSS still resolves independently. Typed typography retains its existing
behavior, including binding the root font to the diagram font when only typed font size is
requested. Unconsumed typed-size strings and size-source lookups are omitted. Ordinary CSS and
layout size fallbacks remain distinct, preserving zero/negative sizes, string root sizes, and
number formatting. Shared inherited-font resolution now parses the configured font only when
it wins over the typed source.

These changes remove repeated or unused work at existing owners. They do not establish a new
asymptotic bound, remove sanitization, or bypass resource and measurement responsibilities.

## Exact allocation evidence

Before production edits, the experiment froze 77 public-render inputs and two characterization
tests. The tests pass on both baseline and candidate: Packet normalization/injection cases and
Kanban's independently configured root/diagram fonts with and without typed size.

The corpus covers Packet/Kanban typography modes, three looks, font and size overrides, empty,
quoted, Unicode, long and hostile values, Packet declaration injection, independent Kanban root
fonts, and scales of 1/16/64 fields or items. Timeline, Class, Flowchart, ER, Mindmap and State are
unaffected-family controls. Each input has five identical allocator observations per executable,
using the existing counting System allocator and retaining the SVG/report through the observation
boundary. The renderer and compiled theme are reused; every operation creates a fresh request,
operation control, render session and output. Renderer construction and theme compilation occur
outside the measurement.

Review also requested a root-only size fallback test. The frozen baseline public probe confirms
that an explicitly blank `themeVariables.fontSize` is needed to exercise that fallback instead
of the default theme's `16px`: numeric root `20` emits `20px`, while root `"20px"` emits `16px`.
A separate integration test preserves those literal outputs. This followup does not replace or
extend the 77-input preregistered memory decision.

All 77 SVGs and public theme reports are identical. Allocation count and cumulative allocated
bytes decrease in 69 inputs and remain unchanged in eight. No input increases peak live-byte
growth or changes retained bytes. All six unaffected-family controls match exactly. The other
two unchanged inputs exercise configured Kanban fonts with typed size, where the old path already
avoided diagram-font reparsing. Counters describe heap operations, not RSS.

| Complete public operation | Allocations, base → candidate | Allocated bytes saved | Peak growth saved | Retained bytes delta |
| --- | ---: | ---: | ---: | ---: |
| Packet medium, no typed theme | 135 → 134 | 16 | 0 | 0 |
| Packet, typed font stack | 2,983 → 2,953 | 540 | 0 | 0 |
| Packet, typed font size | 3,028 → 2,982 | 1,124 | 0 | 0 |
| Packet, both typography fields | 2,992 → 2,962 | 540 | 0 | 0 |
| Packet, declaration injection | 1,909 → 1,906 | 50 | 0 | 0 |
| Kanban medium, no typed theme | 671 → 665 | 187 | 4 | 0 |
| Kanban, typed font stack | 3,636 → 3,631 | 214 | 0 | 0 |
| Kanban, typed font size | 3,685 → 3,682 | 145 | 0 | 0 |
| Kanban, both typography fields | 3,641 → 3,638 | 145 | 0 | 0 |
| Kanban, 2,048-character font input | 2,486 → 2,479 | 4,241 | 4 | 0 |
| Kanban, 64 items | 3,895 → 3,889 | 187 | 4 | 0 |

A separate preparation probe reuses the parsed model and creates a fresh session/artifact per
iteration. Five observations agree: Packet allocations decrease 32 → 31 and bytes 5,732 → 5,716;
Kanban decreases 416 → 410 and 37,021 → 36,834. Timeline remains 614 and 35,242. This attributes
the public reductions to preparation; its retained-artifact boundary is not the public-output
memory boundary.

Baseline macOS samples use the existing profiling example's `layout` stage: parse once, then
repeatedly prepare and drop a fresh artifact. Nine-second runs contain five seconds of 1 ms
sampling. Kanban stacks reach font normalization both through typography preparation and common
CSS preparation. These profiles demonstrate reachable duplicate work, not a claimed percentage
of total rendering time. Packet's default evidence copy is small, as the exact counters confirm.

## Diagnostic timing and validation

The unchanged Criterion pipeline verifies 152 render/end-to-end SVG identities plus two
Packet/Kanban preparation identities. Seven registered timing rows run in two pairs, AB then BA:
28 invocations, 30 samples, two-second warmup and three-second measurement. Each invocation
verifies its output before and after timing. The table contains medians of two observations per
executable, not independent Criterion subsamples.

| Operation | Baseline, microseconds | Candidate, microseconds | Observed change |
| --- | ---: | ---: | ---: |
| layout/packet_medium | 1.491 | 1.470 | -1.43% |
| render/packet_medium | 4.534 | 4.446 | -1.96% |
| end_to_end/packet_medium | 11.3935 | 11.2010 | -1.69% |
| layout/kanban_medium | 18.7810 | 17.7785 | -5.34% |
| render/kanban_medium | 31.816 | 31.049 | -2.41% |
| end_to_end/kanban_medium | 61.7385 | 59.9925 | -2.83% |
| end_to_end/timeline_medium, control | 42.6150 | 42.8165 | +0.47% |

`layout` includes fresh preparation on an already parsed model. `render` prepares outside its
timed closure, so its observed change cannot be attributed directly to removing preparation
work. These are diagnostic observations without A/A calibration, confidence-based admission or
a confirmed speedup/regression claim. The exact allocation gate, not this table, admits the
candidate.

Scoped validation of the frozen production candidate:

- Six-package, all-feature release nextest: 8,244 passed, 13 skipped. Private theme qualification:
  34 passed. WASM host tests: 47 passed.
- Fresh optimized full Web WASM build and runtime smoke: all 37 diagram families, SVG/ASCII,
  deterministic and host-callback measurement routes pass.
- All-family SVG checks pass in structure, parity and parity-root modes. The existing comparator
  explicitly reports its browser-text-layout and parser-diagnostic residuals; these are not new
  exact-DOM parity claims.
- All-target/all-feature Clippy passes across five affected packages. The minimal facade build
  with only `svg,diagram-packet,diagram-kanban` passes. Formatting and whitespace checks pass.

The root-only size test was added after these gates and after timing, without changing production
Rust sources. All 40 Kanban SVG integration tests pass after that addition, and the five-package
Clippy command passes again. Static review covers all eight changed source/test files through
correctness, standards, testing, maintainability, security, performance, API-contract and
adversarial lenses; the added test closes the sole coverage suggestion. Simplification review
finds no worthwhile further cleanup and preserves the independent proof tests.

The previously stopped 661-build release/transport sweep is not rerun; full strict release
verification is not certified by this scoped runtime batch.

## Reproduction and limits

The raw ledger, frozen corpus, five-repeat samples, SVG/report pairs, profiles, compiler artifact
manifests, validation commands and diagnostic logs are under
`target/bench/experiments/packet-kanban-preparation-20261017/`. They are local experimental
artifacts; the production benchmark and allocator implementation are unchanged.

The frozen baseline pipeline SHA-256 is
`9d6831573eec37ec5a823b097d26c82e11f53251fb58d8b923f7960f14433180`; candidate is
`4cfd0c3c05ec147274b79de83e55a895932ecf82dfc9e30e3646c92745da4a0f`.
Library/compiler manifests and source hashes tie the independently linked allocation and
preparation probes to their runtime revisions. The baseline public probe is the prior checkpoint's
frozen final candidate, whose runtime matches this documentation-only baseline revision.

Host: Apple M4 Pro, 48 GiB, macOS aarch64, Rust 1.95.0. Cargo runs serially with up to six jobs.
Timing and sampling exclude concurrent Cargo and allocator runs. The baseline public allocator
capture overlapped compilation of baseline characterization tests; only deterministic counts
and output equality are used from that capture, never elapsed time.

No new v0.9.0 comparison or release-range recovery claim is made. The prior shared-XML
confirmation remains inconclusive; this batch does not retry its calibration or reinterpret its
diagnostic ratios. The admitted result is removal of unused work with exact public allocation
reductions. Remaining default-Packet and Kanban latency gaps require separate attribution.
