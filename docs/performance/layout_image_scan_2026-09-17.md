# Flowchart layout image scanning (2026-09-17)

## Scope and proof

This follow-up to the SVG image-normalization repair targets the separate layout helper
`measure_flowchart_html_images` in `flowchart/label.rs`. The base is `09b9f257d` on
`perf/label-scan-and-math-measurement`. The admission class is structural work reduction;
no latency speedup or peak-memory reduction is claimed.

Let L be the wrapped HTML fragment's byte length and K the tag starts reached by the helper.
Previously each tag triggered lowercase conversion of the entire remaining suffix, just to test
`<img` and `<br`. Summing suffix lengths costs O(KL), reaching Theta(L squared) with repeated
fixed-size tags. The mixed-label image branch is reachable from ordinary Flowchart node/edge
label measurement; one image after many spans suffices. Single-image labels use the existing
fixed-width path instead.

The candidate finds the first closing `>` once per tag and compares only four or three bytes
case-insensitively. Each successful search consumes the entire searched span. An unsuccessful
search appends the remaining text verbatim and stops; the old loop would append it character by
character because no later tag could close. Thus delimiter search spans do not overlap, and
prefix work is constant per tag. Image-src checks operate on disjoint image tags, while text
assembly and normalization also process O(L) total bytes. The helper's preparation is O(L), plus
the unchanged opaque text-measurer work; live space remains O(L), without additional retained state.
This does not establish a bound for the complete HTML/render pipeline.

The existing paragraph wrapper means a raw label with a trailing unclosed tag still encounters
the `>` of `</p>`. That historical consumption behavior is preserved. No helper API is exposed
just to exercise an otherwise unreachable tail branch.

## Compatibility

Fixed-byte predicates preserve the original loose prefix acceptance, including `<IMGx>`, `<BRx>`,
and Unicode after a valid ASCII prefix. They do not impose a new tag-name boundary. UTF-8 slices
that end inside a character cannot match an ASCII prefix and safely return `None`.

The first `>` still closes a tag even inside an attribute quote. Src detection, empty/missing
sources, whitespace and NBSP normalization, image/text block order, fixed 80px image width,
mixed-image width, metric rounding, and custom-measurer call arguments remain unchanged.
Sanitization, parsing, image loading policy, and layout algorithms are untouched.

Two regression tests run through the existing label-layout entry point. A recorded custom
measurer checks exact text payloads, order, width/mode arguments, and resulting metrics across
15 boundary cases. A four-scale test compares repeated tagged Unicode text plus an image with
the same plain Unicode text plus an image. Both tests pass on the original implementation,
freezing behavior before changing the scan.

## Work evidence

The ignored ledger is `target/bench/experiments/layout-image-scan-2026-09-17/`. Its `scan-work.json`
counts suffix lengths and disjoint tag-search spans for exactly the registered wrapped fragments.
The prefix column is a conservative upper bound of seven compared bytes per encountered tag.
These are source-derived byte counts, not timings or measured allocations.

| Spans | Fragment bytes | Old suffix lowercase bytes | New prefix bound | Delimiter scan bytes |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 35 | 112 | 35 | 33 |
| 32 | 500 | 17,193 | 469 | 436 |
| 512 | 7,700 | 3,960,873 | 7,189 | 6,676 |
| 4,096 | 61,460 | 251,887,657 | 57,365 | 53,268 |

The one-time whole-fragment scans remain O(L). No cache, persistent index, feature, dependency,
public API, or resource-accounting policy is added.

## Validation

- The two new regression tests passed on both the original implementation and the candidate.
- Full `merman-render` nextest with `math,layout-cytoscape`: 1,760 passed, two skipped, including
  layout snapshots and public image SVG tests. The completed run used `CARGO_BUILD_JOBS=2`
  and four test threads after an initial build was interrupted for excessive linker concurrency.
- Render-scoped Clippy with `math,layout-cytoscape`, all targets, and warnings denied passed.
  Workspace formatting and diff whitespace checks also passed.
- Full `compare-all-svgs --check-dom --dom-mode structure --dom-decimals 3
  --diagnostic-browser-text-layout` passed on Windows, using the existing reviewed browser-text-layout
  residual policy. No baselines, comparator rules, or accepted residuals were changed.
- Independent source review found no blocking issue in the implementation, UTF-8 handling,
  malformed-tag equivalence, custom-measurer behavior, or linear-work proof.

The earlier workspace `verify --strict` attempt stopped at the pre-existing
`clippy::field_reassign_with_default` in `merman-export/src/lib.rs:3234`, documented in the
[preceding repair receipt](label_scan_math_measurement_2026-09-17.md). That unrelated blocker was
not changed or rerun here; this repair uses render-scoped static checks, tests, and DOM parity.

## Ordinary public controls

The unchanged `pipeline` harness ran `end_to_end/flowchart_large` and
`end_to_end/sequence_medium` with default features plus `svg`, on Windows 11 / Intel i7-11700 /
Rust 1.95.0. Base and candidate each ran four times per fixture in balanced AB/BA order
(16 invocations total), with 30 Criterion samples, a two-second warmup, and a three-second
measurement window per invocation. Both executables were built from the same checkout before
and after the production edit, with the same regression-test additions excluded by the bench
configuration. Input, source, executable, output-identity, and raw-sample receipts are in the ledger.

| Control | Median paired head/base | Median paired delta |
| --- | ---: | ---: |
| flowchart_large | 0.9683 | -730.10 us |
| sequence_medium | 0.9930 | -2.53 us |

Every pre/post output-identity check passed; base and candidate SVG bytes, hashes, and element
counts matched for both fixtures. Neither control breached the registered joint
`>10% AND >50 us` investigation threshold. These are diagnostic non-regression observations:
there was no A/A power calibration, only four pairs, and these ordinary fixtures do not isolate
the image stress shape. They do not admit a latency improvement claim.
