# Reusing XML chunk classification — macOS, 2026-10-17

This checkpoint removes a repeated delimiter search from ordinary XML text chunks. The adjacent
base is `1a35e6d4688002a8c591af1fb558381068345ac8`; its production runtime is the frozen
`feefbf006` preparation candidate. The historical context remains v0.9.0,
`f754a3c3077d35d2a372c2fec04f9e9b87db1c3f`.

The acceptance claim is a small owner-local work reduction with unchanged output, allocation,
append and control boundaries. Complexity remains `O(B)`. No new buffer, threshold, dependency,
unsafe code, persistent state or family-specific serializer is introduced. Public latency is a
separate experiment, not inferred from removed searches or private microbenchmarks.

## Implementation and contract

For chunks that pass its first-byte guard, `XmlTextOutput` already classifies every byte of a
bounded chunk before selecting its searchable or scalar scanner. That classification now also
records whether `&`, `<` or `]` occurs. An ordinary chunk needs one observed append and no
subsequent `memchr3` search. Exceptional chunks retain their original scanner selection.
The first-byte exceptional guard is unchanged.

The 2048-byte chunk boundary, with at most three additional UTF-8 continuation bytes, stays
unchanged. Input work is observed before classification. Plain appends use `write_scanned_text`;
entity expansion uses the normal observed append. Cross-fragment `]]>`, illegal scalar removal,
exact pre-allocation byte admission, sticky first errors and the final checkpoint retain their
ordering. A new characterization test passed on the original implementation before editing it:

| Fragments | Maximum bytes | Retained prefix | First error actual |
| --- | ---: | --- | ---: |
| `abc&de` | 4 | `abc` | 8 |
| `abc`, `def` | 4 | `abc` | 6 |
| One 4096-byte ASCII fragment | 2049 | 2048 ASCII bytes | 4096 |

Unconditional coalescing was rejected: changing append boundaries changes these observable budget
failures and can delay cancellation or declaration-iterator termination. Fixed short-fragment
fast paths were screened privately but not retained; their additional dispatch/threshold machinery
is unnecessary for this smaller shared-classification change.

## Exact work and memory controls

The throwaway probe extracts the actual before/after serializer, changing visibility and crate
paths only. A wrapper counts `memchr3` calls and the lengths of their input slices. A trace sink
records ordered scanned-work observations, ordinary/entity appends with payloads, and checkpoints.
Eight ordinary/hostile CSS, dense-entity, Unicode, newline and bracket inputs at seven UTF-8-safe
fragment sizes produce 56 byte-identical output and trace pairs.

| Same logical input | Fragment size | Search calls, base → candidate | Search slice bytes, base → candidate |
| --- | ---: | ---: | ---: |
| Packet default CSS | Whole input | 1 → 0 | 1601 → 0 |
| Kanban default CSS | Whole input | 19 → 13 | 20677 → 8389 |
| Kanban default CSS | 8 bytes | 1916 → 14 | 15306 → 92 |

Slice lengths count submitted search work, including overlapping slices; they are not instrumented
CPU bytes read. This private String-sink probe excludes production admission and formatter cost.
Private timings only screened candidates. No public speedup is derived from them.

The frozen 77-input public corpus covers default and configured Packet/Kanban, hostile fonts/CSS,
theme variants, scaling and other-family controls. Five candidate repetitions reproduce every
baseline SVG and theme report. Allocation count, cumulative allocated bytes, peak growth and
retained bytes are all exactly unchanged. Discovery also matches all 152 SVG benchmark identities
between adjacent revisions. The prior baseline executable is reused only because its complete
renderer source fingerprint matches the adjacent base.

## Public latency and release context

The existing native Criterion `pipeline` uses default features plus SVG, strict parsing,
deterministic measurement and trusted-native resources. The engine/process are reused; each
public E2E estimate is one complete public render operation. Private render excludes fresh
preparation and is kept separate.

The adjacent diagnostic uses ten registered render/E2E rows, two balanced AB/BA pairs, 30 Criterion
samples, two-second warmup and three-second measurement: 40 invocations with matching preflight
and postflight identities. Values are medians of two estimates per executable, without A/A:

| Operation | Adjacent base, us | Candidate, us | Observed difference |
| --- | ---: | ---: | ---: |
| render/packet_medium | 4.5628 | 4.3768 | -4.08% |
| render/kanban_medium | 31.4025 | 30.1880 | -3.87% |
| render/timeline_medium | 14.8685 | 15.1525 | +1.91% |
| render/class_medium | 240.3000 | 238.6850 | -0.67% |
| render/flowchart_large | 1077.4500 | 1087.3500 | +0.92% |
| end_to_end/packet_medium | 11.4240 | 11.1415 | -2.47% |
| end_to_end/kanban_medium | 60.9580 | 58.8140 | -3.52% |
| end_to_end/timeline_medium | 43.0410 | 43.4965 | +1.06% |
| end_to_end/class_medium | 676.9650 | 676.0300 | -0.14% |
| end_to_end/flowchart_large | 4611.6500 | 4589.0000 | -0.49% |

Packet and Kanban move modestly; Timeline does not improve in this diagnostic. These observations
are not a calibrated public latency claim, and private stage values are not additive.

The separate v0.9.0 context measures every one of the 19 exact-SVG-matching E2E rows identified
before timing. Its two AB/BA pairs use the same 30/2s/3s recipe: 76 invocations. Both executable
hashes and every preflight/postflight output identity are verified. The other 57 current SVG E2E
rows have changed output; their identities remain in `discovery.json`, without a new equivalent-output
latency ratio. The October 16 broad report retains historical changed-output context.

| E2E fixture | v0.9.0, us | Candidate, us | Observed difference |
| --- | ---: | ---: | ---: |
| class_tiny | 130.9150 | 145.9600 | +11.49% |
| class_medium | 675.2350 | 686.6600 | +1.69% |
| class_namespace_dense | 413.9250 | 431.0750 | +4.14% |
| class_nested_namespaces | 1624.3000 | 1648.5500 | +1.49% |
| class_nested_namespaces_large | 15652.5000 | 15640.0000 | -0.08% |
| pie_medium | 25.7225 | 29.0275 | +12.85% |
| journey_medium | 27.9395 | 29.3465 | +5.04% |
| timeline_medium | 40.2390 | 43.2725 | +7.54% |
| gantt_medium | 37.0060 | 40.9540 | +10.67% |
| requirement_medium | 245.1600 | 250.1400 | +2.03% |
| c4_medium | 108.2300 | 119.1900 | +10.13% |
| sankey_medium | 24.0330 | 27.0165 | +12.41% |
| quadrant_medium | 27.1590 | 30.6045 | +12.69% |
| zenuml_medium | 20.3995 | 31.4960 | +54.40% |
| packet_medium | 8.1944 | 11.2260 | +37.00% |
| kanban_medium | 46.1405 | 59.7745 | +29.55% |
| architecture_medium | 48.2625 | 49.4110 | +2.38% |
| wardley_medium | 42.6325 | 45.9590 | +7.80% |
| cynefin_medium | 29.7500 | 31.9415 | +7.37% |

These are diagnostic medians, without A/A or equal release-range error/resource contracts. There
is no global recovery score and no inference that themes cause the whole difference. Class medium
is close to the tag; Zenuml, Packet and Kanban retain the largest relative gaps in this selected
set. The tag's two Architecture observations differ by 10.78% of their median, and Wardley's by
6.36%; their small apparent gaps are especially noisy. No rows are dropped or resampled.

The preregistered adjacent confirmation independently rebuilt clean base `1a35e6d46` and candidate
`b696be5e9`, with one Cargo job and the existing shared-target freezing protocol. Builds took
6m51s and 6m46s. Fresh calibration used eight A/A pairs per binary per row: 64 invocations,
30 Criterion samples, two-second warmup and three-second measurement. Bootstrap uses 10,000
resamples, seed zero, and a simultaneous 95% family covering 16 calibration components. Frozen
public baselines were Packet 11,401 ns and Kanban 59,172.5 ns, with a 32-pair cap.

Calibration is **inconclusive**. Packet base's A/A identity interval excludes zero: absolute
30.3279 to 173.9221 ns, log-ratio 0.00261209 to 0.01573220 (relative +0.2616% to +1.5857%). Its
noise magnitudes satisfy the noise ceilings, but the separately required zero-identity condition
fails. Kanban meets every calibration condition, with absolute noise 507.9152 ns below its
1,000 ns ceiling. The registered complete two-row set is blocked: no fresh A/B pair runs, no row
is removed, no budget is extended and no calibration is repeated. Both rows remain inconclusive;
no confirmed latency improvement or non-regression is claimed.

Executable, source, fixture and clean-revision post-verification all pass; there are zero contract
failures. The separately rebuilt candidate is byte-identical to the diagnostic executable. The
fresh base SHA-256 is `b3ad52b30a5496dac85250805ecd9e8189f616b259ac78d8fde96c03d4fa475a`.
This attempt does not replace the prior shared-XML checkpoint's separate failed calibration.

The acceptance remains the preregistered owner-local repeated-search removal with identical
public output and allocation controls. Its small implementation adds no state; diagnostic or
calibration samples are not relabeled as confirmed public speedups.

## Next owner candidates

A read-only comparison of the unchanged Zenuml parser/model and fixture identifies three narrower
candidates for its remaining diagnostic gap. Paths below are relative to `crates/merman-render/`.
They are source evidence, not attributed latency:

1. `src/zenuml/theme.rs` computes title/visible-text applicability even when `theme=None`, although
   `SingleTitleFillPlan::resolve` immediately returns its default. Defer those predicates until a
   theme consumes them. The default fixture normally short-circuits at its first participant;
   this is not necessarily a complete-layout traversal. The same eager-predicate pattern exists
   at all three `SingleTitleFillPlan::resolve` production callers: Zenuml, Treemap and Venn. The
   latter two compute title/config-ownership predicates before the helper returns its default;
   keep these within their respective theme branches. Inlining may eliminate some pure work,
   so this source inventory does not establish its runtime cost.
2. `src/zenuml.rs` now constructs owned font-family measurement styles per message; the tag shared
   a base measurement style. Attribute and remove unnecessary ownership in the preparation owner,
   preserving custom font/size and annotation-weight behavior, including the default compatibility
   difference between 16px measurement and 14px drawing.
3. `src/svg/parity/zenuml.rs::zenuml_css` XML-escapes individual CSS values into allocated Strings,
   then writes the complete CSS through `XmlTextOutput`. This can encode an inner entity again;
   the default fixture has no triggering characters. Verify accepted authored inputs and Mermaid
   entity decoding first, then assign XML encoding to its final boundary. The whole-CSS String
   already existed in the tag. CSS safety parsing, attribute encoding and append/error boundaries
   remain separate responsibilities.

The three candidates do not account for or divide the observed +54.40% gap. Measure preparation,
layout and CSS emission separately before choosing an implementation. Geometry finite checks,
resource admission and bounded cancellation have real consumers and are not proposed for deletion.

## Validation and provenance

Production commit: `b696be5e9` (`perf(svg): reuse complete XML chunk classification`). Code
simplification and review found no required changes, including XML escaping and resource/error
boundaries. Receipts are retained with the raw experiment. Reviewers read code; the following
checks were executed separately:

- Characterization-first baseline and candidate output-owner tests: 18 passed each.
- Six-package all-features release nextest: 8,246 passed, 13 skipped.
- Five-package all-features/all-targets Clippy with warnings denied; formatting and diff checks.
- Private theme qualification: 34 passed; WASM host tests: 47 passed.
- Minimal SVG/Packet/Kanban/Timeline feature compile.
- Actual optimized full Web WASM package and runtime smoke across 37 diagram capabilities.
- All-family DOM comparison across 37 family groups in structure/parity/parity-root modes, with
  three decimals and the existing browser-text-layout diagnostic.

The diagnostic pipeline SHA-256 is
`fa73cb03b67573e725ad0e137df982861e2016e5cd9db04e5a6d168505dfde74`; adjacent base is
`4cfd0c3c05ec147274b79de83e55a895932ecf82dfc9e30e3646c92745da4a0f`; tag is
`5562fea88eb11457fdbf459c61076cd4011c476a0fc26828eb39701987b65f57`.
All three refer to frozen copied executables, not mutable Cargo output paths. Their source and
input fingerprints are recorded. Fresh confirmation owns its independently rebuilt executable
provenance.

Raw experiment registration, rejected candidates, extracted probes, source/executable hashes,
trace counters, public allocation records, discovery, Criterion logs and validation logs remain
under `target/bench/experiments/xml-fragment-dispatch-20261017/`. No benchmark implementation,
corpus, output policy, dependency or Mermaid baseline changes.

Host: Apple M4 Pro, 48 GiB, macOS aarch64, Rust 1.95.0. Builds, allocation capture and timed
processes run serially. The deliberately stopped earlier 661-build transport/release sweep remains
uncertified; this checkpoint does not claim a full strict release-matrix run.
