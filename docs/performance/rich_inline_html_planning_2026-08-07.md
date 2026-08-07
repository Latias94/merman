# Rich inline HTML planning decision — 2026-08-07

## Decision

Status: **accepted-structural**.

The opaque rich-inline planner now indexes source-backed style-group ends once and keeps every
opaque measurement request on a borrowed slice of one operation-local logical source string. The
change removes repeated all-fragment style discovery and growing scratch-payload construction
without reducing host/fallback callback count, order, request text, style, or failure position.

This receipt claims a structural Rust-planning bound only. It makes no end-to-end latency or peak
memory claim, and it does not treat the extra `usize` side index as a measured memory win.

## Reference and revision boundary

The semantic reference is Mermaid 11.16.0's
`repo-ref/mermaid/packages/mermaid/src/rendering-util/createText.ts`. Its HTML path measures a
nowrap label first and switches to `break-spaces` at the width boundary. The Rust implementation
keeps that staged behavior; the opaque host callback trace is an additional Merman observability
contract and therefore is tested independently rather than optimized away.

| Role | Commit | Meaning |
|---|---|---|
| `A` | `e7f03f8bf15f907bd4a3dab97ea8f3f695884742` | Direct parent with the prior built-in planner and opaque all-run scan/scratch path. |
| `B` | `da1378b70f12987e5ca5aba7e584a61f1fb556fd` | Candidate that adds source offsets, one opaque style-group index, borrowed request slices, and counter-backed regression coverage. |
| prior U8 context | `156eb88b2243e99e6761bec1aa3af1c398b5fd20`, `9f6af1843919104defa01be1acfdcb46813c2b2e`, `685dd3ce1f6d37c85e24a572063ccae7b2926633` | Earlier U8 planner, built-in route, and inactive-path milestones already present on `A`. |

`B` is a direct child of `A`; the focused tests and this receipt are the decision evidence for the
opaque completion slice.

## Structural bound

Let `B` be logical UTF-8 source bytes, `R` styled runs, `K` break segments, `F` indexed fragments,
and `Q` opaque backend requests.

- Before, each candidate line repeatedly discovered same-style runs and checked fragment
  contiguity. A fragmented candidate could therefore rescan `F` per request and copy growing
  payloads, with reachable Rust-side work containing `F * Q` and cumulative scratch bytes
  containing a `B * K` term.
- After, `index_inline_run_breaks` builds one concatenated source and one forward fragment table;
  `index_opaque_inline_style_groups` visits each fragment once from the end and stores one group
  end. Each opaque request performs one constant-time group-end lookup and one `&str` slice. Rust
  planning is `O(B + R + K)` plus explicitly observed opaque backend work `Q`; submitted backend
  bytes and callback execution remain separate, observable work.
- The side index adds exactly one logical `usize` slot per fragment (`O(R + K)` space). The source
  string is operation-local and is retained only for the existing plan lifetime. No unbounded or
  process-global cache is introduced.

## Exactness controls

The focused suite covers:

- empty, single-run, mixed-style, same-style cross-run, Unicode byte boundaries, CRLF, entities,
  combining marks, emoji, RTL, and large alternating-style inputs;
- min-content width, natural width, no-wrap, max-width overflow, candidate rollback, and line
  count;
- recording, stateful, and failing opaque measurers, including exact request sequence and failure
  position; and
- qualified built-in carrier paths, including ULP wrap boundaries and actual backend-report
  counts.

The same-style ASCII trace remains exactly:

```text
aa | bb | cc | aa | bb | cc | aa | aa bb | aa | bb cc | bb | cc
```

For the fixed `B = 4096`, `R = 32`, `K = 32` workload, the pre-change red proof observed
`F = 63`, `Q = 128`, `fragment_measure_visits = 810`, and `measurement_payload_copy_bytes =
16003`. The candidate's matrix assertion requires exactly `F + (Q - R)` fragment visits and
zero payload-copy bytes. The test-only source-pointer check makes a future scratch allocation
fail the zero-copy assertion instead of relying on a default counter value. A second case combines
same-style cross-run UTF-8 fragments with a real width-boundary rollback.

## Verification

- `CARGO_BUILD_JOBS=1 cargo nextest run --locked -p merman-render --all-features -E 'test(inline_planning_tests)' --test-threads=1`: **16 passed**, 1,545 filtered.
- `CARGO_BUILD_JOBS=1 cargo clippy --locked -p merman-render --all-features --all-targets -- -D warnings`: passed.
- `cargo fmt --all -- --check`: passed.
- `git diff --check`: passed before the implementation commit.
- Full render crate control: **1,555 passed, 1 failed, 4 skipped**. The one failure is the existing
  aggregate Mindmap layout-golden test with the same five fixture mismatches reproduced before
  this opaque completion slice (`stress_label_escaping_012`, `stress_multiline_nodes_006`,
  `stress_shapes_mix_003`, `stress_unicode_punct_004`, and the upstream Cypress multiline fixture).
  Those goldens were not regenerated because they are unrelated user/reference changes.
- Independent correctness and performance reviews found no P1/P2 issue. The performance review
  records only the expected `O(R + K)` side-index constant and no unmeasured latency/memory claim.

No adjacent latency experiment was registered; this is not an `accepted-latency` result. The
ignored raw experiment ledger is `target/bench/experiments/u8-opaque-inline-style-groups/experiment.yaml`.

## Residual risks and boundaries

- Opaque backends remain free to perform `Q` requests and receive the same cumulative payloads;
  reducing those calls would violate the current host observability contract.
- Highly fragmented inputs pay the side-index `usize` constant. A future peak-memory candidate
  must measure that trade-off with a clean memory lane rather than infer it from this structural
  proof.
- Browser `getBoundingClientRect`, fonts, `foreignObject`, and DOM wrapper noise remain bounded
  parity residuals and are outside this Rust-planning receipt.

The pre-existing untracked `rust_out`, `test-results/`, and unrelated fixture/document changes
were not staged, modified, removed, or used as evidence.
