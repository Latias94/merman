# Remaining LSP index work and reuse attribution (2026-09-18)

## Decision

The compact index still performs material whole-source work on small range requests. Proceed to a
separate, bounded snapshot-index reuse experiment for repeated requests against unchanged source.
Do not admit a production cache from this attribution: its synchronization, ownership, budget,
first-request cost, and cancellation behavior have not been implemented or measured here.

This diagnostic follows the [compact-index repair](lsp_compact_line_index_2026-09-17.md). It
expands the corpus to final ranges, multiple fences, and a long host-text line, and includes index
construction/destruction when simulating reuse across 1, 4, or 16 requests. Production source is
unchanged; only this receipt and the active performance queue are retained.

## Workload and measurement boundaries

- Base: `c461fba13` (`perf(lsp): compact semantic token line indexes`), initially clean.
- Windows x86-64 / Intel Core i7-11700 / Rust 1.95.0, `x86_64-pc-windows-msvc`; release,
  default LSP features, locked dependencies, two Cargo build jobs. No concurrent Cargo work during
  measurements. This is same-source attribution, not an old/new implementation comparison.
- Ten cases: ordinary 80-byte host lines at 64 KiB, 1 MiB, and 4 MiB with an early Mermaid fence;
  4 MiB dense LF with an early fence; 4 MiB ordinary/dense LF with a final fence; 4 MiB single
  long host-text line before or after one fence; and two ranges in the same 4 MiB document with
  128 evenly distributed fences. Source byte sizes are exact. Padding ends with LF so final fences
  begin on a new line. These fixtures are independently identified by the manifest digests rather
  than assumed byte-identical to the previous day's early-range fixtures.
- Each fence contains `flowchart TD` and `A["😀"] --> B`. Ranges cover its two body lines.
  All range results contain eight tokens; the many-fence full result contains 1,024. The long line
  is outside the Mermaid fence: this measures scanning host source, not a giant Mermaid label's
  UTF-16 conversion or text measurement.
- Three fresh processes; seven samples per lane, with two warmups. Stage batch sizes: index 3,
  captures/projection/prebuilt range 20, full/prebuilt full 3. Batch reuse has one batch per sample.
  The private probe runs before the handler probe, warming global query state. No cold-start claim.
- Reuse batches have `R = 1, 4, 16` requests. Current work constructs an index per call; simulated
  reuse constructs one per batch and frees it before stopping the clock. Both regenerate captures
  and projected tokens on every call. Current/reuse measurement order alternates across samples
  (four current-first, three reuse-first); all other case order remains fixed. Results are
  descriptive, without A/A-calibrated latency admission or tail/confidence-interval claims.
- Handler samples average ten unchanged-snapshot requests. Seven edits each change the selected
  node's first character between `A` and `B`, followed by one range request. Edit parameters are
  prepared outside timing; application, preparation, syntax update, and commit are timed.
  Paired edit-plus-range totals are formed before taking the median.
- Handler timing includes bounded CPU dispatch, snapshot checks, planning, and response creation.
  It excludes stdio, serialization, editor work, and final response-vector destruction. Pull
  diagnostics disable background analysis; exactly one request is in flight and Tokio has two
  workers. All 13 supported token types are negotiated in the same order as the private profile.
- Private stages include temporary-result destruction. A reference index is not retained during
  index, complete-plan, or batch timings. Prebuilt-only stages intentionally exclude index setup.
  Source loading, hashing, equality checks, and console output occur outside timed intervals.

## Remaining work

The following are medians of 21 samples. Index/prebuilt columns are private-stage microseconds;
handler columns are milliseconds. Stages run separately: do not add or divide these medians to
claim precise CPU percentages. Host and allocator variability can make an isolated stage exceed
its separately measured enclosing handler.

| Case | Index build/drop (us) | Prebuilt range plan (us) | Hot range handler (ms) | Edit + range (ms) | Reserved index bytes |
| --- | ---: | ---: | ---: | ---: | ---: |
| 64 KiB ordinary, early | 32.47 | 5.76 | 0.079 | 0.255 | 8,192 |
| 1 MiB ordinary, early | 530.87 | 5.75 | 0.595 | 4.463 | 131,072 |
| 4 MiB ordinary, early | 2336.97 | 6.05 | 2.356 | 17.307 | 524,288 |
| 4 MiB dense LF, early | 15658.00 | 6.08 | 15.682 | 84.841 | 33,554,432 |
| 4 MiB ordinary, final | 2105.07 | 5.49 | 2.272 | 15.382 | 524,288 |
| 4 MiB dense LF, final | 15381.20 | 5.89 | 16.268 | 85.747 | 33,554,432 |
| 4 MiB host long line, final | 1619.40 | 5.42 | 1.589 | 12.937 | 64 |
| 4 MiB host long line, early | 1713.07 | 5.53 | 1.608 | 13.187 | 64 |
| 4 MiB / 128 fences, first | 2341.60 | 6.12 | 2.224 | 15.343 | 524,288 |
| 4 MiB / 128 fences, last | 2344.37 | 5.50 | 2.212 | 15.387 | 524,288 |

For the 4 MiB long host-text line, the entire line table reserves only **64 bytes**, yet building it
still takes about **1.6–1.7 ms**. The constructor's byte loop reaches EOF even with only a few lines;
large allocation is not required for this cost. This isolates whole-source construction as a
remaining target, though it is not a profiler decomposition of individual machine instructions.

Moving the requested fence to the end does not remove the cost. Range captures and projection
remain a few microseconds across these cases, including 128 fences. Full planning of those 128
fences is different: the first/last-range cases measure complete full-plan medians of 3.210/3.246 ms
and prebuilt full-plan medians of 0.809/0.748 ms. Index reuse would leave that full-document capture
and projection work. Full figures are private-stage observations, not full/delta handler timings.

## Reuse amortization, with preparation included

Entries are medians of sample-paired `simulated reuse / current` **batch-total** ratios. They are
neither predicted public handler ratios nor a deployed-cache speedup. `R=1` models a fresh index
used once; `R>1` models successive requests against exactly the same immutable source.

| Case | R = 1 | R = 4 | R = 16 |
| --- | ---: | ---: | ---: |
| 64 KiB ordinary, early | 0.992 | 0.372 | 0.201 |
| 1 MiB ordinary, early | 1.002 | 0.252 | 0.073 |
| 4 MiB ordinary, early | 0.987 | 0.248 | 0.061 |
| 4 MiB dense LF, early | 0.981 | 0.246 | 0.062 |
| 4 MiB ordinary, final | 0.949 | 0.246 | 0.067 |
| 4 MiB dense LF, final | 1.006 | 0.244 | 0.061 |
| 4 MiB host long line, final | 1.007 | 0.280 | 0.065 |
| 4 MiB host long line, early | 0.977 | 0.264 | 0.065 |
| 4 MiB / 128 fences, first | 1.008 | 0.239 | 0.064 |
| 4 MiB / 128 fences, last | 0.996 | 0.238 | 0.065 |

For the ordinary 4 MiB early case, median current/reuse totals are 8.590/2.157 ms for four requests
and 36.421/2.333 ms for sixteen. For dense LF they are 64.560/16.147 ms and 251.674/15.627 ms.
The single-request ratios cluster near one: they provide no stable amortization benefit. The
64 KiB control also has smaller absolute cost and a larger residual per-request contribution.

With `B` source bytes and `R` requests, current construction scans `R * B` bytes. Reuse would scan
`B` bytes once per prepared snapshot, while capture/projection still executes `R` times. This is a
source-level work argument; no byte counter or production cache was added. The saved work depends
on actual request multiplicity, which this synthetic experiment controls rather than measures in
real editors. One request per edit continues to construct once per distinct source snapshot.

The handler edit totals also remain substantial. Reuse of an unchanged snapshot does not eliminate
source preparation or syntax updates, and this experiment does not attribute those edit costs.

## Ownership and budget constraints for the next candidate

Source inspection at the measured revision confirms:

- Full/range requests always replan. Delta also computes the new full plan before comparing it
  with the prior packed stream (`server.rs`, `semantic_tokens.rs`). An empty delta is not a
  same-snapshot computation bypass.
- The old token state is deliberately retained across edits as a delta baseline. Its result ID
  alone does not prove current snapshot identity (`session/documents.rs`). Token-result caching
  would be a different candidate and must not be bundled into index reuse.
- Syntax snapshots hold `Arc<SyntaxDocumentState>` and are validated by document epoch,
  cancellation state, and Arc identity. Editing creates a new syntax snapshot, while in-flight
  requests may continue to hold the old one.
- The default 64 MiB analysis cache stores analysis snapshots/contexts, not syntax-index payloads
  (`session/analysis_cache.rs`). Worker permits do not by themselves bound all retained snapshots:
  requests acquire snapshot references before entering the worker budget.

For the next experiment, freeze an explicit retained-byte budget and lifecycle policy before
implementation. Store only a completed index tied to its immutable source; do not initialize
while holding the session lock. Concurrent requests must not independently retain duplicate
indexes, and one request's cancellation must not poison shared initialization. New snapshots must
not inherit an old source's index. Budget exhaustion must preserve behavior through request-local
construction. Account for retained old snapshots and multiple documents, not only current entries
or worker count. Measure first use, repeated requests, one request per edit, concurrent callers,
close/reopen, stale results, cancellation, and budget exhaustion.

An ordinary 4 MiB index reserves 0.5 MiB, but dense LF reserves **32 MiB per snapshot**. The prior
pure-LF boundary reserves 64 MiB. These figures make unconditional snapshot retention unsuitable
without a separate policy. They measure vector capacity, not RSS, cumulative allocation, or
reallocation peaks. No numeric cache budget is selected or added by this diagnostic.

Range-directed preparation is an alternative experiment if avoiding retained state takes priority,
but it cannot simply stop at the requested end line. Captures are selected by intersection and
retain their full boundaries, which can extend past the request. Out-of-range line errors also
report the exact document line count; UTF-16 errors report the complete line length. These error
and projection semantics must survive any partial scan. Late ranges still need their absolute line
positions. No timing benefit for this alternative is claimed by the current probe.

## Validation and provenance

`cargo nextest run --locked --release -p merman-lsp --lib --test-threads 4` passed **234 tests**;
the two manual probes were skipped, then both passed in each of three processes. All source
SHA-256 values, line counts, and range packed outputs match across private and handler runs.
Full prebuilt/production packed outputs match in every case; edits preserve token data and advance
versions, and no diagnostic analyses ran. Independent review checked probe timing boundaries and
the source-level ownership constraints.

Temporary test-only includes were removed by matching their exact added suffixes. Both Rust files
are byte-identical to the initial revision. Existing integration/render suites and Clippy were not
rerun for the final documentation-only change; `git diff --check` passes.

Ignored evidence: `target/bench/experiments/lsp-index-reuse-attribution-2026-09-18/` contains the
ledger, source manifest and exact fixture files, shared fixture loader, two probe sources, executable,
three raw logs, summarizer, and digests. The probes build using temporary includes in the private
semantic-token and server-test modules with release library tests. Execute the saved binary in
three fresh processes using `probe.exe --ignored --test-threads 1 --nocapture`, without concurrent
Cargo work. These local artifacts are not permanent benchmark or CI infrastructure.

SHA-256:

| Artifact | Digest |
| --- | --- |
| `Cargo.lock` | `a910ef1851947cbfe6b1c8e3ed97bb7f35e35efbfe2b074427779be9bb9b581a` |
| `experiment.yaml` | `91ee78e66068a69a745e14f4b1d4e48203907c0ebe2839ad32328a9514364913` |
| `manifest.json` | `3fba246203fdcea322b03058d82f5dd6d628b1bdc072be0d99932e01c28dfdac` |
| `probe.rs` | `410b52c8defb2405a280d73e78b742d1beff1b8ea0b1b45cad1c88dc5a1455ca` |
| `handler_probe.rs` | `01d8a6b626d6a21519755d2f34311e8308fe67277777003041e77be720f17229` |
| `fixtures.rs` | `23806ab7f00f805897bd7df7abb9c87d782e010a74a5a3f703cb84a2c2ab3469` |
| `summary.json` | `37d27ac861060ac4ecc68891838316d46ed98633cef1b70435d66f2d6d46907e` |
| `probe.exe` | `6eabf5318f393c7911df3f0f2a39ae5bdc7c1966bd3bee428f71a3e0f70ffbf9` |
| `run-1.log` | `4f13d91cad493f6fb0d45b2daf43f0ebbf23183af932fdcb77b5ab70f3a67e75` |
| `run-2.log` | `7f114e5a94423b138e44c4db5f00f8fdebad143540125ef50980e3c04a3e66d1` |
| `run-3.log` | `f3d6bcc981852de8645767b1206dfe56b2421bdcc692aba785132a0d19f38848` |
| `summarize.py` | `b6f0504efef45b8505d2e58c1acbd75d40475059533619b44028aba9592276ce` |

Exact source digests are recorded in the manifest; `many_first` and `many_last` intentionally share
the same source bytes and differ only in the selected range.
