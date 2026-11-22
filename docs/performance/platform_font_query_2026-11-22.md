# Platform font query checkpoint — 2026-11-22

The ordered-family refactor and macOS character fallback repair reduce unnecessary
discovery and snapshot work. This checkpoint does **not** establish an additional
end-to-end speedup or qualify a new CLI default. The baseline is PR #336 at
`2ce0fa4589f13c6c80c4e0be9d3292eedfbc4471`; the candidate contains the accompanying
platform-font changes. Both sides use Rust 1.100.0, locked dependencies, the
`cli-release` artifact recipe, and `dist`.

## Work and correctness

For `F` ordered generic/fallback family IDs, the old provider expanded all `F`
families before acceptance. The candidate expands only the prefix `K` through the
first accepted face. Worst-case expansion remains O(F); collecting IDs remains
O(F). There is no new persistent cache. Controlled catalog tests count family
calls and verify first acceptance, rejection order, cancellation, and admission
errors. Fontique still owns its initial platform catalog construction.

On macOS, Core Text receives the actual missing Unicode scalar and the registered
base face. The returned PostScript identity is matched to a physical collection
index; Macintosh Roman ASCII name records are needed for Lucida Grande. Existing
snapshots take precedence over changed files during both identity lookup and
admission. Fontique maps discovery sources; mapping a file is not an eager copy or
a measurement of pages read. The existing parser and coverage predicate reject
non-covering fallback sources before a new bounded snapshot is acquired.

Host tests verify that native measurement and painting use the same faces and
advances. The macOS symbol label `Latin ✓ ∑ ∫ é` changes from a resource error to
a successful PNG using Arial and Lucida Grande. U+0378 changes from a resource
error to an output with missing-glyph diagnostics. Real selected CJK collections
and color emoji files can still exceed the default 64 MiB snapshot budget.

## Linux file access

Ubuntu 22.04.5 ARM64 in a Lima VM on an Apple M4, Fontconfig 2.13.1, with 11 font
files: six DejaVu files, four Noto CJK collections, and Noto Color Emoji.
Separate `strace` runs observed `openat,read,pread64,mmap` including child threads.
Every row below retained identical PNG bytes between revisions.

| Platform input | Unique font files, base → candidate | Bytes returned by read/pread, base → candidate |
| --- | ---: | ---: |
| Named Latin | 2 → 2 | 757,076 → 757,076 |
| Generic sans-serif | 9 → 2 | 757,076 → 757,076 |
| Rare Han | 9 → 4 | 20,241,860 → 20,241,860 |
| Unassigned scalar | 9 → 9 | 32,047,852 → 757,076 |

The unassigned case still queries candidate metadata, but no longer acquires
non-covering binary snapshots. The generic case avoids loading unrelated families;
its unchanged read count must not be reported as a reduction in bytes read.

To reproduce the generic input, save:

```mermaid
---
config:
  fontFamily: "sans-serif"
  htmlLabels: false
---
flowchart LR
A["Generic serif and sans"] --> B["Destination"]
```

Run each frozen executable with:

```console
strace -qq -f -yy -o font.trace -e trace=openat,read,pread64,mmap \
  ./merman-cli render generic.mmd --format=png --output=generic.png \
  --scale=2 --svg-id=font-provider-probe --font-system=platform
```

For the other rows use family `DejaVu Sans` and label `Native font: AV iii WWW mmm`,
`Latin 𠮷野家`, or `Latin ` followed by U+0378. Keep the same host font inventory,
build recipe, input, and output options when comparing revisions.

## Timing and verification boundary

The fresh-process schedule used two warmups, a median of three sequential spawns
per paired side, independent eight-pair A/A calibration for both executables, and
eight fresh alternating A/B pairs. The registered improvement margin was jointly
10% and 1 ms, with 10,000 bootstrap resamples and seed 336.

Linux calibrated successfully. Generic PNG changed from 10.859 to 10.553 ms;
named small Flowchart PNG from 20.763 to 20.776 ms; native SVG from 6.124 to
6.107 ms. None clears the improvement margin. The macOS 27.0 / M4 rows failed
A/A qualification and remain inconclusive. These observations support the
structural-work claim, not a further public-latency claim.

The 19-case corpus on each host covered Latin, generic families, Sequence,
Arabic, Devanagari, CJK, rare Han, combining symbols, emoji, unassigned scalars,
explicit closed fonts, PNG/JPEG/PDF, and raw SVG conversion. All previously
successful outputs retained their bytes. macOS passed 300 export tests including
host tests, 60 explicit-only tests, 90 bounded tests, and 29 CLI font contracts.
Linux passed 294 export tests including host contracts. Windows test targets
cross-compiled; Windows runtime qualification remains with native CI.

Both executables were frozen from Cargo artifact messages after removing only
workspace outputs from the shared `dist` profile and rebuilding the workspace
owners. An earlier copy of an old cached baseline was rejected and excluded from
this checkpoint. Local raw samples and recipes are under
`target/bench/experiments/platform-font-query-2026-11-22/`.

Keep the current defaults. The remaining policy gates are selected-face snapshot
design for oversized macOS sources, and distinguishing absent Fontconfig from an
intentionally empty Linux configuration. See [CLI font policy](../FEATURES.md#optional-cli-fonts).
