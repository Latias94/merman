# Zenuml style ownership and unused title work — macOS, 2026-10-17

This checkpoint removes repeated message-style ownership in Zenuml and unused title-theme
predicates in Zenuml, Treemap and Venn. The adjacent base is
`9af25d2f640a343d788f618b7e019becdc626843`. Admission is exact public allocation/work reduction,
with output and error/resource controls. No calibrated latency improvement is claimed.

## Ownership changes

Zenuml layout already creates one message geometry style. Its width constraints and final message
width loops now borrow that style instead of copying the font family for each measurement.
Authored weight/style overrides still receive a temporary owned style. Measurement calls are not
coalesced: custom measurers retain their complete ordered text/style sequence. The default 16px
measurement and 14px drawing compatibility remains; configured presentation typography retains
its selected size and comment weight/style behavior. No persistent cache or retained state is added.

Let `C_cross` and `C_self` count the actual cross-participant/self constraint measurements, and
`M`/`K` count final ordinary/creation message width measurements. Without authored weight/style
overrides, the old repeated font copies are `C_cross + C_self + M + K`; the new path borrows the
existing style. A repeated ordinary message usually contributes two copies. Both algorithms remain
linear in these measurements; no asymptotic speedup is claimed. With actual weight/style overrides,
the corresponding owned temporary remains.

All three `SingleTitleFillPlan` callers now return the same default plan before computing title
presence, visible-text predicates or source config ownership when no theme exists. The shared
resolver accepts a present theme by reference, making that boundary explicit. Zenuml retains its
source style; source configuration continues to affect rendering. Theme-enabled rule, evidence,
resource and failure ordering remains. Some skipped config ownership queries do allocate; pure
predicate removal alone is not treated as a measured CPU saving. Venn preparation still resolves
its effective title before entering the plan, and its typography resolver still accepts an unused
`_title` parameter. Removing that outer redundant interface is separate cleanup; this checkpoint
does not claim that all no-theme title predicates have disappeared.

## Public allocation and semantic controls

The first frozen corpus has 45 inputs. Examination of rendered CSS showed that its source-init
font configuration did not select the intended effective font; those rows remain recorded, but
are not claimed as authored-font coverage. The failure and correction are registered in the
ledger; they are not evidence that the intended configured path was fast. Before measuring the
selected candidate, a second registration adds 14 direct public site-config inputs. Both revisions are measured across all
59 inputs, with five repetitions each, covering real font/size overrides, comment weight/style,
Unicode and entity spelling, three message scales, title/source ownership, theme variants and
Packet/Kanban/Timeline/Class controls. Renderer/theme construction and warmup are outside allocator
capture; the complete public render request and output are inside it.

All 59 SVGs and theme reports match. Thirty-five inputs reduce allocation count and cumulative
bytes; 24 are unchanged. None increases count, bytes or peak growth, and every retained-byte count
matches. Full custom-measurer text/style callback traces and output match for all 59 pairs. All
152 adjacent SVG benchmark identities also match.

| Public input | Allocation count, base → candidate | Allocated bytes saved |
| --- | ---: | ---: |
| Default medium Zenuml | 570 → 564 | 150 |
| One ordinary message | 341 → 339 | 50 |
| 16 ordinary messages | 1488 → 1456 | 800 |
| 128 ordinary messages | 9821 → 9565 | 6400 |
| 128 messages, actual site-config font/size | 9824 → 9568 | 4352 |
| Default Treemap | 765 → 761 | 129 |
| Default Venn | 3446 → 3444 | 99 |

The scale rows support removal of two font copies per ordinary message. They are allocator counts,
not a public latency estimate. Empty and title-only Zenuml remain unchanged, as do unrelated-family
controls and the relevant theme-enabled title paths.

## Cost attribution and rejected CSS experiment

The unchanged native pipeline baseline uses default features plus SVG, strict parsing,
deterministic measurement and trusted-native resources. Ten registered stage observations use
30 Criterion samples, two-second warmup and three-second measurement. Zenuml baseline estimates
are parse 9.3176 us, fresh-engine parse 12.9400 us, preparation/layout 4.6930 us, emission 12.9250 us
and public E2E 29.8020 us. Nearby Timeline controls remain separate. These stages are not additive.

The unchanged `profile_render` example then samples layout, render and E2E separately: ten-second
processes, batch 100, five-second macOS `sample` captures at 1 ms, with no concurrent Cargo or
allocation capture. The example uses its default deterministic resource profile, not the
Criterion trusted-native recipe. Render whole-process sampling includes fresh preparation; only
the `emit_prepared_svg` subtree defines emission.

| Sampled owner | Inclusive samples | Owner denominator | Share |
| --- | ---: | ---: | ---: |
| Deterministic text measurement | 1035 | layout function 1823 | 56.77% |
| XML text writer | 580 | emission 2565 | 22.61% |
| `escape_xml*` | 669 | emission 2565 | 26.08% |
| Allocating `fmt::format_inner` | 367 | emission 2565 | 14.31% |

Matching nested frames are counted once. The final three categories do not overlap in this
sample, but do not measure exclusive CPU time or potential savings. Not all escaping belongs to
CSS. These observations motivate encoding ownership work; they do not assign the release gap.

A screened CSS candidate replaced eager `escape_xml` values with existing `escape_xml_display`
formatting while retaining the full CSS String and one final append. It preserved every SVG and
report, but failed the registered allocation gate: default Zenuml 570 → 579 allocations and empty
Zenuml 233 → 248. Repeated placeholders rerun Mermaid entity decoding, including allocation on
ordinary `#hex` colors; the font appears twelve times. This is a concrete counterexample to
assuming a Display wrapper removes allocation. The candidate is rejected and its exact diff and
raw results remain recorded. No CSS production change is retained.

## Latency controls

The selected candidate receives twelve adjacent layout/render/E2E rows, two balanced AB/BA
pairs each, 30 Criterion samples, two-second warmup and three-second measurement: 48 invocations.
Every preflight/postflight output identity matches. Values are medians of two estimates per side:

| Operation | Adjacent base, us | Candidate, us | Observed difference |
| --- | ---: | ---: | ---: |
| layout/zenuml_medium | 4.7450 | 4.7760 | +0.65% |
| render/zenuml_medium | 13.2780 | 13.6530 | +2.82% |
| end_to_end/zenuml_medium | 31.0320 | 31.4250 | +1.27% |
| layout/treemap_medium | 11.9135 | 11.9460 | +0.27% |
| render/treemap_medium | 19.2105 | 19.3030 | +0.48% |
| end_to_end/treemap_medium | 39.5660 | 39.4245 | -0.36% |
| layout/venn_medium | 56.6775 | 56.3875 | -0.51% |
| render/venn_medium | 4.5395 | 4.5438 | +0.10% |
| end_to_end/venn_medium | 147.8750 | 147.3100 | -0.38% |
| layout/timeline_medium | 20.6360 | 21.1330 | +2.41% |
| render/timeline_medium | 15.0240 | 14.8430 | -1.20% |
| end_to_end/timeline_medium | 43.3025 | 43.3550 | +0.12% |

No row triggers the preregistered material-control screen of greater than 10% and greater than
1 us public E2E regression. This is a diagnostic screen, not confirmed non-regression. In
particular, Zenuml does not show a latency improvement; its allocation reduction is the accepted
claim. No A/A or fresh confirmation is run for a latency claim in this batch.

The separate four-invocation v0.9.0 context observes Zenuml E2E 20.0490 → 30.5910 us
(+52.58%). SVGs match; release-range error/resource contracts differ. The historical
reference is `f754a3c3077d35d2a372c2fec04f9e9b87db1c3f`. Treemap and Venn differ from the tag in SVG
output and receive no equivalent-output release ratio. This does not isolate themes or prove
release recovery.

## Remaining encoding defect

The direct site-config corpus confirms an existing bug with accepted `fontFamily` equal to
`"A&B", monospace`. After parsing the SVG as XML, style text still contains
`font-family:&quot;A&amp;B&quot;,monospace` instead of logical CSS
`font-family:"A&B",monospace`. Individual CSS values are XML-encoded and the complete CSS is
encoded again by `XmlTextOutput`. This checkpoint deliberately preserves those bytes.

Fix encoding ownership as a separate correctness change: produce logical CSS, encode at the XML
boundary, and explicitly verify literal entities, Mermaid spelling, quotes, Unicode and hostile
style-closing text. Preserve font/CSS safety parsing, attribute encoding, output limits,
cancellation and sticky first errors. Changed output/byte limits cannot be quietly admitted as a
same-output performance shortcut. The existing Railroad writer provides a local logical-font,
single-XML-encoding precedent. Mermaid's Zenuml plugin does not pass this extension configuration
to its companion; it does not supply an upstream contract for these authored values.

## Validation and provenance

Three independent read-only reviews cover correctness, adversarial composition and
maintainability/test scope, with no required changes in the final five-file candidate. The CSS
candidate's repeated-decoder counterexample was independently identified and its allocation
gate failed; no rejected code remains. Independent report fact checking confirms the allocation,
callback, SVG, stage and sampling records.

Validation passes:

- Six-package all-features release nextest: 8,246 passed, 13 skipped.
- Five-package all-features/all-targets Clippy with warnings denied; formatting and diff checks.
- Private theme qualification: 34 passed; WASM host tests: 47 passed.
- Minimal SVG/Zenuml/Treemap/Venn feature compile.
- Actual optimized full Web WASM build and 37-diagram runtime smoke.
- All-family DOM comparisons across 37 family groups in structure/parity/parity-root modes.

The initial candidate additionally passed 138 focused family tests; after rejecting its CSS part,
the complete final-candidate native suite above covers the retained code. No test assertion was
removed or weakened; one existing typography test receives the mechanical extra base-style
argument required by its private helper.

Production commit: `4060f91e17c3b327db86816d4fe4b301ad1371f0`
(`perf(render): remove repeated Zenuml style ownership`).

The selected pipeline SHA-256 is
`78f62faeceb682aa054430a47e32403eee88fef872396c0e1209d30ee109d1f3`; adjacent frozen baseline is
`fa73cb03b67573e725ad0e137df982861e2016e5cd9db04e5a6d168505dfde74`.
Source snapshots and runtime-library build receipts bind the allocator and callback probes.

Raw registrations, rejected candidate, source/artifact digests, all allocator records, callback
traces, profiles, stage observations and validation logs remain under
`target/bench/experiments/zenuml-ownership-20261017/`. Corrected site-config controls are in
`corpus-v2.json`; the initial corpus/results remain available. Cargo work, allocation capture,
profiling and latency processes run serially on Apple M4 Pro, 48 GiB, macOS aarch64, Rust 1.95.0.

The previous XML checkpoint's failed A/A remains inconclusive and is not retried. No full strict
release/transport-matrix certification is claimed for this local ownership batch.
