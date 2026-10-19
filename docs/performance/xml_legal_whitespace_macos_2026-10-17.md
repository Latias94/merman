# XML legal-whitespace classification and Zenuml residual costs — macOS, 2026-10-17

This checkpoint follows the Zenuml CSS encoding repair. The adjacent base is
`063ce0cf9` (runtime equal to `44fccd368`). It removes unnecessary per-byte XML state processing
for legal whitespace, after a fresh release-range diagnosis selected that owner. Admission is
exact redundant-work removal with same-output controls, not calibrated latency improvement.

## Release-range diagnosis before the change

Frozen v0.9.0/current pipeline executables use the identical benchmark source and medium fixture
bytes, default features plus SVG, strict parsing, deterministic measurement and trusted-native
resources. Ten rows receive two balanced AB/BA pairs each, 30 samples, two-second warmup and
three-second measurement: 40 invocations, with stable pre/postflight identities.

| Zenuml operation | v0.9.0, us | Pre-change current, us | Output identity |
| --- | ---: | ---: | --- |
| Reused-engine parse | 8.6797 | 9.2209 | Same render-model JSON |
| Fresh-engine parse | 12.3915 | 12.8265 | Same render-model JSON |
| Preparation/layout | 3.5922 | 4.5662 | Different layout projection; context only |
| SVG emission | 5.2332 | 9.7627 | Same SVG |
| Public end-to-end | 19.8395 | 27.0380 | Same SVG |

The largest observed stage difference is emission, +4.5295 us. Stage estimates are not additive,
and release-range error/resource contracts differ. Layout projections differ for both Zenuml and
Timeline, so those rows are not equivalent-output latency comparisons. The raw stage report's
inherited global same-output label is too broad; each row's `same_identity` field and this table
provide the actual boundary. Timeline control emission is 18.0885 → 14.5355 us, while public E2E
is 39.6975 → 43.1310 us; a common theme-only explanation does not follow.

The existing `profile_render` example, linked against verified current optimized libraries,
samples current render/layout/E2E separately: 12-second processes, batch 100, six-second macOS
`sample` captures at 1 ms. Its deterministic interactive resource profile differs from the pipeline
recipe. Whole-process render sampling includes fresh preparation; only `emit_prepared_svg` defines
the emission subtree.

| Inclusive sampled owner | Samples / subtree denominator | Share |
| --- | ---: | ---: |
| XML writing within SVG emission | 719 / 2606 | 27.59% |
| Bounded appending within SVG emission | 363 / 2606 | 13.93% |
| Root SVG protocol within SVG emission | 256 / 2606 | 9.82% |
| Source-style construction within preparation | 658 / 3347 | 19.66% |
| Text measurement within preparation | 1395 / 3347 | 41.68% |

These include descendants and overlapping categories; do not add them or interpret them as
removable CPU time or shares of the historical regression.

## Selected work removal

XML 1.0 permits TAB, LF and CR. The existing classifier nevertheless treats every byte below
0x20 as a reason to run the full per-byte scalar state machine. Actual public Zenuml CSS contains
2,818 logical bytes: its leading LF forces the first 2,048 bytes into scalar processing and its
trailing LF forces the remaining 770 bytes into it. There are no forbidden scalars in those chunks.

The two classification conditions now exclude exactly TAB/LF/CR from forbidden ASCII controls.
The chunk-wide reduction uses non-short-circuit boolean comparisons so LLVM can retain vector
comparison/reduction; its first-byte guard remains the exact forbidden/special set.
Full UTF-8/chunk classification remains; `&`, `<`, `]`, cross-fragment `]]>`, forbidden controls
and U+FFFE/U+FFFF keep their existing handling. Legal whitespace resets the bracket run exactly
like other ordinary characters. The same 2,048-byte UTF-8-safe chunks, scan observations, append
segments, final checkpoint, retained-byte admission and sticky first error remain. No new helper,
cache, buffer, dependency, unsafe code or production counter is added.

Both algorithms remain O(B) for B logical bytes and use constant extra state. The removed work is
per-byte scalar state processing after a safe chunk can use existing bulk classification and
delimiter search. The actual CSS scalar-byte counter falls from 2,818 to zero; logical bytes,
classification work and required encoding remain. This is not an asymptotic speedup.

An ignored probe extracts both actual implementations, changing only visibility/import context
and adding a scalar-byte counter. A recording sink compares ordered scan-observation, retained
plain append, entity append and checkpoint events. All 507 paired cases produce identical bytes
and events, including whitespace breaking a preceding `]]`, special/entity text, forbidden scalars,
Unicode and all suffix splits at 2,047–2,049-byte boundaries. This is not a replacement resource
policy implementation; real budget/cancellation/first-error tests cover the production sink.
The actual-CSS branch probe uses a common empty first fragment before the CSS, adding one
checkpoint on both sides; it is not a production-complete callback trace.
The extraction initially selected an alternate std-only memchr artifact; those records remain,
and the authoritative rerun selects the renderer's default+std feature set. No timing uses this
instrumented probe.

## Public controls and timing

All 74 frozen public inputs receive five allocation captures per side. SVG bytes, theme reports,
full ordered custom-measurer callbacks and all four allocation metrics match exactly. All 152
default benchmark SVG identities also match. Unlike the preceding correctness repair, this
candidate intentionally changes no authored-output bytes.

The first range-pattern formulation is rejected. Despite Zenuml public E2E observing -2.66%,
Kanban and Timeline public E2E observe +10.72%/+18.95%, each beyond 1 us. It fails the registered
material-control screen. The ignored extracted classifier's optimized assembly shows the baseline
NEON comparison/reduction replaced by a branchy scalar loop with variable bit shifts. All its raw
correctness, memory and diagnostic results remain in the parent experiment directory.

The second variant changes only the equivalent whole-chunk predicate expression to explicit
non-short-circuit boolean comparisons. Its extracted classifier regains NEON instructions and
passes the same 507 byte/event controls before production rebuilding. This is codegen attribution
on this host, not a promise of identical vectorization on WASM. Fresh final-candidate diagnostics
and complete semantic/resource gates are required; none of the rejected candidate timing is
reused for final acceptance.

Fresh final-candidate diagnostics use the same eight registered rows, two balanced AB/BA pairs,
30 Criterion samples, two-second warmup and three-second measurement: 32 invocations. Every
preflight/postflight SVG identity matches. Values are medians of two estimates per side.

| Operation | Adjacent base, us | Final candidate, us | Observed change |
| --- | ---: | ---: | ---: |
| render/zenuml_medium | 10.0252 | 7.5101 | -25.09% |
| end_to_end/zenuml_medium | 27.6735 | 24.9990 | -9.66% |
| render/packet_medium | 4.3257 | 4.4154 | +2.07% |
| end_to_end/packet_medium | 11.2035 | 11.5720 | +3.29% |
| render/kanban_medium | 29.7715 | 29.5255 | -0.83% |
| end_to_end/kanban_medium | 58.9020 | 59.0115 | +0.19% |
| render/timeline_medium | 14.7140 | 15.0655 | +2.39% |
| end_to_end/timeline_medium | 43.4035 | 43.8335 | +0.99% |

No public E2E row triggers the registered greater-than-10% AND greater-than-1-us regression
screen. Packet E2E still observes +0.3685 us/+3.29%; Timeline +0.4300 us/+0.99%; Kanban
+0.1095 us/+0.19%. These small control differences are retained rather than described as proven
zero cost. The experiment admits exact work removal with passing semantic/resource/memory
controls, not a calibrated latency improvement. There is no fresh A/A or confirmation run.

Separate same-SVG v0.9.0 context measures Zenuml public E2E 19.9770 → 24.9730 us
(+25.01%). This remains a diagnostic release gap with differing error/resource contracts;
it is not an all-family ratio or a theme-only causal attribution.

## Verification and remaining work

Correctness and adversarial/XML-security reviews find no required production changes. The existing
scalar/fragment-boundary test gains two suffixes, `]]\t>` and `\n&<]]>尾`, without weakening any
assertion or introducing a new matrix. The final variant independently passes:

- Six-package all-features release nextest: 8,250 passed, 13 skipped.
- Five-package all-features/all-targets Clippy with warnings denied; formatting and diff checks.
- Private theme qualification: 34 passed; WASM host tests: 47 passed.
- Minimal SVG/Zenuml/Treemap/Venn feature compilation.
- Optimized full Web WASM and 37-diagram runtime smoke.
- DOM comparisons across 37 family groups in structure/parity/parity-root modes.

The focused XML/bounded-output selection also passes all 19 tests. The final complete native run
includes both added suffix cases.

The next separate preparation candidate is source-style namespace ownership: when
`themeVariables.zenuml` is absent, 19 direct color reads plus 15 fallback reads and two typography
reads repeat a path that cannot succeed. Resolve that subtree once while preserving all actual
configuration and fallback semantics; absence of a compiled theme is not sufficient proof because
public site configuration still applies. Removing the unused stored `surfaceColor` entry must
retain its role as a fallback input. Two smaller independent candidates are post-write root state
retaining a complete viewport plan although only `viewBox` is read, and the extra intermediate
Engine clone at the public facade. None is implemented or credited as release-gap attribution here.

Production commit: `0ebf21cb25430e9113057e30a6c391acb7c04060` (`perf(svg): classify legal XML whitespace in bulk chunks`). Pipeline SHA-256: `ae866e31886ac606b08fb6d934b4defcff381add2c497df8c525cc5a25b76d84`.
Raw diagnosis, profiles and audits are under
`target/bench/experiments/zenuml-release-stages-20261017/`; preregistration, work probes, public
controls, diagnostics and validation for the selected change are under
`target/bench/experiments/xml-legal-whitespace-20261017/`, with final candidate receipts in
`vector-predicate/`. Work runs serially on Apple M4 Pro,
48 GiB, macOS aarch64, Rust 1.95.0. The previous XML A/A failure remains inconclusive and is not
retried. No full strict release/transport-matrix certification is claimed.
