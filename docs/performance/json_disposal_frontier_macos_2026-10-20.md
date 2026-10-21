# Iterative JSON disposal — October 20, 2026

## Decision

The registered independent memory and behavior comparison and all 16 primary integration
validation stages pass. This is memory/work-removal evidence, without a latency or v0.9.0
convergence claim.

## Change

`drop_value_nonrecursive` previously put every JSON value into a temporary heap worklist,
including scalars that need no recursive disposal. The shared function now handles the root
directly, disposes scalars immediately, keeps one child container inline, and queues only other
child containers. A chain with at most one container child per level needs no worklist allocation,
even when each level also has scalar siblings. Multiple pending container siblings still use
an ordinary `Vec`; no general monotonic capacity or memory guarantee is claimed for arbitrary
container-dense inputs.

The original nonrecursive disposal contract remains necessary for deeply nested configuration
and managed semantic JSON. The model, configuration, cancellation, resource policy, evidence
and output algorithms are unchanged. No metadata owner is kept alive to hide a peak, and model
field order is unchanged. The original helper body is already present in v0.9.0; this is a shared
JSON memory optimization, not attribution of the release regression to the theme refactor.

The clean base is `b8a5f59c13d7eb2ae3b953db177dd6d28b9bd799`, after integration of PR #226
and the appearance/CSS release report. Only `crates/merman-core/src/compatibility_json.rs` changes
runtime code. One new test disposes a 10,000-level mixed array/object value on a 128 KiB stack;
it first passed on the original implementation, then passed with the candidate.

## Measurement

The complete public `Renderer::render(RenderRequest::svg(...))` result remains owned at the
allocator checkpoint. The recipe is release/opt-level 3, all facade features, macOS on Apple M4
Pro, Rust 1.95.0, one warmup and five saved samples, serial execution. Themes compile before the
measured operation. The immutable original base observations and four frozen physical probes
are reused from `facade-svg-metadata-owner-20261020`; they are not observations from that
experiment's rejected metadata candidate.

Before editing runtime code, the independent primary was registered as a strict decrease in
Venn medium / None catalog peak growth. All four memory metrics must be nonincreasing on every
jointly stable site/catalog control; variable controls remain inconclusive without resampling.
The catalog baseline is 183,387 B peak growth. The separate site probe baseline is 184,404 B;
these two observations are not interchanged.

| Catalog workload / metric | Base | Candidate | Change |
| --- | ---: | ---: | ---: |
| Venn / None allocations | 3424 | 3417 | -7 |
| Venn / None allocated bytes | 458148 | 393996 | -64152 |
| Venn / None peak growth bytes | 183387 | 180461 | -2926 |
| Venn / None retained growth bytes | 2835 | 2835 | 0 |
| Packet / None allocations | 108 | 104 | -4 |
| Packet / None allocated bytes | 19117 | 18829 | -288 |
| Packet / None peak growth bytes | 11742 | 11742 | 0 |
| Packet / None retained growth bytes | 3951 | 3951 | 0 |

All 418 catalog cells, covering 35 logical families and 38 fixture aliases with ten compiled
presets plus None, are stable and improve. All 645 final SVG/report identities and independent
ordered-measurer traces match. Of 505 jointly stable site controls, all 505 improve and none
regress; 140 variable controls remain inconclusive. All 26 complete public errors match.
Warmup result metadata and all five saved catalog sample SVG/report identities match.

Every catalog cell reduces allocation count by 2–55 and allocated bytes by 288–127,728.
Peak growth decreases in twelve cells: Venn/None and all eleven Usecase cells. The other 406
catalog peaks stay unchanged, as do all 418 retained-growth observations. These are total
requested allocation/live-heap measurements, not RSS or elapsed time.

## Validation

| Check | Result |
| --- | --- |
| New small-stack disposal test on original algorithm | 1 pass before runtime changes |
| Focused compatibility/deep/bounded/resource/cancel/theme release tests | 1394 pass |
| Four-crate all-features release nextest | 8101 pass, seven skipped |
| Matching strict release Clippy | Pass with `-D warnings` |
| Full 37-entry DOM matrix | Pass under existing policies; fresh reports archived |
| Optimized full WASM, including wasm-opt | Pass; 1217 inputs and six artifacts verified |
| Release theme acceptance | 34 pass |
| Format, diff and indexed path length | Pass |
| Compiled complete pipeline discovery | 459 benches / receipts, seven lane groups |
| Existing native-memory smoke and report contract | `protocol_smoke_pass`, verified |

The complete serial primary driver has a real terminal exit status of zero and sixteen
successful stages. The added test exercises the actual disposal helper, while existing managed
semantic JSON and deep/bounded lifecycle tests retain multiple-container sibling, cancellation,
resource and ordinary disposal coverage. No benchmark dependency, tracing, CI benchmark job or
alternate production parser is added.

The ignored experiment is `target/bench/experiments/json-drop-container-frontier-20261020/`.
It preserves preregistration, the original baseline references and four immutable probes, all
939 frozen input hashes, both sides' raw observations, candidate library/probe identities,
source/correctness/independent measurement reviews, native terminal receipts and fresh DOM/WASM
identity records. The checked-in evidence archive preserves the decision-bearing JSON and logs;
physical executables and individual SVG files remain in the ignored experiment directories.

## Rejected predecessor and next work

The earlier independent facade metadata-owner candidate saved one Packet allocation but advanced
configuration disposal while other parsed data was live. Seven stable Venn site controls gained
660–1,919 B peak growth, and four catalog cells also regressed. It was rejected under its original
gate, its patch and failed observations remain preserved, and its four source files were removed
only after exact owned-draft/base hash verification. None of that runtime patch is present here.
Baseline hardware watchpoints localized its original operation peak to the JSON disposal worklist;
the candidate debugger launch failed before main and supplies no candidate comparison evidence.

Reevaluate metadata ownership independently against this new baseline after integration. The
fresh prepared-source owner and ZenUML's unused editor-facts projection are separate unmeasured
source drafts. ZenUML's original files also match v0.9.0 and cannot explain the theme regression.

The latest admitted release timing remains the b9 appearance/CSS checkpoint: 14 within the joint
budget, two confirmed regressions and three inconclusive scenes. These memory results do not
reclassify any of those timings. Refresh calibrated A/A and fixed AB/BA at the next meaningful
runtime batch checkpoint; the all-family convergence goal remains incomplete.
