# Compact LSP line index (2026-09-17)

## Scope and decision

The semantic-token planner now stores one `usize` line-start offset per logical line instead of
three offsets (`start`, `content_end`, and `end`). It reconstructs each requested line's boundaries
from the next start and the source terminator. The index remains local to each request; no cache,
public API, dependency, supported capability, resource limit, or cancellation policy changes.

This follows the [baseline attribution](lsp_line_index_attribution_2026-09-17.md). Admission is based
on reduced index storage with equivalent behavior. Handler timings are adjacent non-regression
controls, not a separately admitted latency speedup or a whole-process memory claim.

## Representation and semantic argument

Let `B` be source bytes and `L` the number of logical lines, including the empty final line after a
terminator. The old line table holds `3 * L * size_of::<usize>()` initialized bytes; the candidate
holds `L * size_of::<usize>()`. Both construct in `O(B)` time and occupy `O(L)` request-local space.
Reconstructing a line is `O(1)`; locating the line containing a byte remains `O(log L)`.

The starts vector always contains zero and one strictly increasing offset immediately after each
LF, isolated CR, or CRLF pair. The next start is the current line's end; bounded checks for LF and
then CR recover the content end. Checks cannot move before the current start, preserving adjacent
empty lines and the trailing EOF line. The last unterminated line has no terminator to strip.

`usize` remains appropriate for configurable source limits. No assumption that every deployment
uses the default 4 MiB limit is introduced. UTF-16 conversion and invalid-range error strings stay
unchanged. Full, delta, and range requests still share the same planner and token projection.

## Registered experiment

- Base source: `0a7bcf4ada1cd2a50f89266176f665354ef1dfc3`; candidate: its focused
  `crates/merman-lsp/src/semantic_tokens.rs` patch and three boundary regression tests.
- Windows x86-64, Intel Core i7-11700; Rust 1.95.0, `x86_64-pc-windows-msvc`.
- Release, locked dependencies, default features; two Cargo build jobs. No Cargo work ran during
  measurements. Prior attribution documentation was the only initial working-tree difference.
- The saved immutable baseline executable avoids switching revisions in the active checkout.
  Candidate probes reuse the same fixture generation, token capabilities, and request handlers;
  only representation-specific memory field reads change in the private probe.
- The six fixtures and pure-LF boundary probes are byte-identical to the attribution workload:
  64 KiB, 1 MiB, and 4 MiB, ordinary 80-byte lines or dense LF padding after the same Mermaid fence.
  The measured range is `(1, 0)..(3, 0)`, including an emoji label.
- Eight fresh process pairs alternate base/candidate and candidate/base. Each process has two
  warmups, nine samples of ten repeated handler requests, and nine incremental edits each followed
  by one request. Pull diagnostics prevent background analysis; one request is in flight.
- Primary gate: initialized index bytes exactly one third of baseline and reserved index bytes no
  more than one third, on every registered fixture and pure-LF boundary. No persistent state.
- Control gate: for each fixture, reject if the geometric mean of paired candidate/base run-median
  ratios exceeds 1.10 **and** the mean paired absolute increase exceeds 50 us, separately for hot
  requests and paired edit-plus-request totals. This gate does not admit a latency improvement.
- Source hashes, packed token outputs, edited document versions, and zero background diagnostic
  execution are checked. The new boundary tests cover empty input, LF/CRLF/CR, mixed empty lines,
  EOF, emoji and combining characters, exact UTF-16/range errors, and multiline capture splitting.

## Results

All eight pairs passed the registered memory and handler-control gates. Every source digest and
packed output matched between implementations. On this 64-bit target, initialized bytes fall by
exactly **two thirds**, from 24 to 8 bytes per line. Measured reserved capacity also falls by exactly
two thirds on all nine registered shapes.

| Source | Shape | Baseline reserved | Candidate reserved |
| --- | --- | ---: | ---: |
| 64 KiB | Ordinary | 24,576 B | 8,192 B |
| 64 KiB | Dense LF | 1,572,864 B | 524,288 B |
| 1 MiB | Ordinary | 393,216 B | 131,072 B |
| 1 MiB | Dense LF | 25,165,824 B | 8,388,608 B |
| 4 MiB | Ordinary | 1,572,864 B | 524,288 B |
| 4 MiB | Dense LF | 100,663,296 B | 33,554,432 B |
| 64 KiB | Pure LF boundary | 3 MiB | 1 MiB |
| 1 MiB | Pure LF boundary | 48 MiB | 16 MiB |
| 4 MiB | Pure LF boundary | 192 MiB | 64 MiB |

Handler controls below report the median of eight run medians in microseconds. Ratios use the
geometric mean of the eight paired candidate/base ratios, so they need not equal the ratio of the
two displayed medians. The 64 KiB ordinary hot-request mean paired change is +1.91 us, below the
50 us absolute guard; all other hot-request mean paired changes are negative.

| Source | Shape | Base hot range | Candidate hot range | Paired hot ratio | Paired edit + range ratio |
| --- | --- | ---: | ---: | ---: | ---: |
| 64 KiB | Ordinary | 59.42 | 57.98 | 1.0293 | 1.0007 |
| 64 KiB | Dense LF | 794.56 | 163.87 | 0.1991 | 0.6437 |
| 1 MiB | Ordinary | 544.12 | 570.24 | 0.9596 | 0.9717 |
| 1 MiB | Dense LF | 11473.60 | 4240.05 | 0.3688 | 0.7043 |
| 4 MiB | Ordinary | 2642.50 | 2266.97 | 0.8682 | 0.9634 |
| 4 MiB | Dense LF | 46068.64 | 16023.70 | 0.3488 | 0.7060 |

These timing observations support the non-regression check for this memory repair. They do not
replace noise-calibrated latency admission, claim editor responsiveness across workloads, or
compare against the different run order and host conditions of the earlier attribution receipt.

## Limits and next work

Capacity accounting excludes allocator metadata, relocation peaks, syntax trees, source storage,
and other allocations; it is neither cumulative allocation nor RSS. Vector growth remains a
factor: the pure-LF EOF line still crosses a capacity boundary. Per-request construction still
scans the full source, and cancellation checkpoints are unchanged.

The handler corpus is a fixed early range in one Markdown fence. It does not establish full/delta
throughput, multi-fence behavior at scale, transport latency, or other-host performance. Existing
full/delta/range functional tests protect those protocol paths, but no broad timing claim follows.

Snapshot-level reuse remains a separate hypothesis. It would require explicit retained-memory
accounting and evidence that repeated requests per unchanged snapshot justify the retained state.
For one request per edit, rebuilding a cache once per snapshot does not amortize index construction.
Do not expand this focused representation repair into a cache without that evidence.

## Validation and provenance

- Release default-feature library: **234 passed**.
- Complete LSP crate with `stdio`: **321 passed**, including protocol integration and process tests.
- Final three boundary tests passed again after a test-only local type alias resolved Clippy's
  type-complexity warning; the measured production implementation is unchanged.
- `cargo fmt --all -- --check`, `cargo clippy --locked -p merman-lsp --all-targets --features stdio -- -D warnings`,
  and `git diff --check` pass. Independent review found no semantic or bound issue.
- Renderer/SVG sweeps were not rerun: no shared parser, layout, renderer, fixture, or comparator
  changed. The owner surface is LSP semantic-token planning.
- Temporary probe includes were removed by matching their exact added suffixes. `server/tests.rs`
  is byte-identical to its baseline; the only retained Rust change is the index and its regressions.

Ignored evidence lives in `target/bench/experiments/lsp-compact-line-index-2026-09-17/`:
`experiment.yaml`, probe sources, measured/final patches, `candidate.exe`, sixteen raw process logs,
`summary.json`, `digests.json`, and correctness/lint logs. The baseline executable remains in the
adjacent `lsp-line-index-attribution-2026-09-17` ledger. These local artifacts are not CI tooling.

The executable command for each side is `probe.exe --ignored --test-threads 1 --nocapture`
(or `candidate.exe` for the candidate). Alternate the two sides for eight pairs; do not run Cargo
concurrently. Probe construction uses test-only includes in the semantic-token and server test
modules, followed by `cargo test --locked --release -p merman-lsp --lib --no-run`.

SHA-256 (raw-log digests are individually preserved in `digests.json`):

| Artifact | Digest |
| --- | --- |
| `Cargo.lock` | `a910ef1851947cbfe6b1c8e3ed97bb7f35e35efbfe2b074427779be9bb9b581a` |
| `baseline executable` | `c789a811c5cd154e59c7d25572bc118527f4f656dddc750db91f99e8be02c038` |
| `measured patch` | `56be4755568ae63ed882e2243e0075bc8b482801266d8bd49ec48c49a8586ee5` |
| `final patch` | `a05050c387677c7caeb3fa9945e6e6b34581bf5ab9db6878cf3e1146f3988246` |
| `candidate.exe` | `e54582ef2f843114689e3e59eb657d306a6866658c5f96c39441650da60e36cc` |
| `probe.rs` | `904e0a321a1b46f09e1b6ca701ba557a2e022a197cce79db797a4e19050801dc` |
| `handler_probe.rs` | `2592867e7b2b187241c233d0f31a1bb1695942b4c8dad75f9610213a64f135a0` |
| `summary.json` | `6d25c5a429a9194f32a75c5a99660b3aeedcba392670b0fc124b0836f4d36f62` |
| `experiment.yaml` | `8ef28b640ac21f4873ff60bc65ef31c3676b74fc47f329bfc228103c0ccfefbc` |
| `digests.json` | `8d8fff5bed1cc4f2bfc2282ce39316de57eff82d6b7c6622b1eebe77baf4f8f6` |
