# Alpha.6 to current Class SVG cost — 2026-09-21

Current Class SVG latency remains above alpha.6 by 209.94% for tiny and 102.32% for medium,
with identical SVG bytes. Both exceed the existing regression gate after the SHA upgrade.

## Scope

This checkpoint renews the published-alpha.6 comparison after the accepted
[SHA-256 upgrade](sha2_upgrade_2026-09-20.md). It compares complete default Class SVG operations
on one macOS ARM64 host. Dependencies remain part of the release range; the result does not
attribute the difference solely to typed themes.

The source baseline is published alpha.6, `d529f858ea3d337a1bdc8fe12e44e1403ededf2e`.
The current product source is `aa35110ee`. Its production crates, root manifest and lockfile
match the integrated SHA upgrade at `62e5afcea`. The subsequent `23fd4b8e6` changes only the
preset-usability audit. No production change is introduced by this measurement.

## Comparison contract

Two clean local measurement commits, `cdbc77075` and `e4db6ff9c`, preserve their respective
production source, manifests and lockfiles. Each changes only the benchmark's untimed preflight
SHA-256 hex formatter to the same iterator-based formatter. This accommodates the old and new
digest types without adding a dependency or changing a timed operation. Their complete
`crates/merman/benches/pipeline.rs` files are byte-identical.

Both executables use Rust 1.95.0, the bench profile, no default features, and
`svg,layout-elk,layout-cytoscape` on Apple M4 Pro, macOS 26.6.2. Builds and measurements are serial. Class tiny
and medium are retained because their complete SVG byte identities match. The earlier discovery
excluded Flowchart, Sequence and State fixtures with changed SVGs; this checkpoint does not
reinterpret those rows as equivalent-output regressions.

The preregistered regression gate requires both more than 10% and more than 50 microseconds
of added latency, with simultaneous 95% confidence. Each executable receives eight A/A pairs;
the calibration determines a fixed confirmation count of at least eight, capped at sixteen.
Each invocation uses 30 Criterion samples, two seconds of warmup and three seconds of
measurement. The comparison alternates AB/BA and uses 10,000 paired bootstrap resamples.
Inference uses paired invocation estimates, not individual Criterion samples as independent
release comparisons. Raw reports retain those invocation estimates and pre/postflight identities.

## Results

Both fixtures remain `confirmed_regression`. Calibration is stable for both sizes on both
executables and selects eight confirmation pairs. All source, corpus, executable and output
postflight checks pass; the comparison has zero contract failures. Exit code 1 records the
confirmed regression, not a build or harness failure.

| Fixture | alpha.6 µs | Current µs | Added µs (95% bound) | Relative change (95% bound) |
| --- | ---: | ---: | ---: | ---: |
| Class tiny | 50.65 | 156.99 | 106.34 (105.77–106.95) | +209.94% (+208.27–211.56%) |
| Class medium | 600.34 | 1,214.61 | 614.28 (612.53–615.69) | +102.32% (+101.93–102.65%) |

Bounds have simultaneous 95% coverage over four relative/absolute components, using
Bonferroni-adjusted 98.75% component confidence. There are 64 A/A calibration invocations and
32 confirmation invocations, each configured with 30 Criterion samples. The report retains all
96 invocation estimates; the eight fresh pairs per fixture are the confirmation inference unit.

Class tiny retains 14,421 SVG bytes and 95 elements, with SHA-256
`297d969418ccf35d7fbf63f126e29c3d9a6712e6e265952fdf7c27bdb9012669`.
Class medium retains 74,203 bytes and 576 elements, with SHA-256
`4f6b471d69d3bd5119e6e198887d8e5173bb11f0823155cdfd461e4407a88865`.

## Stage boundaries

The follow-up diagnostics use the same binaries, capabilities and fixtures, with two AB/BA
pairs registered per stage. They retain exact typed-model, layout and SVG identity checks. Their timings
are attribution evidence only; they do not admit a production optimization.

- `parse` reuses an engine; `parse_cold_engine` constructs an engine inside the operation. The
  latter is not process cold start.
- `layout` times family preparation with parsing and cloned-input/session setup outside timing.
- `render` times `PreparedFamilyArtifact::render_svg` with family preparation outside timing.
  It returns a `RenderedFamilySvg` before standalone finalization.
- `end_to_end` times the public `Renderer::render` SVG operation. Its additional work includes
  operation setup, standalone finalization, resource accounting, digests and target admission.

These are independent benchmark boundaries, not additive components. Source ownership places
standalone finalization in `crates/merman-render/src/svg/pipeline/standalone.rs`, XML/reference
budget checks in `svg/pipeline/final_validation.rs`, and public digest/admission completion in
`crates/merman/src/render/document.rs`. A stage difference alone cannot assign an exact share
to one of those operations or justify removing its contract.

| Stage | Class tiny alpha.6 → current µs | Class medium alpha.6 → current µs | Evidence |
| --- | ---: | ---: | --- |
| Reused-engine parse | 3.76 → 4.35 | 76.75 → 77.55 | Two diagnostic pairs; exact model identity |
| Fresh-engine parse | 7.29 → 7.99 | 81.13 → 81.49 | Two diagnostic pairs; exact model identity |
| Layout | Not sampled | Not sampled | Projection identity differs; comparison rejected |
| Family SVG render | 26.66 → 43.16 | 227.84 → 320.41 | Two diagnostic pairs; exact SVG identity |

The layout projections change from 23,769 to 23,723 bytes for tiny and from 41,398 to 41,352
bytes for medium. Their hashes differ, so discovery rejects both rows before timing. The
report records a suite contract failure plus two failed rows. This is an incomparable stage,
not a failed renderer operation or evidence that layout has no regression. The 46-byte change
has not been attributed or normalized away. The diagnostic batch stopped there; after inspecting
that failure, the independently comparable render stage was run with the registered settings.

All three sampled stage reports pass executable/source/output postflight checks. The parse
deltas are below one microsecond in these observations, while family SVG rendering adds about
16.50 and 92.57 microseconds. These observations do not explain the whole end-to-end gap.
Prioritize named operations in family emission and standalone finalization, while retaining the
unmeasured layout delta as an open attribution gap. No new optimization is admitted from these
two-pair diagnostics.

## Limits and next decision

This is a warm native SVG lane for two Class fixtures. It does not measure installed package
size, process cold start, first render, PNG/PDF, themed-output deltas, browser/Node-WASM, Python,
Typst or another host. Same SVG bytes are the comparability gate for this latency question;
they do not imply that alpha.6 and alpha.7 expose identical public admission metadata.

Keep the existing resource, cancellation, XML/reference and digest contracts when investigating
the next candidate. Any proposed change needs its own adjacent baseline, cross-family controls
and semantic/error tests. The prior SHA improvement remains supported by its adjacent experiment;
independent historical timing runs cannot be subtracted to infer an exact recovered percentage.
This checkpoint does not close C7a or U10 and changes no default budget.

## Evidence and reproduction

Raw evidence and preregistration are retained in
`target/bench/experiments/alpha6-current-u10-20260920/`. The directory date reflects the start
of the run before midnight. Reproduction requires the recorded clean measurement revisions;
the production revisions alone have different preflight formatter source.

The final confirmation JSON SHA-256 is
`82e0b0a681d4db8fb5de5fd3e2736e30d9af4f76ad9d671d2d1388515910afe6`.
The original invocation resolved its relative report paths under the head checkout; byte-identical
copies are retained in the main worktree's evidence directory. The command below uses absolute
output paths to avoid that ambiguity.

The base executable SHA-256 is
`5690698b499f1071e6af1e1eb397b0e67af92e6a726c1bb975792fcfef8947d8`;
the current executable is
`b49a02b83234e63296c2afa6deef570e720899e0528a5db7c0b8f24ee60fe208`.
Both use benchmark source SHA-256
`c37c5bdf2029305b6102885e5c204bdeb95c242e6a6141f59ff396352ea2889e`.

| Stage report | SHA-256 |
| --- | --- |
| `parse.json` | `fb5f3bf8590771c23e01fcef343ed3dcdf1fde7929798220340d0af45fe21edc` |
| `parse_cold_engine.json` | `7e41d713b130da91f15c2a8178ef46de1e5502cc89c67c82f2562ffeb3337dc3` |
| `layout.json` (rejected) | `5c6160677b5e014260db04fc6424478cd1b0f0b0c2227659532b5f9d3cda1a7a` |
| `render.json` | `d7e410bc021a89d4786e7847577d526face8c2c1300a00e3402cb6a2d0724ede` |

```console
CARGO_BUILD_JOBS=1 python3 tools/bench/compare_self.py \
  --base-dir /private/tmp/merman-alpha6-u10-common \
  --head-dir /private/tmp/merman-alpha7-u10-common \
  --base-label alpha6 --head-label alpha7-after-sha2 \
  --base-target-dir /private/tmp/merman-alpha6-audit-20260920/target/bench/alpha6-self-all \
  --head-target-dir "$PWD/target" \
  --base-features svg,layout-elk,layout-cytoscape \
  --head-features svg,layout-elk,layout-cytoscape \
  --no-base-default-features --no-head-default-features \
  --base-toolchain 1.95.0 --head-toolchain 1.95.0 \
  --filter 'end_to_end/(class_tiny|class_medium)' \
  --preset long --sample-size 30 --warm-up 2 --measurement 3 \
  --evidence-mode confirmation --calibration-pairs 8 --max-pairs 16 \
  --bootstrap-resamples 10000 \
  --relative-threshold-percent 10 --absolute-threshold-ns 50000 \
  --out "$PWD/target/bench/experiments/alpha6-current-u10-20260920/confirmation.md" \
  --json-out "$PWD/target/bench/experiments/alpha6-current-u10-20260920/confirmation.json"
```

For each diagnostic, replace the filter group with `parse`, `parse_cold_engine`, `layout` or
`render`, set `--evidence-mode diagnostic --pairs 2`, and use a distinct output path. Keep
toolchains, features, fixtures, sample size, warmup and measurement duration unchanged.
