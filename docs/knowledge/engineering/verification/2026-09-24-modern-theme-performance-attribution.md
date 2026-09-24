---
type: Audit Report
title: Modern Mermaid README theme capability and performance attribution
timestamp: 2026-09-24
source_commit: 8325653c3
reference_commit: a021cbce37fc0b07a9f4791c28e983101ea06f2d
comparison_revision: v0.8.0-alpha.6
---

# Decision

The current product can produce a complete Cyberpunk scene with visible glow in SVG, PNG and PDF
within bounded workloads. Larger scenes can exceed native export limits and must fail explicitly.
Keep that boundary; do not raise limits or silently remove effects to make demonstrations pass.
The maintainer accepts explicit failure when faithful adaptation is not cost-effective.

The README appearance goal remains open. Public Brutalist and Spotless currently supply shared
palette recipes, not the reference's full hard-shadow or paper-grid treatments. Their successful
exports do not prove those missing effects. Ghibli, Memphis and HandDrawn have no public presets.
The initial delivery should retain Cyberpunk's bounded scenes and add real, family-scoped
Brutalist and Spotless treatments before expanding the catalog.

The default-render performance regression remains open. Fresh CPU sampling identifies ordinary
XML/reference budget validation as a substantial cost center. Code inspection also finds Class
theme terminal-expectation construction on the no-theme path. The regression cannot be assigned
entirely to theme styling, but it cannot be declared unrelated to the theme architecture either.

## Corrections to the first version of this report

- The font-retirement comparison has theme code on both sides. Its approximately ±2% changes
  support only a non-regression observation for that removal; they do not establish that theme
  rendering overhead is acceptable.
- `profile_render` logs total iterations over about eight seconds, not iterations per second.
  The table below divides by actual elapsed time.
- Its `render` stage includes session creation, parsed-model cloning, preparation/layout and SVG
  emission. Call it **prepare + render**. Criterion `pipeline` has a separate render-only timed
  body, with preparation outside that body. Do not mix these two measurements.
- Ordinary BestEffort SVG validates XML and reference budgets. It does not perform the strict
  resvg compatibility/resource-closure certification. A resource fingerprint is a hash of SVG
  bytes and font policy, not a resource-closure scan.
- The earlier Class CPU samples were collected at `e4db6ff9c`, not this revision. Fresh samples
  below supersede them for current hotspot attribution.
- The 18 Cyberpunk native failures in the 510-output reference probe occurred on larger literal
  examples. They do not imply that the three bounded public scenes fail.

# Appearance and output boundaries

| Reference design | Required visible identity | Current public preset |
| --- | --- | --- |
| Brutalist | Bold outlines, hard offset shadows, strong ordinal accents | Shared palette only; dedicated treatment remains open |
| Cyberpunk | Cyan/magenta glow, navy grid and layered background | Complete scoped recipe; bounded Flowchart, Sequence and XY scenes verified below |
| Ghibli | Cream paper, brown lines, soft card shadows | Not present |
| Memphis | Heavy black lines, hard shadows, vivid ordinal colors, geometric texture | Not present |
| Spotless | Paper grid, manual-like ink and typography | Shared palette only; dedicated treatment remains open |
| HandDrawn | Handwriting and irregular-line treatment | Not present; host-font and geometry/filter scope must be explicit |

The recipe dispatch in `crates/merman-render/src/diagram_theme/presets/catalog.rs:254` selects a
dedicated builder only for Cyberpunk. Brutalist/Spotless go through `build_cross_family_recipe`.
Inspection of their actual Flowchart PNGs confirms palette changes without the reference hard
shadow/grid treatment. This is a product gap, not an export failure.

## Current correctness observations

Source `8325653c3` was rebuilt with `cargo build --locked --release -p merman-cli` using one build
job. Production sources match the preceding diagnostic revision; only documentation changed.
All captures use the current host's fonts, not embedded theme font resources.

- **54/54 exports succeeded:** Brutalist, Spotless and Cyberpunk; public Flowchart, Sequence and
  XY fixtures; SVG, PNG and PDF; both default label configuration and `htmlLabels: false`.
  This is execution coverage, not appearance qualification for all three presets.
- **110/110 Cyberpunk PDF pixel probes passed:** 26 Flowchart, 35 Sequence and 49 XY observations.
  The existing `tools/debug/check_cyberpunk_pdf.py` renders actual PDFs through PDFium at 96 dpi,
  requires identical pixels after raw SVG replay, then removes individual effects, markers,
  canvas layers and labels to prove that each contributes pixels. Label probes disable glow first.
  Source and output hashes, PDFium/Pillow versions, dimensions and font scope are recorded.
- **Four expected native rejections:** literal reference examples 01 and 05 still emit SVG;
  their PNG and PDF exports return the stable
  `max_total_svg_conversion_filter_primitives` error at 132 and 136 against the cap of 128.
  Fresh output paths remain absent. The counter may stop at the first exceeded bound; these
  values are not necessarily the completed scene's total primitive count.

The 110 probes prove visible contributions, not exact agreement with reference blur pixels or
universal absence of clipping. Font equivalence across hosts and broad family qualification remain
open. No `qualified_cells` were promoted. Visual inspection of the bounded Flowchart PDF raster
also confirms readable labels, arrowheads, cyan glow and the full grid background.

The earlier literal-example probe ran 34 examples × five presets × three formats: 477/510
executions succeeded. Of 33 failures, 18 were Cyberpunk native-budget rejections and 15 came from
one GitGraph input with an unsupported branch reference. That probe checks execution/signatures,
not visual fidelity.

## PDF adaptation decision

The existing export path preserves vector content where supported and rasterizes filtered regions
locally. `PdfOptions` requests filter scale 4 by default and separately budgets aggregate filter
image pixels. That is useful existing support; no new backend or dependency is needed for the
bounded glow scenes. The captured `htmlLabels: false` Cyberpunk PDFs are 489,421, 408,308 and 366,365 bytes
for the Flowchart, Sequence and XY scenes respectively.

Maintain these rules:

1. Preserve requested stage order, color/alpha, labels, markers and background within supported
   budgets. Validate actual PDF pixels, not only a PDF header or filter receipt.
2. Keep filter-complexity ceilings active. Lowering raster scale does not remove the primitive
   count limit. An SVG success is not a native-export guarantee.
3. For an unsupported effect or exceeded budget, retain explicit error/admission behavior. A
   caller may explicitly edit its recipe; the renderer must not silently remove glow, choose a
   different preset or relax limits.
4. The existing reported filter-image sampling policy is separate from primitive complexity.
   Do not change sampling quality or report a new fidelity guarantee in a performance patch.

# Default-render performance attribution

## Historical-to-current stage diagnostic

Both executables use default `complete-svg` features and the same inputs, without an explicit
preset. The example source is identical. These are sequential single-run diagnostics, not
calibrated A/A and AB/BA confirmation. The XOR length checksum does not prove equal output bytes;
use the separate matched Class evidence for a decision-grade release-range regression.

| Scene | Profile stage | alpha.6 ops/s | Current ops/s | Current/base |
| --- | --- | ---: | ---: | ---: |
| flowchart_tiny | parse | 198,594.6 | 185,611.9 | 93.46% |
| flowchart_tiny | prepare | 72,674.6 | 55,825.0 | 76.81% |
| flowchart_tiny | prepare + render | 36,070.5 | 21,229.5 | 58.86% |
| flowchart_tiny | end-to-end | 28,293.1 | 9,585.8 | 33.88% |
| sequence_medium | parse | 34,193.8 | 34,756.1 | 101.64% |
| sequence_medium | prepare | 13,677.4 | 20,482.0 | 149.75% |
| sequence_medium | prepare + render | 9,147.7 | 6,202.4 | 67.80% |
| sequence_medium | end-to-end | 7,115.0 | 3,072.2 | 43.18% |
| class_medium | parse | 13,019.2 | 13,235.8 | 101.66% |
| class_medium | prepare | 3,346.6 | 2,926.0 | 87.43% |
| class_medium | prepare + render | 1,890.3 | 1,527.0 | 80.78% |
| class_medium | end-to-end | 1,647.2 | 938.8 | 56.99% |

Parse is not the dominant regression signal. Sequence preparation improves while its end-to-end
operation regresses. This prioritizes SVG emission and post-emission work for investigation; it
does not allow subtracting independently measured stages to obtain an exact causal breakdown.
The earlier default CLI comparison showed stripped growth of 6,987,552 bytes (19.00%). That is
whole-version footprint, not size attributed to one theme module or dependency.

## Fresh current CPU samples

Rebuilt `profile_render` with default features plus redundant `svg`, optimized bench profile.
Each fixture ran the public end-to-end operation for 13 seconds; macOS `sample` attached after
one second for eight seconds at 1 ms intervals. Runs were serial. Attached-sampler timings are
excluded from latency evidence. No theme preset was requested.

| Symbol scope | Flowchart tiny (6,646 samples) | Sequence medium (6,530) | Class medium (6,047) |
| --- | ---: | ---: | ---: |
| Standalone finalization | 37.21% | 33.45% | 24.79% |
| Ordinary XML/reference budget validation | 36.50% | 33.20% | 24.49% |
| XML element validation | 19.73% | 19.98% | 14.07% |
| Family SVG emission | 17.79% | 25.60% | 23.35% |
| SHA-256 symbols across callers | 7.78% | 6.66% | 4.93% |

These are inclusive, overlapping stack counts. XML element validation is inside budget validation,
which is inside finalization; do not add percentages or interpret them as removable time. Optimized
symbol inlining/merging also prevents exact attribution of small helper costs without disassembly
or an isolated experiment. This is current hotspot evidence, not a speedup or acceptance result.

## Actual default path and remaining hypotheses

`render_svg_target` always finalizes the standalone artifact. With `pipeline=None` and BestEffort,
`StandaloneSvgArtifact::finalize_exact` hashes the resource identity and checks XML/reference
budgets. Strict resvg/CSS/resource-closure certification belongs to different policy branches.
The ordinary validator already collects XML and reference facts in one traversal; the previously
merged single-pass optimization must not be proposed again as unfinished work.

Class has an empty prepared-text ledger. Its partition helper still searches for reserved label
spelling, but does not run token-bearing XML rewriting.
Flowchart and Sequence have separate prepared-text paths and must be measured independently.

There is theme-related work without a selected theme: `svg/parity/class/render.rs:54-99` builds
node, relationship and marker expectations, an ID map, a cloned node list and a terminal receipt.
`class/theme/evidence.rs:65-67` skips final theme aggregation only later. This is a concrete candidate
for demand-driven ownership, but current sampling does not prove how much time it would save or
which expectations also serve non-theme source styles.

Prioritize experiments in this order:

1. Attribute allocations/work inside ordinary XML attribute validation and reference-plan
   construction. Preserve QName/namespace/entity checks, duplicate attributes, raw element and
   reference amplification limits, error ordering and cancellation.
2. Determine which Class terminal expectations are genuinely theme-only. Reuse or avoid those
   only when the owning layer proves they are unnecessary; retain source-style and marker checks.
3. Measure reserved-label scanning and artifact hashing separately. Reusing immutable artifacts
   may help, but changing domain-separated fingerprints or dropping token defenses changes
   contracts and is not a transparent optimization.
4. Keep effect-heavy resvg-safe SVG, PNG and PDF as separate workloads. Glow is legitimate opt-in
   work; default no-preset CPU samples cannot measure its marginal cost.

The necessary checks have value. Their current implementation cost is not automatically justified
by that value. No production bypass, budget relaxation or optimization was made in this checkpoint.

# Reproduction and evidence

Ignored evidence root: `target/bench/experiments/theme-correctness-20260924/`.

- `matrix.json`: exact 54 commands, source/output hashes, CLI hash and host.
- `boundary-errors.json`: six larger-example executions, including four expected native errors.
- `pdf-pixels/results.json`: 110 pixel probes and reader/tool versions.
- `nextest.log`: 24/24 focused `theme_composed_effects` and `theme_xychart_text` tests passed
  with `--features svg,png,pdf`, one build job and one test thread. The suites include primitive
  budget rejection, composed shadows, text paint and marker preservation.
- `profile/runs.json`, `*.sample.txt`, `symbol-counts.json`: current executable/input identity and
  raw CPU samples. Count only the call graph, not the repeated bottom-of-stack summary.
- Earlier stage logs: `target/bench/experiments/theme-cost-attribution-20260924/` and the alpha.6
  worktree `/private/tmp/merman-alpha6-u10-common/target/bench/experiments/theme-cost-attribution-20260924/`.

The existing PDF probe can be reproduced without adding application dependencies:

```console
uv run --with pillow --with pypdfium2 python tools/debug/check_cyberpunk_pdf.py \
  --cli target/release/merman-cli --output <new-output-directory>
```

A bounded README acceptance cell must specify recipe, family, source, target and host font
conditions; show actual intended visual mechanisms; preserve labels, geometry and markers; and
retain explicit unsupported/over-budget behavior. Export success and catalog presence alone do
not close that cell. Brutalist/Spotless dedicated recipes and the overall performance/footprint
acceptance remain open.
