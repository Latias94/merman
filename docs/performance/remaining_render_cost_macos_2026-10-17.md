# Remaining Packet/Kanban cost — macOS, 2026-10-17

This is a diagnosis of clean `5b8040264`'s runtime, after the
[preparation cleanup](packet_kanban_preparation_macos_2026-10-17.md). It does not introduce a
production optimization. Existing frozen executables match the current production source hashes.
The historical reference is v0.9.0, `f754a3c3077d35d2a372c2fec04f9e9b87db1c3f`.

The strongest next optimization target is fragmented logical CSS serialization at the shared XML
writer. Default Packet/Kanban do not execute a hidden per-element typed-theme certification pass.
Necessary escaping and bounded output remain; the avoidable part to investigate is how often
the serializer and sink are entered for the same stylesheet. Several smaller ownership copies
are independently removable but do not explain the complete release-range difference.

## Comparable measurements

Both pipeline executables use the existing native benchmark, default features plus SVG, strict
parsing, deterministic measurement and trusted-native resources. Six rows have matching output
identities: Packet/Kanban render and public end-to-end, plus Timeline parse/end-to-end controls.
Each runs two balanced pairs, AB then BA: 24 invocations, 30 Criterion samples, two-second warmup,
three-second measurement, and preflight/postflight identity verification. Values below are medians
of two observations per executable. They are diagnostic, without A/A calibration or confirmed
latency admission.

| Operation | v0.9.0, microseconds | Current, microseconds | Observed difference |
| --- | ---: | ---: | ---: |
| render/packet_medium | 3.3250 | 4.4039 | +32.45% |
| end_to_end/packet_medium | 8.06225 | 11.40100 | +41.41% |
| render/kanban_medium | 26.4070 | 31.2335 | +18.28% |
| end_to_end/kanban_medium | 44.5710 | 59.1725 | +32.76% |
| parse/timeline_medium, control | 4.03290 | 4.50985 | +11.83% |
| end_to_end/timeline_medium, control | 39.307 | 42.534 | +8.21% |

The tag and current ordinary SVG/resource/error contracts are not equivalent. These same-SVG
workflow observations cannot establish a security-equivalent release regression or assign the
entire difference to themes.

Packet/Kanban parse, cold-engine parse and prepared-layout projections do **not** match across
revisions: each parse projection is 19 bytes larger, each layout projection 38 bytes larger.
The discovery stopped before timing those comparisons; no identity normalization was applied.
The generated default config's only change is `mindmap.colorBy = "branch"`, which adds exactly
19 compact JSON bytes. This is consistent with the projection deltas, not a full runtime
projection-diff proof. The original mismatching receipts remain recorded.

Those six stages instead receive one current-only observation each:

| Family | Reused-engine parse | Fresh-engine parse | Preparation/layout |
| --- | ---: | ---: | ---: |
| Packet | 2.6807 us | 6.2272 us | 1.4269 us |
| Kanban | 7.2912 us | 11.4110 us | 17.6440 us |

These boundaries are not additive. Criterion's layout setup excludes parsed-model cloning and
session creation; its render setup excludes fresh preparation. Public end-to-end includes request
and environment creation, admission, reports and destruction. Moving color calculation from
emission into preparation can move a private-stage ratio without reducing the public operation.
No remainder is inferred by subtracting these stage estimates.

## CPU attribution

The unchanged `profile_render` example samples current Packet/Kanban in `end-to-end` and `render`
modes. Each process runs ten seconds with batch size 100; macOS `sample` captures five seconds at
1 ms intervals. No Cargo, other profiler or allocation capture runs concurrently. End-to-end
profiles use the full reused-renderer operation. Whole-process render profiles also include fresh
preparation; only `profile_render::emit_prepared_svg` defines emission and output consumption.
The example uses the default deterministic resource profile, while Criterion uses trusted-native;
these samples attribute code, not equivalent-policy latency.

Within that emission subtree:

| Current profile | Emission samples | Samples beneath XML text writing | Inclusive share |
| --- | ---: | ---: | ---: |
| Packet | 2,631 | 909 | 34.55% |
| Kanban | 2,284 | 1,195 | 52.32% |

Each sampled branch is counted once if it passes through `XmlTextOutput`; nested occurrences are
not added again. These are inclusive stack samples, not exclusive CPU time, wall-clock savings,
or confidence bounds. They include descendant sink/allocation work. The categories overlap and
must not be summed. Only five samples in each emission subtree reach the operation checkpoint
symbol; this does not justify deleting limits or cancellation. Inlining and sampling bound all
symbol-level attribution.

Kanban's unchanged twelve-slot palette template has 41 substitutions per slot: 492 dynamic fields,
plus literal fragments, before the remaining CSS. This is a source count of substitutions, not an
instrumented count of formatter calls. Those pieces now enter XML serialization directly. The
tag assembled CSS in an ordinary String; its path lacked the current stylesheet XML escaping.
Current preparation also visibly spends time in palette color calculation and Markdown/XHTML
label measurement. Both have real consumers; the twelve palette slots already existed in the tag.

## Fragment-size experiment and counterexample

A throwaway probe extracts the actual current `XmlTextOutput` algorithm, changing only visibility,
crate-local error paths and the unused resource import. It links the recorded existing `memchr`
artifact. The sink is a String; this excludes production resource admission and real formatter
cost. The logical input is decoded from each frozen public SVG stylesheet. All fragment sizes
reproduce the exact default stylesheet. Hostile inputs additionally append a NUL, `]]>`, `<`, `&`
and U+FFFE followed by `]x`, and every fragment size produces identical normalized output.

The registered UTF-8-safe sizes are 1/8/32/128/512/2048 bytes and whole input. Two 250 ms observations
use forward then reverse size order; inputs and fragment lists are prepared outside timing. The
table reports medians for selected sizes; every registered result remains in the ledger.

| Same logical CSS | 32-byte fragments | 128-byte fragments | Whole input |
| --- | ---: | ---: | ---: |
| Packet default | 0.673 us | 0.280 us | 0.105 us |
| Kanban default | 4.627 us | 1.705 us | 1.193 us |
| Packet plus hostile suffix | 0.709 us | 0.383 us | 1.654 us |
| Kanban plus hostile suffix | 4.705 us | 1.820 us | 2.153 us |

The default rows establish sensitivity to fragment count in the real serializer. They do not
reproduce the formatter's uneven boundaries or measure a public candidate. The hostile suffix is
a counterexample to unconditional coalescing: an exceptional byte can select scalar processing
for an otherwise ordinary bounded chunk. Increasing chunk size can therefore add work. Whole-input
writes still scan in the existing bounded chunks; they do not make scanning unbounded.

## Responsibility audit and next work

Two independent source audits trace the actual default paths and compare the tag:

- No-theme Packet has no text selection, text output observer or terminal theme outcome. Default
  style fields return literal fallbacks without declaration parsing. The previous preparation
  cleanup already removed empty compatibility snapshots and declaration roundtrips.
- No-theme Kanban disables style-fact capture, per-task theme decisions and terminal outcomes.
  It does not build a hidden typed-theme array for every card.
- Ordinary BestEffort standalone generation already skips complete terminal XML/native
  certification. Root-prefix ownership and closing-root checks existed in the tag.
- Default Packet CSS uses a normalized ordinary identifier string. Its many selectors do not
  invoke the operation-aware diagram-ID projection. That projection cannot be deleted globally:
  other families still create intermediate Strings before the bounded document sink sees them.
- Font normalization and safety parsing have distinct jobs. The tag also parsed its configured
  root font, so current Packet cannot be described as introducing an entire extra default font
  parse. Borrowing the input instead of copying it is a separate small ownership opportunity.

The recommended implementation order is:

1. **Experiment at the shared XML writer.** Compare reducing per-fragment setup against bounded
   coalescing that also localizes exceptional scalars. Do not restore an unbounded complete-CSS
   buffer, assume authored values are safe, remove XML normalization, or introduce a second family
   serializer. Preserve cross-fragment `]]>`, exact byte admission before backing-buffer growth,
   bounded cancellation, resource accounting and sticky first failures. Any new staging state
   needs a fixed space bound and explicit failure-order analysis. Register adjacent-candidate
   public E2E gates and memory controls before production edits; this experiment has not selected
   an implementation or demonstrated its acceptance.
2. **Delete proven unused ownership separately.** `root_svg.rs` clones and retains an entire
   `RootViewportPlan` although `RootedSvg` only consumes its `Option<ViewBox>`. Kanban first builds
   a complete temporary label Vec, then moves labels into a second final-item Vec. Common CSS
   clones the diagram font into a root-font field immediately overwritten on the ordinary path.
   These are candidates for exact allocation gates, preserving root/theme/error semantics and
   label measurement order. They are not presented as the main remaining latency cause.
3. **Leave necessary work intact unless separately attributed.** Keep all twelve Kanban palette
   slots, actual Markdown/XHTML measurements, CSS/font safety parsing, user-ID admission and
   terminal consumer preparation. Reducing palette slots would change CSS and invalid-color
   behavior even when the diagram has fewer sections.

The next writer experiment is distinct from the prior shared-XML confirmation, which remains
inconclusive. It must not retry that failed calibration until it passes, reuse its samples, or
relabel diagnostic microbenchmarks as public-operation improvements.

## Evidence and validation scope

Raw provenance, discovery failures, all 30 Criterion invocations, four profiles, source-level
audits, fragment inputs/build manifests and all microbenchmark observations are retained under
`target/bench/experiments/remaining-render-cost-20261017/`. Frozen pipeline SHA-256 values are
`5562fea88eb11457fdbf459c61076cd4011c476a0fc26828eb39701987b65f57` (tag) and
`4cfd0c3c05ec147274b79de83e55a895932ecf82dfc9e30e3646c92745da4a0f` (current).

Host: Apple M4 Pro, 48 GiB, macOS aarch64, Rust 1.95.0. No production source, dependencies, benchmark
implementation, output policy or test contract changed. The prior native/private/real-WASM/DOM
checks remain evidence for the identical production runtime; they were not rerun for this
documentation-only diagnosis. No full strict release-matrix certification is claimed.
