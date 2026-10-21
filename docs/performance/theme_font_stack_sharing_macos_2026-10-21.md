# Immutable theme font-stack ownership - October 21, 2026

## Decision and scope

The registered warm allocation gate passes on all 418 catalog cells: 356 improve, 62 remain
unchanged, and none regresses on allocation count, cumulative bytes, peak growth, or retained
growth. All 350 improved non-None cells reduce both allocation count and cumulative bytes.
This is ownership/work evidence, not a latency or v0.9.0 convergence result. Source commit
`0a2d250dd547a14c418390f2a657a6c0727329fe` changes three renderer files.

## Cause and repair

`FontStack` exposed immutable validated families but owned a `Vec<String>`. Resolving a typed
style cloned that storage for separate base/computed typography values. Each clone copied the
family vector and every string, although the font list itself never changes. Base and computed
styles remain distinct because `Clear` must restore the receiving base.

Store the compiled font list in `Arc<[String]>` and clone only its immutable owner. Public family
access, validation order, errors, CSS, canonical/wire identity, value equality and Debug remain
unchanged. Source-owned CSS parsing retains a transient `Vec<String>` and uses the same extracted
validator; it does not construct a throwaway compiled Arc. There is no global cache or retained
theme/program owner.

For K clones, F families and B family-string bytes, the old copy work is O(K(F+B)); the new clone
work is O(K) reference-count operations without clone allocations. Construction still validates
and freezes each independently constructed list. Existing limits bound F to 32 and each family
to 256 bytes; this is not a claim of removing an unbounded input amplification.

## Warm public render evidence

The base runtime is `cf26d54bbcbbc7436858270f659404f52af02378`; the candidate's two intervening
commits change documentation only. Both sides use newly captured all-feature release libraries.
The operation is the existing complete public SVG render with its rich result alive at the
counter snapshot. Theme compilation and warmup precede counters; output projection, I/O and
result destruction follow the snapshot. It is not a full-destruction or process-cold measurement.

- Catalog: 35 logical families / 38 scenes, ten compiled presets plus None, five samples per cell.
  All 418 cells are stable. The 380 themed cells comprise 350 strict improvements and 30 unchanged
  cells; Error, Usecase and Wardley are the unchanged themed scenes.
- Site controls: 645 rows, with 504 jointly stable; 259 improve and 245 remain unchanged on all
  four metrics. All 141 variable rows remain inconclusive without resampling.
- Every saved SVG/report identity, all 645 ordered measurement traces and all 26 complete public
  error controls match. Catalog warmup records metadata, rather than physical warmup SVG bytes.

| Scene / preset | Allocation count | Cumulative bytes |
| --- | ---: | ---: |
| Flowchart medium / brutalist | 27,716 to 26,436 | 3,397,737 to 3,363,545 |
| Packet medium / editor-light | 284 to 214 | 29,655 to 27,793 |
| Kanban medium / editor-light | 881 to 791 | 137,515 to 135,121 |

Six None scenes reduce cumulative/peak bytes by 16-192 bytes with unchanged counts and retained
bytes. Their embedded optional style carriers become smaller in this frozen build; the None
branches do not resolve typed fallback fonts. This is not evidence of removing None font clones,
a stable Rust layout guarantee, or an explanation of the release latency gap.

## Bounded cold tradeoff

Fresh construction/compilation controls contain ten groups and five stable samples per side.
Inputs/specification preparation are outside counters; "cold" does not mean a fresh process.

| Operation | Count delta | Allocated-byte delta | Peak-growth delta | Retained delta |
| --- | ---: | ---: | ---: | ---: |
| Default construction | 0 | +16 | +16 | +16 |
| One-family construction | +1 | +40 | +40 | -56 |
| Maximum 32 x 256-byte construction | +1 | +784 | +528 | +16 |
| Representative custom theme compilation | 0 | +8 | -8 | -8 |

Holding 64 clones of the default, one-family and maximum stacks changes allocation counts from
320, 128 and 2,112 respectively to zero, and allocated bytes from 8,320, 2,176 and 573,440 to zero.
The subsequent drop groups release the corresponding source and clone storage. Their signed
live-byte differences reflect different previously owned storage, not a leak or a peak saving.
Arc headers and Vec-to-slice transfer remain explicit construction costs.

## Provenance and validation

The first attempted head reused baseline libraries from the shared target despite Cargo success.
Its byte-identical library hashes and unchanged cold clone counts exposed the failure. Preserve
that complete attempt separately and exclude its observations and 338 focused test results.
Only file mtimes were refreshed before an identical-source rebuild; the valid renderer and
facade receipts both report `fresh=false` with hashes different from the baseline. All candidate
controls then ran once in a new directory with the original thresholds. A byte-identical copy
adapts the old helper's baseline error-file location; its comparison logic is unchanged.

All 8,108 affected release tests pass (seven skipped), with strict production Clippy, 37 fresh
three-mode DOM reports, optimized full WASM, 34 internal theme acceptance tests, 459 compiled
pipeline benches/receipts, native-memory protocol smoke and its consumer contract, and
format/whitespace/path checks. The first native-memory owner invocation is rejected because
adding this report changed its Git status fingerprint during the build. Removing exactly that
new untracked-document line reconstructs the recorded initial fingerprint; the runtime hashes
are unchanged. With the workspace fixed, the existing protocol-only `--smoke --executable`
path passes on the same freshly built binary, frozen read-only. Both invocations and the
explicit executable reuse remain recorded; this is not comparative memory admission.

The [compact evidence archive](evidence/theme-font-stack-sharing-2026-10-21/manifest.json) records
the exact source/tool/library identities, original registration and amendments, original samples,
comparisons, independent reviews, and final owner checks. Physical SVGs, executables and complete
local experiment files remain under the ignored experiment directory; archive coverage is stated
explicitly rather than implying a standalone replay of omitted binaries.

The latest formal release classification remains 14 within budget, two confirmed regressions
and three inconclusive scenes. Next independently measure report transport, then refresh the
public v0.9.0 comparison; do not add independent diagnostic gains.
