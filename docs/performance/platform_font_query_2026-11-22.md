# Platform font query checkpoint — 2026-11-22

This checkpoint records family lookups, font-file access and output for the
ordered-family refactor in PR #337. Its baseline was PR #336 at
`2ce0fa4589f13c6c80c4e0be9d3292eedfbc4471`. The macOS candidate was built from
that revision with local changes. Its recorded production inputs and lockfile
match #337's final `62a876a14868187338cde0d0622c2bfa3138afbc`. Both sides used
Rust 1.100.0, locked dependencies, the `cli-release` artifact recipe and `dist`.

## Work and correctness

For an ordered list of `F` candidate families, the old provider called Fontique
to expand all `F` before accepting a face. The candidate expands only the first
`K` families through the accepted face, or all `F` when none qualifies. Controlled
catalog tests count these family calls and check rejection order, cancellation
and admission errors. Fontique still builds the initial platform catalog.

Collecting one generic-family list takes time linear in its length. Character
fallback first merges script and generic lists. Both measured revisions use
`Vec::contains` to preserve the first occurrence of each ID. Visiting `M` IDs
with `F` distinct families takes O(M × F) comparisons in the worst case and
retains O(F) IDs. With `M = F`, the scan makes `F(F−1)/2` comparisons before
the first family is expanded.

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
  ./merman-cli render - --format=png --output=- --scale=2 \
  --svg-id=font-provider-probe --font-system=platform <generic.mmd >generic.png
```

For the other rows use family `DejaVu Sans` and label `Native font: AV iii WWW mmm`,
`Latin 𠮷野家`, or `Latin ` followed by U+0378. Keep the same host font inventory,
build recipe, input, and output options when comparing revisions.

## Timing and verification boundary

On each host, the CLI probe calibrated three inputs individually. Each side ran
two warmups and eight A/A pairs per input. Each side of a pair is the median of
three fresh processes. The probe then ran eight alternating A/B pairs per input
on each host. The registered improvement margins were 10% and 1 ms, with 10,000
bootstrap resamples and seed 336.

On Linux, both executables passed A/A for each input and each input required
eight pairs. Generic PNG changed from 10.859 to 10.553 ms, the named small
Flowchart PNG from 20.763 to 20.776 ms, and native SVG from 6.124 to 6.107 ms.
All three fell short of the joint improvement margin. On macOS 27.0 / M4, all
three inputs failed A/A. Their eight A/B pairs per input are diagnostic. The
family-call counts and Linux file traces establish where discovery work fell.

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

This checkpoint retained the provider defaults. Oversized macOS collections and the distinction between an
unavailable Fontconfig runtime and an intentionally empty configuration were
still open. The subsequent checkpoint records both repairs. [Native font runtime checkpoint](native_font_runtime_2026-11-22.md) · [CLI font policy](../FEATURES.md#optional-cli-fonts)
