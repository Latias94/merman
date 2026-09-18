# Omit identity translations from native sRGB shadow lowering

Baseline: `12ef9974b` plus this increment.
Plan: [theme product boundaries](../plans/2026-09-16-1618-refactor-theme-product-boundaries-plan.md), U8/U10.
Decision: retain the bounded structural simplification.
This is not a latency-improvement or full C7a qualification claim.

## Problem and boundary

The complete public Cyberpunk XY fixture needs 28 single-stage shadows. Previously every sRGB
shadow emitted GaussianBlur, Offset, Flood, Composite and Merge, including an identity Offset.
The resulting 140 conversion primitives exceeded the unchanged default ceiling of 128. Raising
that ceiling was considered but not implemented.

The shared shadow writer now omits Offset only when both coordinates are exactly zero, including
negative zero. Composite then consumes the blur result directly. Nonzero offsets and linearRGB
DropShadow lowering remain unchanged. For S sRGB shadow stages and Z exact identity translations,
the resolved count changes from 5S to 5S-Z. Emitted primitive count and the per-stage lowering/
recognition traversal remain linear in S; this is not a bound on whole-export time or memory.
There is no asymptotic speedup claim. Native resvg's identity-offset implementation returns its input, preserving the
input region. Pixel comparison separately verifies the actual rendering result.

Raw XML validation recognizes only the bounded six/seven-element stage forms; resolved usvg
validation recognizes four/five primitives. Both retain exact input/result wiring, color space,
colors, offsets, ordering and trailing-element rejection. Semantic shadow receipts are unchanged.
No generic SVG filter interpreter, cache, dependency, resource-limit increase or public field was
introduced. Conversion budgets continue to count actual emitted/resolved primitives. Previously
rejected requests may now succeed because they contain less work, not because limits were bypassed.

## Experiment

The preregistered ledger, frozen probe, allocator source identity, executables and raw outputs are
under `target/bench/experiments/zero-offset-shadow-12ef9974b/experiment.yaml`. Host: Apple M4 Pro,
macOS 26.6.2 arm64, rustc 1.95.0. Lockfile SHA-256:
`e008f728a8bd80a163810a66ffc1fb5a456edd44e959068e02237316915d98a2`.
The profile is Release, no defaults, `svg,png,pdf,layout-cytoscape,embedded-fonts`. Unrelated
untracked August history documents were untouched; the baseline executable links the production
Rust source committed as `12ef9974b`. Candidate measurements use the changed production source.

The temporary public-facade probe reuses the checked-in native-memory counting allocator; existing
pipeline and flowchart-memory benches do not expose this native theme-export operation. Each
sample uses a fresh process, prepares one document, performs one warmup export, then measures one
PNG or PDF export. Five repetitions alternate output/control order. The fixed four-category scene
is byte-identical to the public fixture; generated scales are 8, 16, 32, 64 and 128 categories.
An effects-removed control retains typography, canvas and colors. Both browser and native use the
same caller-owned Arial regular/bold bytes recorded in the preceding
[axis-label experiment](../knowledge/engineering/verification/2026-09-18-xychart-axis-label-glow.md).
These font bytes are local experiment resources, not bundled library assets.

Diagnostic scaling explicitly sets total conversion primitives to 4096. Initial scale-16 rendering
also hit the conservative reference-expanded `max_svg_elements` estimate (368334 versus 250000),
so a separately recorded diagnostic profile sets that limit to 64000000. The non-overridable
backend cap remains enforced. Baseline glow at scale 32 then rejects at 1088958 expanded elements
versus 1000000; larger rejected rows remain unavailable export measurements. This is a conservative
reference estimate, not the number of XML elements or allocated nodes observed at runtime.

Baseline: 90 successful samples and 30 rejected samples. Candidate: 100 successful and 20 rejected.
The candidate newly measures scale 32 only under explicit diagnostic limits; no baseline ratio is
claimed for that row. Scales 64/128 with glow still reject. All six effects-removed controls run.

## Results

| Categories | Filter references | Before primitives | After primitives | Native image result |
| ---: | ---: | ---: | ---: | --- |
| 4 | 28 | 140 | 112 | PNG and PDF bytes identical |
| 8 | 40 | 200 | 160 | PNG and PDF bytes identical |
| 16 | 64 | 320 | 256 | PNG and PDF bytes identical |
| 32 | 112 | rejected | 448 | no comparable baseline; diagnostic limits only |

Every comparable PNG and PDF artifact is byte-identical. Chromium 151.0.7922.34 screenshots are
also byte-identical for the three comparable glow scenes at DPR 1. Six additional browser pairs
cover composed SourceGraphic/Previous inputs and mixed zero/nonzero X/Y offsets using the bounded
SVG templates. Actual fixed-scene PDFs rasterized through the existing PDFium library at 96 dpi
are byte-identical; library SHA-256:
`6dbc4ceaa40178e3b583a51144cccd7900a19608fd71f45ecca4a6c766d8024b`.

For the fixed scene, median peak-live-heap growth is 3,927,507 bytes before and after for PNG, and
37,118,747 to 37,085,237 bytes for PDF. Cumulative allocated bytes decrease by 178,740 for each
output. Every comparable row passes the registered peak-memory non-regression bound: baseline
plus max(5%, 1 MiB). Effects-removed allocation controls are exactly unchanged. These are Rust
allocator observations around a warmed export, excluding document preparation and its retained
fonts; they are neither whole-process RSS nor a cold-start measurement.

Instrumented diagnostic medians are 37.26 to 39.08 ms for fixed-scene PNG and 363.11 to 380.25 ms
for PDF. They do not show a measured speedup and were not collected with balanced, A/A-calibrated
confirmation. Acceptance rests on exact redundant-work removal, unchanged output, memory controls
and successful default-budget export. U10 latency acceptance remains separate.

A fresh public-facade capture with **default render and export budgets** now emits PNG/PDF with
112 primitives, 28 filter references and unchanged Portable/Embedded admission under the explicit
caller-font profile. Both files equal the diagnostic-profile artifacts. This closes the fixed
scene's 140-versus-128 gate without promoting the resource-free preset to Portable on every host.

## Validation and remaining work

The first Release nextest run passed 109 tests with no skips: exporter/facade libraries, composed
shadows, XY text/series and canvas exports. New tests cover negative zero, nonzero X/Y preservation,
raw/resolved cross-form receipt and raster equality, wrong masks, nonzero-translation mismatches,
the complete default-budget fixture, and rejection at its exact lower total limit of 111.
The second scoped Release run passed 41 tests with no skips, covering Flowchart node/edge/marker
geometry and glow plus root canvas behavior. Combined: 150 passing tests, no skips. Formatting,
`git diff --check` and Release Clippy on renderer/export/facade libraries passed; Clippy retains
existing warnings. These are scoped checks, not a full workspace or installed-platform matrix.
Logs are `/tmp/zero-offset-tests.log`, `/tmp/zero-offset-shared-tests.log`,
`/tmp/zero-offset-fmt.log` and `/tmp/zero-offset-clippy.log`.

The actual simplify pass found no additional worthwhile simplification. Actual `ce-code-review`
run `20260919-060234-4cb5c8bd` completed without actionable source findings. Review lenses ran
serially in one independent Codex context under the thread-capacity limit; the parent owns runtime
and image verification. The receipt identifier is a machine-local identifier, not the date of
this checkpoint.

Larger default-profile charts remain an open product/resource question. The conservative reference
estimate and primitive caps must not be relaxed by hiding failed rows. PDF's measured cumulative
allocation also remains substantial: scale 32 uses about 1.078 GB of cumulative Rust allocation
with about 42.8 MB peak growth under diagnostic limits. This is hotspot evidence, not a confirmed
regression against alpha.6 or a reason to build new proof infrastructure. U8 whole-scene
qualification and the wider U10 performance/package audit remain open.
