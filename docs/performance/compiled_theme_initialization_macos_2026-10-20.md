# Prepared compiled-theme initialization — macOS, October 20, 2026

The candidate removes repeated initialization from warmed installation of a compiled theme with
exactly absent host overrides. Kanban / Editor Light drops from 3661 to 966 allocations and
from 755,278 to 153,183 allocated bytes per public render. All 380 themed catalog cells improve;
the 38 None cells retain all four operation-growth metrics. This is allocation/work removal,
with a bounded persistent-memory tradeoff, not a latency or v0.9.0 convergence claim.

## Ownership change

Each installation previously rebuilt its ordered site JSON, initialization inputs and resolved
effective configuration. The existing internal parse plan now retains one lazily prepared
initialization. The same initialization function serves preparation and ordinary rebuilding.
Any host configuration with retained authority or mutation history uses the original merge path.
Registries, runtime policy and the rest of the Engine remain intact.

Preparation keeps the effective configuration as a Result, consumed at the original operation
stage. Per-installation journals preserve initialization-input then site ordering, one detector
clock read per original assignment, revisions and later framework writes. Tracking state uses
its existing copy-on-write isolation and is frozen independently for each parse. The plan stores
no parsed source, selected family, request counter or completed parse configuration.

This targets warmed reuse of typed compiled themes. The first installation still prepares the
snapshot, and compilation is outside the measured render. No cold-start, compilation, first-install
or None public19 benefit is claimed. This does not explain the latest no-theme release gaps.

## Public operation evidence

Baseline: `d9e91a28ce0ae92fcc31117118e2ece8d2958ec1`. Both sides use the original release/all-public-feature
library recipe, deterministic SVG environments, one warmup and five measured operations with rich
results retained at the checkpoint. Separate ordered-callback probes use custom measurers. All native
command stages run serially; nextest uses four test threads.

All 645 saved SVG/report identities and independent ordered measurement traces match; all
26 public error controls match. Of 503 jointly stable controls, 208 improve and 295 retain all
four metrics, with zero increases. All 208 improvements have a compiled theme: 65 save 1658
allocations and 380,393 B; 143 host-fallback controls save three allocations and 168 B by
sharing raw Tracking ownership. The 295 stable None controls are unchanged. The 142 variable controls remain inconclusive, without retry
or resampling. The base has 506 stable controls and the candidate 503; three base-only stable
controls are retained as variable: `family-flowchart_backedges_subgraphs`,
`family-flowchart_direction_conflict` and `treeview-fixture-02`.

All 418 catalog cells across 35 logical families and 38 fixture aliases, ten presets plus None,
are stable and exact within the original contract: warmup result metadata plus all five saved
sample SVG/report bytes and metadata. Warmup SVG bytes are not captured. All 380 themed cells
improve; 38 None cells retain every growth metric. Retained growth is unchanged throughout the
catalog. Within each preset, allocation and allocated-byte savings are identical in all 38 scenes:

| Presets | Cells | Fewer allocations per render | Fewer allocated bytes |
| --- | ---: | ---: | ---: |
| Editor Light, Ayu Light, Gruvbox Light | 114 | 2695 | 602,095 |
| Editor Dark, One Dark, Ayu Dark, Gruvbox Dark | 152 | 2720 | 602,463 |
| Brutalist, Spotless, Cyberpunk | 114 | 1658 | 380,393 |

The preselected Kanban / Editor Light primary is:

| Operation-growth metric | Base | Candidate | Reduction |
| --- | ---: | ---: | ---: |
| Allocations | 3661 | 966 | 73.61% |
| Allocated bytes | 755,278 | 153,183 | 79.72% |
| Peak growth bytes | 311,625 | 62,464 | 79.96% |
| Retained growth bytes | 31,225 | 31,225 | unchanged |

## Persistent memory tradeoff

The operation metrics start after warmup. Prepared values persist with the compiled plan and
therefore increase pre-request process-live memory. A separate amendment fixed the observation
and space contract before applying or measuring the candidate: at most one initialization per
compiled plan, O(D + C + V) retained space for pinned defaults, normalized bounded compatibility
input, resolved effective variables and path metadata. There is no source-, family- or
request-keyed accumulation. This bounded one-time retention is permitted for reuse; it is not
a total-memory improvement.

The original absolute process-live counters show the following candidate-minus-base ranges.
Every preset's start/end delta is identical in all 38 scenes and all five samples; every cell's
three absolute deltas are constant across its five samples.

| Presets | Pre-request and retained-result process-live increase | Absolute process peak change |
| --- | ---: | ---: |
| Editor Light, Ayu Light, Gruvbox Light | +284,624 B | -12,564 to +248,920 B |
| Editor Dark, One Dark, Ayu Dark, Gruvbox Dark | +284,560 B | -12,661 to +248,579 B |
| Brutalist | +175,285 B | -12,727 to +175,285 B |
| Spotless | +175,285 B | -11,593 to +175,285 B |
| Cyberpunk | +175,285 B | -10,690 to +175,285 B |
| None | +12 B | +12 B |

For the primary, process live before the render is 2,147,927 to 2,432,551 B; after retaining the
result it is 2,179,152 to 2,463,776 B. Absolute process peak rises from 2,459,552 to 2,495,015 B,
a 35,463 B increase despite the smaller operation peak growth. These are process counters,
not an isolated cache-size measurement.

Independent review confirms that the constant None +12 B follows the retained probe argument
strings: both executable and output-directory paths are six bytes longer under `candidate1/`
than `base/`. Raw counters remain unadjusted. The additional typed state is the plan's prepared
JSON and metadata owners, independent of the rendered scene and request count.

## Validation and execution record

Three new tests compare the legacy and compiled installation paths: host/reset/font ownership
and ordered JSON; deferred invalid-color errors with preprocessing and cancellation; and
unfinished nested detector clones escaping after inner completion. The last case covers both
an unprepared plan and one warmed before the outer checkpoint.

The original test draft incorrectly expected ordinary malformed directive JSON to be fatal.
The original implementation correctly ignores it and then reports the deferred bad-color error.
The failure and draft are retained; only that new assertion was corrected. All three tests then
pass on the original implementation before applying the candidate. No production semantic repair
was made for that test error.

All 13 registered command stages exit successfully, and the runner terminates with exit code zero.
All 930 focused tests and 7651 full release tests pass (seven full tests skipped) using the same
`merman-core`, `merman` and `merman-render` all-feature graph. Strict release Clippy, all 37 DOM
entries in three modes, the optimized full WASM build including `wasm-opt`, and formatting, diff
and path checks pass. Browser-text/parser diagnostic residuals retain their original scopes.
No new WASM runtime smoke or latency experiment is claimed.

Independent source, raw-evidence, report and terminal reviews find no blocker. The original raw
reviews and rejected test draft remain unchanged.

## Reproducibility

The ignored experiment is `target/bench/experiments/compiled-theme-initialization-20261020/`.
Registration SHA-256: `4964ba930cd647a51b65eb485dd4abfc1af93248938f5c7d92a9bbf7a74dd617`.
Persistent-space amendment: `0815384a11385f56d508af4e2d03dad91438f6a8f4c64e790f61419498f6b2d0`.
Actual source freeze: `1af0cf1882b39e2ac2146e5b545081f867bc5bcc29eb3092f592317eb693d25e`.
Source review: `b7cb9446f968b90fe6b85adb8ef196e5c513cc609cf85d38a3f0ae1fc28c6187`.
Control comparison: `69bfac60c6267df5ab63a51c33a07a67a9d38b81846f1a6ae66b9ff96180aa25`.
Raw-evidence review: `2c8325fe6f3bad3859ce74f7ba45e26aa81c6eb86bc93f5b97d652e47126c532`.
Catalog comparison: `6a2ffb6b39d990d2c42cb76bf00516a5ff226b43d512d018d7c18c9a2786085b`.
Absolute-live comparison: `ff8a8ff7ec942f7cc44d68ce470fb8c1ac6e5f6dc90bd4ef423af65696e02cb4`.
Terminal review: `0e263f88b9232c78aa777e7662d92bbfca792081addc3c5847ab533e95ff377c`.
DOM manifest: `5536786c82f277f219ad1c1f0dc451789bafeaf81dcdb5bb6cca608e1db38468`.
Earlier drafts, failures, frozen probes, raw five-sample counters and SVG/report artifacts remain
unchanged.

The latest full19 release scoreboard remains at its earlier frozen head: 12 confirmed
non-regressions, three regressions and four inconclusive scenes. This candidate does not update
that scoreboard. Next run a fresh full19 confirmation on the newly admitted clean head, including
the preceding literal XHTML paragraph change. The all-family optimization goal remains active.
