# Bounded syntax-snapshot line-index reuse (2026-09-18)

## Decision

Keep the candidate under structural repeated-work and retained-byte admission. An immutable syntax
snapshot can reuse a completed compact line index within a separate **8 MiB per-session budget**.
Budget denial and contended initialization fall back to request-local construction. No token
results, cancellation errors, or partial indexes are cached.

This follows the [remaining-work attribution](lsp_index_reuse_attribution_2026-09-18.md). All
semantic/resource gates and eight paired handler controls pass. Timing observations support the
candidate, but are not a separately noise-calibrated latency speedup admission or a promise about
editor responsiveness across workloads.

## Ownership, bounds, and fallback

- `LanguageSession` owns the budget. It is separate from the existing analysis-result cache and
  does not silently use that cache's allowance. The fixed internal 8 MiB allowance admits multiple
  ordinary document indexes while rejecting the measured 32 MiB dense index; it was selected
  before timing and is not a new public configuration API.
- `SyntaxDocumentState` owns an `Arc` cache shared by clones of exactly that snapshot. Parse and
  update create a fresh cache, even when source text or client version happens to match. URI and
  client version are not sufficient cache identity; existing epoch/Arc stale-result checks remain.
- Construction runs inside the existing bounded blocking executor. No source scan holds the
  session state lock. A per-snapshot `try_lock` allows one request to retain an index; contenders
  immediately build local indexes instead of waiting for initialization. Initial requests may
  duplicate transient work within existing worker limits, but only one index is retained.
- Admission uses checked arithmetic for actual `Vec` capacity, the index wrapper, and two `usize`
  Arc counters. A retained index owns its budget lease; the vector is destroyed before the lease
  refunds its charge. Old snapshots or requests holding an index continue to count until its last
  Arc disappears. A cache accessed with a different budget identity builds locally instead of
  borrowing another session's allowance.
- There is no eviction policy. Admitted live snapshots keep their allowance; later snapshots fall
  back while it is full and retry admission on subsequent requests. Closing or updating a document
  frees allowance only when old references are gone. This is bounded retention, not an LRU policy
  or a claim about optimal hit rate across many open documents.
- The bound covers charged retained indexes, not allocator metadata, transient construction,
  fixed empty-cache wrappers, source/tree storage, or total process memory. On this target, the
  index wrapper and Arc counters add 56 bytes to each admitted vector's capacity charge. The Arc
  allocation itself completes destruction immediately after its embedded lease drops; the counter
  is a lifetime-accounting policy rather than a per-instruction RSS ceiling.

For `B` source bytes and `R` sequential requests against an admitted unchanged snapshot, construction
changes from `O(R * B)` to one `O(B)` preparation. Capture and token projection still run on every
request. Denied or concurrently initializing snapshots retain the original `O(B)` local work per
request. Per-snapshot local construction remains `O(L)` for `L` logical lines, and total charged
retained bytes across current/old snapshots never exceed 8 MiB.

Range validation still precedes the capture-stage cancellation check. A cancelled request can
leave a completed, valid index, but cannot cache cancellation or corrupt later requests. Full,
delta, and range all use the same index path; delta baseline/result semantics are unchanged.

## Registered experiment

- Base: `c461fba13`; candidate: the focused LSP patch and new `line_index.rs`. Initial dirty files
  were the prior authorized attribution documentation only. The saved immutable base executable
  permits comparison without switching revisions in the active checkout.
- Windows x86-64 / Intel Core i7-11700 / Rust 1.95.0 (`x86_64-pc-windows-msvc`), release,
  default features, locked dependencies, two Cargo build jobs. No Cargo work during timing.
- Same ten hashed source/range cases as the expanded attribution: 64 KiB/1 MiB/4 MiB ordinary
  padding; 4 MiB dense LF; first/final ranges; a long host-text line; and first/last of 128 fences.
  The requested fence contains an emoji label. All capabilities, source bytes, and token output
  are matched. The host long line is not a long Mermaid label.
- Eight fresh process pairs, alternating AB/BA. Only the handler test runs in each executable:
  `--ignored --exact server::tests::index_reuse_handler_probe::handler_probe --test-threads 1 --nocapture`.
  Each case records its first range request, two warmups, seven batches of ten unchanged-snapshot
  requests, and seven edits each followed by one range request. First-request timing is after
  document open; it is not process startup. Case order is fixed.
- Pull diagnostics prevent background analysis; one request is in flight, with two Tokio workers.
  Timing includes handler dispatch, planning, snapshot checks, and response creation. Parameter
  creation, source loading, assertions, stdio, and final output destruction are outside timing.
- Candidate-only accounting reads occur outside timed sections: retained bytes must stay within
  8 MiB, dense 4 MiB cases must retain zero, ordinary cases must retain an index, and closing each
  document must return its session charge to zero. Timed calls and sampling match the baseline.
- The preregistered adjacent control rejects a fixture/metric if its geometric mean paired
  candidate/base ratio exceeds 1.10 **and** its mean paired increase exceeds 50 us, independently
  for first request, hot request, and paired edit-plus-range totals. These are diagnostic controls;
  no A/A-derived latency confirmation or percentile claim is made.

## Observations

Hot times are the median of eight per-process sample medians in microseconds. Ratios are geometric
means of paired candidate/base process values, so they do not necessarily equal ratios of the two
displayed medians. Every first/hot/edit control passes its registered joint gate.

| Case | Base hot (us) | Candidate hot (us) | First ratio | Hot ratio | Edit + range ratio | Retained charge (B) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 64 KiB ordinary, early | 71.22 | 24.66 | 0.936 | 0.352 | 0.936 | 8,248 |
| 1 MiB ordinary, early | 580.10 | 19.43 | 0.941 | 0.035 | 1.029 | 131,128 |
| 4 MiB ordinary, early | 2292.53 | 22.66 | 0.970 | 0.011 | 1.017 | 524,344 |
| 4 MiB dense LF, early | 16610.15 | 16606.06 | 1.071 | 1.002 | 0.978 | 0 |
| 4 MiB ordinary, final | 2324.28 | 44.39 | 0.987 | 0.018 | 0.982 | 524,344 |
| 4 MiB dense LF, final | 16113.74 | 16993.15 | 1.036 | 1.037 | 0.960 | 0 |
| 4 MiB host long line, final | 1610.59 | 22.95 | 0.971 | 0.019 | 1.008 | 120 |
| 4 MiB host long line, early | 1586.79 | 40.97 | 0.986 | 0.023 | 0.966 | 120 |
| 4 MiB / 128 fences, first | 2308.22 | 25.83 | 0.968 | 0.014 | 0.954 | 524,344 |
| 4 MiB / 128 fences, last | 2291.21 | 22.55 | 1.056 | 0.009 | 0.972 | 524,344 |

The ordinary 4 MiB index retains 524,344 charged bytes. Dense 4 MiB indexes exceed the budget and
remain request-local, with no retained charge and no claimed construction saving. Warm requests
benefit when admission succeeds; first requests and one request per edit still build an index.
No real-editor request-frequency distribution or multi-document hit-rate claim follows from the
sequential fixture measurements.

## Correctness and resource evidence

**331 release tests with `stdio` passed**, including ten new tests:

- exact capacity/metadata reservation boundaries, overflow rejection, and local fallback;
- separate session budgets and cached index identity;
- a gated initializer plus a contender that must complete locally before the initializer is
  released, with only one retained charge afterward;
- old reader charge after cache drop, competing snapshot refusal, and retry after release;
- initialization panic/poison recovery;
- cached versus zero-budget full/range output and exact UTF-16/range errors;
- cancelled cold/warm callers, unchanged invalid-range error precedence, and later active callers;
- clone sharing, empty caches on updates, and independent snapshot refunds;
- close/reopen with the same URI and version but different source, retaining the old snapshot
  while verifying the new output against zero-budget construction.

Existing tests continue to cover full/delta/range protocol negotiation and output, stale workers,
queued edits, request-local cancellation, task/CPU permits, and stdio lifecycle. Independent review
found no blocking ownership, accounting, cancellation, or lock-order issue.

`cargo fmt --all -- --check`, scoped all-target `stdio` Clippy with `-D warnings`, and
`git diff --check` pass. No parser, layout, renderer, dependency, public API, or SVG comparator
changed, so renderer sweeps were not repeated.

The first debug test build failed during Windows PDB linking (`LNK1318`) when E: had about 75 MiB
free. The two failed PDB outputs were preserved under
`D:/merman-build-archive/lsp-bounded-index-reuse-2026-09-18/`, then the complete test suite passed in
release. No source or historical benchmark files were removed. Debug execution is not claimed as
passing for this candidate.

## Evidence and follow-up

Ignored ledger: `target/bench/experiments/lsp-bounded-index-reuse-2026-09-18/`. It contains the
preregistration/outcome, candidate probe/executable, sixteen process logs, summary and summarizer,
source digests, tracked patch, new module copy, correctness/lint logs, and failed-build record.
The base executable and fixture manifest remain in the adjacent expanded-attribution ledger.
The temporary server-test include was removed; `server/tests.rs` matches its original bytes.

Further work should use concrete editor workflows before increasing the allowance or adding
replacement policy. A range-directed scan may help budget-denied documents, but must preserve
complete intersecting captures and exact invalid-position errors. First-request/source-update
costs remain separate from unchanged-snapshot reuse. Keep token-result caching a separate question.

SHA-256:

| Artifact | Digest |
| --- | --- |
| `Cargo.lock` | `a910ef1851947cbfe6b1c8e3ed97bb7f35e35efbfe2b074427779be9bb9b581a` |
| `baseline executable` | `6eabf5318f393c7911df3f0f2a39ae5bdc7c1966bd3bee428f71a3e0f70ffbf9` |
| `candidate source manifest` | `ad50efe1ffacb881d3379ba4192f86239a1ebdd6f3e3c6388b1b9bce0dd0ee49` |
| `experiment.yaml` | `93cbe47fbe7d50caafefeca21abc31ce022d15772a73a6c9cb01a129ec50772c` |
| `candidate.exe` | `b1a7b18eeca8687711bdad3b9ea19882f124bb491c3fd4cad75f948c4e8e6604` |
| `handler_probe.rs` | `fc9e579668e6288a01c865965d7aacd04d6da420f2718059358d6993d9f07148` |
| `summary.json` | `ea7dd02c1acc7e788f4444eeab47cd8a3c3d71267bdbfe0e00e41371a6f71aa1` |
| `summarize.py` | `02644b24ec2b4eff65715d9e95e65fa36fb96a3e3136f0db6a36cd2c01920dbe` |
| `digests.json` | `840908b5331b586c01b02d0640757185274dc05cee0faa3cad3fd00a0cf36e2f` |
