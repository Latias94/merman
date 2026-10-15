# Native SVG scalar and ownership experiments on Windows, 2026-10-15

The exact UTF-8 character candidate is committed as `21671959cf55c0895ac23413d198e39d39980fc3` with performance confirmation pending. It is not an admitted latency improvement and does not close the v0.9.0 SVG regression.

Base product: `a56aa0792f6d97e976517887cc86cdfe58e46d74`. The earlier [terminal checkpoint](native_svg_terminal_followup_windows_2026-10-15.md) retains its own rejected dispatch experiment. This experiment evaluates two independent candidates, returning production to the exact base between them. The following observations were frozen before the candidate commit; their uncommitted-source evidence class is retained. No push is performed.

## Ownership boundary investigation

A complete structured SVG writer can supply success-only XML/reference facts without an event transcript. Only a complete, valid, unmodified generation below the operation's raw-element ceiling can retain facts; every uncertain/raw/invalid/mutated case falls back to the existing terminal parser. Reaching the element ceiling must abandon proof collection without publishing an early error or retaining facts beyond the old prefix resource bound.

This is feasible but is not yet an implementation win. Calling the same complete element validator for every emitted tag merely moves QName, normalization and scalar work, adding tag scratch construction. A typed serializer could fuse escaping with proof, but must preserve the actual append boundaries: `SvgByteBudget::appended_len` exposes the first rejected append end as the resource error's actual value. Normal SVG byte identity alone cannot admit changed fragmentation. Root opening, namespace normalization, split text, theme/root edits and opaque processors need explicit generation ownership. No new writer framework is retained.

The locked quick-xml 0.42.0 already switches raw duplicate-name checks to a hash prefilter after a fixed 32 attributes. The conjectured ordinary all-unique quadratic attribute term does not exist in this locked source. No replacement duplicate checker is introduced.

The facade freezes one session report, then unnecessarily clones it before consuming its original owner for diagnostics. Consuming decomposition is a legitimate later ownership repair; its report cardinality is bounded and it is not the full-SVG regression owner. Resource and artifact hashes bind different contracts; metadata document/target/admission hashes are not repeated full-SVG serialization.

## Reference-name candidate: diagnostic only

The reference collector defines exactly seven local-name inputs: `id`, `href`, `d`, `points`, `marker-start`, `marker-mid`, `marker-end`. After all XML, namespace, normalization, scalar and duplicate checks, unrelated attributes can skip the second namespace resolution and out-of-line observer call. Native compatibility processing remains unchanged. Independent review finds no semantic blocker; all 78 focused tests and all-target renderer Clippy pass, with the full feature set. The legacy unfiltered oracle covers aliases, rebinding, malformed unrelated attributes, reference expansion and cancellation.

All 152 render/end-to-end SVG identities match exactly; each binary emits 459 preflights. Native pipeline executable size is 40,279,040 / 40,285,184 bytes, an increase of 6,144 bytes. Three alternating long diagnostic pairs observe mixed movement. These are not calibrated confirmation results and establish neither a stable speedup nor a regression. The exact single-file patch is reversed after source/diff verification and remains frozen in the isolated worktree and ignored ledger.

## Exact UTF-8 character algorithm

For a valid Rust string, XML 1.0 scalar membership is exactly equivalent to excluding ASCII controls other than TAB/LF/CR and the sequences for U+FFFE/U+FFFF. Surrogates cannot occur; noncharacters in other planes remain accepted. The candidate scans scalar-aligned blocks of at most 256 bytes with one complete fold for forbidden controls and the presence of EF. Only EF-containing blocks search for the two exact forbidden sequences. Only a proven-invalid block uses the original scalar predicate to find the first invalid character. Error pass/message and error-before-publication checkpoint remain unchanged.

Time is O(B) for normalized string bytes B, with O(1) additional scratch. This is an exact whole-domain implementation, not an ASCII fallback. A 256-byte block can undergo up to four scans, so the cancellation work bound is at most 1,024 logical byte visits plus at most three alignment retreats; the old absolute checkpoint count is not claimed. Outer entity/attribute normalization, QName, XML framing, namespace, structure and reference checks remain complete.

The first isolated variant searches for both forbidden noncharacters in every block. It passes complete-domain checks but regresses short attributes, and is rejected. The second variant only performs those exact searches when EF is present. No size, family, content-policy or Unicode scalar fallback is added.

Both variants pass exhaustive checks over 1,112,064 valid Rust scalars, 13,284 boundary/ordering cases and 164 independent cancellation cases. Sixteen alternating isolated pairs per control observe the following median candidate/reference ratios; these are diagnostic private-function results, not public-operation latency claims:

| Variant | Short attributes | ASCII CSS | Dense Unicode |
| --- | ---: | ---: | ---: |
| Always search | 1.2148 | 0.1278 | 0.2645 |
| EF-gated search | 0.6845 | 0.0553 | 0.2658 |

The production candidate changes only `validate_xml_characters` and four tests. Independent review finds no blocking issue. All 81 focused terminal/standalone tests pass with nextest; the same freshly built test binary also passes all 307 pipeline tests and all 3,317 renderer tests, with two pre-existing ignored tests. These scopes overlap and must not be added together. Renderer code uses optimization level zero in the correctness-only release dependency profile; performance binaries use the unchanged optimized production bench profile. All-target renderer Clippy with all diagram/layout/math features and warnings denied, cargo fmt and diff checks pass.

Both frozen optimized binaries emit 459 preflights; all 152 render/end-to-end SVG identities match exactly. The executable decreases from 40,279,040 to 40,271,872 bytes: -7,168 bytes (-0.018%). This is a matching native executable control, not a packaged/WASM size claim. Source, patch and binary hashes are frozen in the experiment ledger.

End-to-end sampling has not started. Another workspace repeatedly starts Rust compilation; two attempted starts are stopped by the pre-timing compiler guard before producing samples. No user-owned process is terminated. The private function diagnostics do not substitute for public timing, and uncommitted source receipts do not substitute for clean-revision confirmation. The candidate is committed with `confirmation pending`, not accepted as a performance repair. No push is performed.

The next measurement must use this exact candidate and the frozen base, the same inputs, capabilities and lifecycle, with no compiler overlap. Start with the registered three-pair discovery; only a material signal justifies calibrated A/A and fresh power-sized confirmation of at least eight alternating pairs. Keep unrelated reference-name filtering separate. If public benefit is absent or a control regresses, remove only this owned candidate after checking its current source/diff identity; do not retain an unhelpful fast path. Complete shared DOM/strict qualification remains required before final performance integration; this checkpoint does not claim it was rerun for the pending candidate.

## Evidence boundary

Ignored artifacts are in `target/bench/experiments/svg-event-facts-20261015/`: experiment.yaml, event-facts-design.txt, facade-ownership.txt, reference-source/patch/frozen/verification records, reference-diagnostic/, xml-byte-probe/v1/, xml-byte-probe/raw-v2.jsonl and its receipt, xml-byte production patch/source/test records. The original candidate patches were frozen before commit and are not clean-revision confirmation receipts. New confirmation must identify the committed revision independently. The host has concurrent user-owned Unity/editor and another workspace's Cargo activity; no such process is terminated. No timing is started while an observed compiler is active, and host interference cannot be interpreted as candidate causality.
