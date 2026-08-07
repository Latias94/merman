# Requirement Markdown Width Reuse Disposition — 2026-08-07

## Decision

Status: **rejected-as-written**, **rejected-upper-bound**.

The historical hypothesis in
`573fbbb55a516fbf4f606b3c004967cabb342092` is not admissible on the current
text-measurement contract. It removes one complete width request from the shared Markdown helper
for every `TextMeasurer`, including opaque custom and host routes whose request count, order,
stateful return values, failure position, and measurement-report provenance are observable.

A family-local re-derivation limited to the exact built-in operation carrier is semantically
possible, but it does not qualify for production work. On the current `requirement_medium` path,
the most optimistic measured cost of the two sites removed by the historical diff is 13.249 us,
or 7.38% of the complete public operation. The zero-noise minimum low-latency gate is still 10%
and 17.954 us. A/A noise could only raise those thresholds.

No production candidate, adjacent A/B pair, candidate-only test, cache, prepared field, or host
exception is retained. The shared helper continues to execute both requests because changing it
globally would be a behavior regression; Requirement does not add an owner-local built-in reuse
path because its current-base upper bound cannot clear admission.

## Reference and revision boundary

The semantic reference is Mermaid 11.16.1 at
`repo-ref/mermaid@7ecca0cd7f1658ef74f4e7e91f925724ef403bbf`.

| Role | Commit | Meaning |
|---|---|---|
| historical base | `2d2e891c77a924e78702302bd22ca48bee6dff02` | Parent of the unmerged width-reuse hypothesis. |
| historical hypothesis | `573fbbb55a516fbf4f606b3c004967cabb342092` | Moves `raw_parsed` instead of cloning it and reuses the first line-width result instead of issuing the second width request. |
| profiled production revision | `a74da31da440b947b986ab8e0ff7f80658a6cb26` | Frozen bench executable used for the current-base screen. |
| reviewed current base | `8cbabee58a90b736d82e8537442c71e8779a0c10` | Branch revision at disposition; the commits after the profiled revision change only Sequence tests and the Sequence/Mindmap U9 receipts, not Requirement, shared text metrics, the pipeline bench, or the fixture. |

The historical diff is not an ancestor of the reviewed branch. Its production diff SHA-256 is
`f6ebe7d1babb5581e6dccc91134d25dabc819ef38394b312fba0bdfc52ffdf96`.

Evidence identities:

- frozen executable SHA-256:
  `0d5c9bf1359d25ec37939f50643e49d10fdbd9fc45f6f6a6b5fbdbd02e2fb641`;
- `requirement_medium.mmd` SHA-256:
  `219c5d15060bc00139be5aab4737f1203badfb8f4ac02ceb582970096ed3f0ac`;
- current `requirement.rs` SHA-256:
  `9fe460ea332278e3b494e1f4cddb160c5fdb657052e178b22a2b3a4fd47a63c2`;
- current `text/metrics.rs` SHA-256:
  `3b58091889d985301d5db85e92bcecb4d86e056444c61480eb99ce08803b8abe`;
- `Cargo.lock` SHA-256:
  `4eb38cee29796405570fa3172fffd049429fdef5692f868fd6be02e45ee93a71`.

The screen ran on macOS 26.5.1 build 25F80, Darwin 25.5.0 arm64, Apple M4 Pro, with Rust 1.95.0
and Cargo 1.95.0. The executable uses the Cargo `bench` profile. No Cargo command was run for this
disposition; the already frozen current-production-equivalent executable was reused.

The authoritative date for this disposition is **2026-08-07**. Automatic later Git timestamps on
the reviewed branch are local clock drift and are not used to order or date the evidence. Git
ancestry, source/executable hashes, fixture identity, and the recorded commands are authoritative.
The later `59cd7da08` correction updates only the observable built-in profile version string to
Mermaid 11.16.1; it does not change this measured Requirement width path or its upper bound.

## Source-backed behavior boundary

Pinned Mermaid's
`packages/mermaid/src/rendering-util/rendering-elements/shapes/requirementBox.ts` calls
`addText(...)` independently for each non-empty type, name, body, and relationship label.
`addText(...)` computes the wrapping width with `calculateTextWidth(inputText, config) + 50`, calls
`createText(...)`, then reads the resulting SVG or HTML bounding box. Empty fields return before
creating a label. Merman retains those meaningful label and wrap decisions; headless built-in
metrics approximate the browser measurements, while opaque routes remain observable Merman host
contracts.

The current Requirement owner already has a separate, accepted prepared-artifact boundary:
layout measurements may be reused during SVG emission only when
`RequirementLabelMeasurementBinding` matches the exact built-in profile, phase, operation, font
family, and font size. Host and custom measurers return no carrier and replay the SVG-stage
requests. That existing behavior is not the hypothesis reviewed here.

`573fbbb55` instead changes the shared
`measure_markdown_with_inline_styles_impl(...)` helper. In the non-manual-wrap HTML path it:

1. moves `raw_parsed` instead of cloning the nested word vectors; and
2. substitutes the first `max_line_width` result for the later unwrapped-width request used by the
   `needs_wrap` branch.

The second step reduces an arbitrary measurer's callback trace. The historical test explicitly
expects fewer wrapped calls from a custom counting measurer, confirming that this is observable
rather than a built-in-only implementation detail. Therefore the diff is rejected as written
before timing.

## Current-base upper-bound screen

The medium fixture contains 12 non-empty Requirement/Element node lines and two relationship
labels. Each of the 14 labels reaches the non-manual-wrap HTML path once during built-in layout.
The historical diff removes exactly these two compiled call-site subtrees:

- the nested `raw_parsed.clone()` return site at
  `measure_markdown_with_inline_styles_impl + 2556`; and
- the second `markdown_word_line_plain_text_and_width_px(...)` return site at
  `measure_markdown_with_inline_styles_impl + 4968`.

The offsets were verified against the frozen executable's disassembly. Counting every sample in
both subtrees as removable is deliberately optimistic: it includes allocator and nested backend
work, and it does not charge the replacement tuple/`Option` bookkeeping introduced by the old
diff.

Three independent `/usr/bin/sample` screens used the existing Criterion
`layout/requirement_medium` profile loop, a three-second sample window, and a one-millisecond
interval:

| Run | Total samples | Clone subtree | Second-width subtree | Optimistic removable share of layout |
|---:|---:|---:|---:|---:|
| 1 | 2,252 | 9 | 259 | 11.901% |
| 2 | 2,255 | 16 | 260 | 12.239% |
| 3 | 2,262 | 19 | 233 | 11.141% |

The same frozen executable measured:

| Public or attribution lane | Criterion interval | Point estimate | Output identity |
|---|---:|---:|---|
| `layout/requirement_medium` | 107.17-108.25 us | 107.64 us | prepared layout SHA-256 `aa30d74de460cea2f09a3ac970e4b5e83aee2815cad56ca41595de2ee56d51fb`, 46,916 bytes |
| `end_to_end/requirement_medium` | 179.54-181.47 us | 180.40 us | SVG SHA-256 `9c849756a5294780d08e1de992c1da7aaa6eee8fe321c68bb9b6bddedcb1aa32`, 15,350 bytes, 115 elements |

To favor admission, apply the largest sampled removable share to the upper layout estimate and
compare it with the lower public estimate:

```text
optimistic removable cost = 108.25 us * 12.239% = 13.249 us
optimistic public saving  = 13.249 us / 179.54 us = 7.379%
minimum low-latency gate  = 179.54 us * 10% = 17.954 us
```

The candidate reaches only 73.8% of the zero-noise absolute threshold. Its removable-site share
would need to rise from 12.239% to 16.586% of layout, a 35.5% relative increase, merely to reach
the optimistic minimum gate. Independent A/A noise would increase both the relative and absolute
requirements under the checked-in low-latency formula.

The screen is sufficient for pre-admission rejection; it is not presented as adjacent A/B causal
evidence or as an accepted latency claim. A compliant Requirement-only built-in carrier could
remove no more backend work than these same two sites and would add owner/key validation, so its
reachable upper bound is no larger than the rejected screen.

## Commands and evidence disposition

The read-only commands were equivalent to:

```text
pipeline --bench --exact layout/requirement_medium --sample-size 10 \
  --warm-up-time 1 --measurement-time 1 --discard-baseline

pipeline --bench --exact end_to_end/requirement_medium --sample-size 30 \
  --warm-up-time 2 --measurement-time 3 --nresamples 10000 --discard-baseline

pipeline --bench --exact layout/requirement_medium --profile-time 6 \
  --discard-baseline
/usr/bin/sample <pid> 3 1 -file /dev/stdout
```

No test or Cargo command was needed because no source code changed. No production residue search
found `raw_parsed_for_width`, and the old branch remains isolated. The ignored experiment ledger is
`target/bench/experiments/u9-requirement-markdown-width-reuse-v1/experiment.yaml`.

## Residual boundary

The remaining Requirement text cost is result-producing work: Mermaid-compatible width selection,
Markdown/entity projection, styled width, wrapped dimensions, and Dugong layout. The existing
built-in layout-to-SVG prepared-artifact reuse remains valid and independently guarded. Revisit
this closed hypothesis only if a materially larger public Requirement workload or a documented
high-volume integration is registered before measurement, or if a new profile identifies a
different owner-local term. Do not reopen the global callback-reducing implementation.
