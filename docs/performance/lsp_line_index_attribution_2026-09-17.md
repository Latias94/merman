# LSP short-range line-index attribution (2026-09-17)

## Decision

Prioritize reducing the temporary semantic-token line index before adding snapshot-level reuse.
Whole-source index construction is a material cost even when the requested range covers only two
lines. Dense newlines also amplify scratch capacity: a supported 4 MiB Markdown fixture reserves
96 MiB for this index, and the separate 4 MiB pure-LF boundary reserves 192 MiB.

This is baseline attribution, not an admitted implementation or a measured speedup. Production
Rust source is unchanged. No cache, public API, dependency, or CI benchmark was added.

## Frozen workload and validation

- Base: `0a7bcf4ada1cd2a50f89266176f665354ef1dfc3`; initially clean working tree.
- Host: Windows x86-64, Intel Core i7-11700; Rust 1.95.0, `x86_64-pc-windows-msvc`.
- Build: release, `merman-lsp` default features, locked dependencies, two Cargo build jobs.
- Sources: exactly 64 KiB, 1 MiB, or 4 MiB of UTF-8. Each begins with one Mermaid fence
  containing `flowchart TD` and `A["😀"] --> B`. Ordinary padding is 79 ASCII `x` characters
  followed by LF; dense padding consists only of LF. The final padding is truncated to size.
- Requested LSP range: `(1, 0)..(3, 0)`. Every source produces the same eight tokens with all 13
  supported token types negotiated in the same order. Source SHA-256 and packed output equality
  were checked mechanically between private and handler probes across all three processes.
- Handler calls include the existing CPU budget, blocking dispatch, current-snapshot checks,
  planning, and response construction. They exclude transport serialization, stdio, editor work,
  parameter construction, and final destruction of returned tokens. Pull-diagnostic capability
  disables push analysis; diagnostic execution count remains zero.
- Each process runs private attribution first, then handlers, with sizes ascending and ordinary
  before dense. Each lane has two warmups and nine samples. Three fresh processes provide 27
  descriptive samples per lane. Hot handler samples average ten requests each.
- Private index/plan samples use 100/20/5 iterations for ordinary sources and 10/3/1 for dense
  sources. Capture, projection, and prebuilt-index samples use 100 iterations. Scratch destruction
  is inside private timing; no reference index is retained during index or whole-plan timing.
- Edit samples alternate the first identifier between `A` and `B`, with one real incremental
  edit followed by one range request. Document version and unchanged token output are asserted.
  Edit timing includes source preparation and syntax update. Paired edit-plus-request totals are
  computed before taking the median.
- `cargo nextest run --locked --release -p merman-lsp --lib --test-threads 4`: **231 passed**,
  two manual probes skipped. Both probes then passed in each of the three measurement processes.
  Existing coverage includes UTF-16, multiline captures, range errors, syntax updates, and worker
  lifecycle behavior. Integration/render suites were not rerun for this documentation-only result.

Temporary test-only includes accessed private code without changing visibility. Those exact
additions were removed after measurement; both Rust files match their original bytes. The ignored
probe source and executable remain with the raw ledger.

## Public handler observations

Times below are milliseconds. Hot-request spread is the minimum and maximum of the 27 batch
means, not a confidence interval or a tail-latency estimate.

| Source | Shape | Hot range median | Hot batch min–max | Edit median | Post-edit range median | Paired edit + range median |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| 64 KiB | Ordinary | 0.084 | 0.048–0.103 | 0.193 | 0.072 | 0.298 |
| 64 KiB | Dense LF | 0.842 | 0.691–1.105 | 1.303 | 0.774 | 2.082 |
| 1 MiB | Ordinary | 0.562 | 0.485–0.671 | 3.575 | 0.742 | 4.341 |
| 1 MiB | Dense LF | 12.301 | 11.256–17.516 | 20.114 | 13.598 | 33.311 |
| 4 MiB | Ordinary | 2.736 | 2.412–3.438 | 15.545 | 2.834 | 18.453 |
| 4 MiB | Dense LF | 54.588 | 46.801–70.288 | 82.692 | 49.689 | 130.371 |

The first request after opening each snapshot is retained in raw results. Opening already uses the
blocking executor, and the earlier private probe warms the global highlight query. These values
are neither process cold-start measurements nor isolated query-initialization measurements.

One request per edit still pays the index construction cost, but a cache initialized once per
snapshot would also pay it once. It cannot amortize that construction in this scenario. The edit
path itself is substantial and must remain part of any future interactive-workflow comparison.

## Private attribution

Independent stage medians in microseconds; do not add them or divide them by handler timings to
claim precise CPU percentages. Sequential measurement, allocation context, and host variability
can make an isolated stage slower than a separately measured enclosing operation.

| Source | Shape | Index build + drop | Complete token plan | Captures | Projection | Plan with prebuilt index |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| 64 KiB | Ordinary | 30.06 | 37.81 | 4.57 | 0.69 | 5.66 |
| 64 KiB | Dense LF | 796.01 | 845.56 | 5.17 | 0.83 | 6.91 |
| 1 MiB | Ordinary | 674.83 | 636.50 | 5.27 | 0.79 | 6.21 |
| 1 MiB | Dense LF | 12081.40 | 12024.23 | 5.00 | 0.95 | 6.11 |
| 4 MiB | Ordinary | 2419.28 | 2443.18 | 4.92 | 0.80 | 6.62 |
| 4 MiB | Dense LF | 48493.50 | 54499.00 | 5.67 | 1.12 | 6.33 |

`semantic_tokens::token_plan` constructs `SourceIndex` before translating the requested range and
querying the stored syntax tree. Construction scans all source bytes and pushes one `SourceLine`
per line, including the final empty line after a terminator. With source bytes `B` and line count
`L`, this costs `O(B)` time and `O(L)` scratch space on every supported request, even for this fixed
range near the beginning of the document.

The prebuilt column bypasses index construction/destruction and preserves packed output. It is a
private ceiling on removable preparation work, not a cache implementation or a predicted handler
latency. It excludes cache lifetime, synchronization, invalidation, and retained-memory costs.

## Line-table memory

Measurements are actual `Vec::len()` and `Vec::capacity()` multiplied by the 24-byte `SourceLine`
size on this target. They exclude allocator metadata, reallocation peaks, all other allocations,
and RSS. They are not cumulative allocated bytes or whole-process peak memory.

| Source | Shape | Lines | Used bytes | Reserved bytes |
| --- | --- | ---: | ---: | ---: |
| 64 KiB | Ordinary | 823 | 19,752 | 24,576 |
| 64 KiB | Dense LF | 65,497 | 1,571,928 | 1,572,864 |
| 1 MiB | Ordinary | 13,111 | 314,664 | 393,216 |
| 1 MiB | Dense LF | 1,048,537 | 25,164,888 | 25,165,824 |
| 4 MiB | Ordinary | 52,433 | 1,258,392 | 1,572,864 |
| 4 MiB | Dense LF | 4,194,265 | 100,662,360 | 100,663,296 |

The separate pure-LF probes have `B + 1` lines. At 64 KiB, 1 MiB, and 4 MiB, measured reserved
capacity is respectively **3 MiB, 48 MiB, and 192 MiB**. At 4 MiB this is 4,194,305 used entries
and capacity for 8,388,608 entries: the trailing empty line crosses a vector-growth boundary.
This is a memory-only probe of the same index constructor, not a pure-LF handler latency result.
The default LSP source limit admits 4 MiB; deployments can configure different limits.

Persisting this existing representation would turn request-local scratch into snapshot-retained
memory. Multiple documents and in-flight older snapshots would add to it. Existing CPU/task
permits bound active work, but do not make such retained state free or automatically charge it to
the analysis-result cache budget.

## Next candidate and exit conditions

First evaluate a compact **request-local** index of line-start offsets, deriving `content_end` and
`end` from the next offset and source terminator. One `usize` per line replaces three, without
introducing snapshot lifetime or cache invalidation. This is a candidate, not an implemented or
measured saving. Keep `usize` unless all configurable input sizes can be proven safe for a narrower
type; a default 4 MiB limit alone is insufficient.

Register memory/resource admission before implementation. Preserve empty input, LF, CRLF, isolated
CR, trailing empty lines, EOF positions, UTF-16 surrogate rejection, invalid-range errors,
multiline token splitting, cancellation, and full/delta/range equivalence. Compare exact used and
reserved capacities, including the pure-LF growth boundary, and rerun adjacent handler controls.
No percentage latency improvement should be admitted from the diagnostic samples above.

Only after the compact representation is measured should snapshot reuse be reconsidered. It needs
an explicit retained-memory policy, complete snapshot ownership, concurrent old-snapshot behavior,
and evidence from repeated requests versus one request per edit. Avoid reusing edit-coordinate
helpers blindly: their position clamping and line conventions may differ from semantic-token
range errors. No broad benchmark framework or CI timing gate is needed for this next experiment.

These synthetic inputs isolate one short range near the first fence. They do not characterize
multi-fence documents, ranges near EOF, long individual lines, full/delta workloads, other hosts,
or Mermaid rendering performance. The visible timing spread warrants fresh controlled A/B work
before any latency claim.

## Reproduction and provenance

Ignored ledger: `target/bench/experiments/lsp-line-index-attribution-2026-09-17/`.
It contains `experiment.yaml`, both probe sources, raw logs, summary code/JSON, and `probe.exe`.
The receipt preserves the decision; ignored local artifacts are not distributed benchmark tools.

Build with temporary test-only includes in `semantic_tokens.rs` and `server/tests.rs`:

```powershell
$env:CARGO_BUILD_JOBS='2'
cargo test --locked --release -p merman-lsp --lib --no-run
cargo nextest run --locked --release -p merman-lsp --lib --test-threads 4
# Run the preserved executable in three separate processes, without concurrent Cargo work.
& target/bench/experiments/lsp-line-index-attribution-2026-09-17/probe.exe --ignored --test-threads 1 --nocapture
```

SHA-256:

| Artifact | Digest |
| --- | --- |
| `Cargo.lock` | `a910ef1851947cbfe6b1c8e3ed97bb7f35e35efbfe2b074427779be9bb9b581a` |
| `probe.rs` | `db9d0faec84a0a897406ccb4810633511d2ff68fac84906129c2fd8fa6fdfec0` |
| `handler_probe.rs` | `2592867e7b2b187241c233d0f31a1bb1695942b4c8dad75f9610213a64f135a0` |
| `experiment.yaml` | `5e8f2486c998c0a2fefb01fc214b323e61731bc92278a1af9d6881b945d263e4` |
| `run-1.log` | `2826ffdb80bc9dc6ec0726c6a5ee2fd8d226568f3a8e326a92a7e131fc785c25` |
| `run-2.log` | `c06a67830299948030dc8bc8da889b457d9ab0a847e89a92f0443972a8086034` |
| `run-3.log` | `bd389bed4689a4ccd175183490a4d59add99279704466dc49b0f0366678c9bf2` |
| `summary.json` | `ad05ae1ccc58fb01142c2a6616024253a565e3e315312dda6ff6a3a65add34b1` |
| `probe.exe` | `c789a811c5cd154e59c7d25572bc118527f4f656dddc750db91f99e8be02c038` |

Source SHA-256 digests (the output token sequence is identical across all six sources):

| Size | Shape | Digest |
| --- | --- | --- |
| 65536 bytes | Ordinary | `941740e52bbfd752f9f57d88c80de8a11282c4581fe4f5006428e3e79f45f2c6` |
| 65536 bytes | Dense LF | `a4a1a5ed145c1addc34ab44589e457293022ce03197c1afb385e0fdc7279a874` |
| 1048576 bytes | Ordinary | `944b8d3646d13def38335b3ed7be5759dab6c7f8b7b5b4664aa72039bdbcf390` |
| 1048576 bytes | Dense LF | `ca06a4482f3f399fc318dbee4199e14c3355d0a9ef6d1c4e6a0da3675e62de2d` |
| 4194304 bytes | Ordinary | `a9ae7a57d6bfdb0481086fbcc370c44f3ced1d224f16b745118d2c70fa6f47ef` |
| 4194304 bytes | Dense LF | `f6357508bc44de4a821245a00b36bba3874770021957b98d1af6884d22de17aa` |
