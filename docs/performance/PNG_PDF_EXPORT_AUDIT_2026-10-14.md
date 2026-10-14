# PNG/PDF export audit — October 14, 2026

This audit follows the #201/#202 integration and the review of issue #199's Mindmap screenshot.
The initial source is `051c435fe79c32dc63aa9876969a09ed5661606e`. The frozen Windows CLI has
SHA-256 `fb4dcb43912349db5798777fb48809c4f702902f94427af0290136d6a58f5cb4`.
The investigation uses explicit checked-in Cousine faces, small SVG probes, actual PNG pixels,
PDF extraction/rasterization, and browser comparisons. It does not claim cross-platform pixel
identity or a new rendering-speed improvement.

## Confirmed boundaries and repairs

| Surface | Evidence | Repair |
| --- | --- | --- |
| Mindmap font metrics | Deterministic `with icon` viewport is 61.125px; native Cousine uses 86.421875px. PDF extraction changes from `ith ico` to `with icon`. | Keep native measurement opt-in; expose an operation-local shared font context to the Rust binding engine. |
| Bindings native painting | A host measurement callback supplies metrics but cannot carry its font database into PNG/PDF painting. | Reuse the main facade's `NativeFontContext` for layout, fallback and all native targets, with fresh operation state. |
| HTML emphasis | Native measurement selects Bold/Italic, but old fallback PNGs are identical to Regular and PDF spans all use Regular. | Preserve resolved text runs through the existing whitespace and line planner, then emit isolated SVG runs. |
| Missing glyphs | A closed Latin font set returns success for unsupported characters; PDF glyphs can be `.notdef`. | Report actual font-resolution facts and missing glyph counts, preserving best-effort and strict admission policies. |
| Explicit raster viewport | A 100×100 SVG without `viewBox` becomes a 10×10 PNG; offsets and shadows disappear. | Preserve valid explicit root dimensions; retain content fitting for dimensionless Info-style roots. |
| PDF filter dimensions | Tiny sampling budgets or extreme aspect ratios can round one dimension to zero and silently erase content. | Require every sampled image to have two nonzero dimensions; reject impossible size/budget combinations. |
| PDF filter accounting | Mask/pattern streams and isolated streams do not share the page's absolute transform. | Budget the actual localized sampling streams and include reachable resource subtrees. |
| Directional filter transforms | resvg 0.47 loses rotated, skewed or reflected offsets. PDF localization fixes ancestor transforms, but not a filter group's own transform. | Continue encoding and retain a directional-transform diagnostic; explicit portable-output admission rejects this known residual. |
| SVG stylesheet priority | usvg 0.47 can prefer a stylesheet's `!important` rule over derived inline `!important`, changing projected text. | Keep the correctly expressed SVG and best-effort output, with a fallback-CSS diagnostic; do not patch the upstream cascade. |

Default deterministic measurement remains useful for headless, reproducible layout. Selecting
native measurement for precise glyph fitting is a reasonable explicit contract. It does not
excuse lost HTML styles, altered explicit viewports, or silent empty filter output caused by
Merman. Known upstream fidelity limits remain visible in reports while default export continues.

## Integration and evidence policy

- Font acquisition stays host-owned. No JSON font paths or implicit system scanning are added.
- Native contexts and host callbacks are mutually exclusive; explicit JSON measurement selectors
  also conflict with constructor-owned native fonts.
- Each engine operation shares loaded font data while creating independent cancellation, cache,
  failure and measurement counters. The loading operation cannot cancel later requests.
- C, UniFFI, JNI, Node, Web and Typst do not gain an unimplemented native-font constructor.
- New run styles isolate ordinary SVG-only selectors. Unsupported projection effects and upstream
  priority differences lower fidelity admission instead of suppressing the image. Shared resolved
  strings avoid copying long inherited font names for every text leaf.
- Existing layout goldens, comparator tolerances, resource ceilings and portability assertions
  are not widened to admit the repairs.

The Linux CI failures also had distinct causes: State's ordinal test assumed the retired flat
fallback shape; Flowchart's new viewport matched root SVG typography and falsely failed filter
isolation. The repaired assertions preserve occurrence, viewport and important fill checks;
the renderer explicitly isolates viewport typography and keeps native filter receipts strict.

Current capture recipes and output evidence live in
[`issue-199-font-bounds`](../assets/issue-199-font-bounds/README.md). Historical #201 images remain
immutable and retain their original API/revision labels.

## Backend direction

[resvg #1155](https://github.com/linebender/resvg/pull/1155) is an open upstream implementation
that supersedes the narrower offset-vector proposal in #1081. It separates rotation/shear from
filter-layer sampling, paints the layer in local coordinates, then transforms the complete
bitmap. This is the correct direction for offset, region clipping and intermediate filters.
The directional diagnostic describes a backend limitation, not a completed fidelity fix.

A workspace-only Cargo patch is insufficient: dependency patches are read only from the consumer
workspace root, and krilla-svg must use the same corrected rasterizer. A future backend update
must validate matching resvg/usvg/krilla dependencies for distributed CLI artifacts and crates.io
consumers together. This PR does not add an unpublished dependency fork or claim an upstream fix
has shipped.

Dimensionless raster content fitting uses the parsed layer bounds, including filter paint. PDF
retains a source viewBox or explicit dimensions; when neither exists, painted content outside the
backend-derived viewport produces an implicit-viewport clipping diagnostic. Default output still
includes the supported content and does not rewrite sealed SVG geometry. A fuller automatic PDF
viewport projection belongs at the SVG owner, with its geometry and evidence bound before sealing.

Default best-effort export attempts all supported content and reports known residuals through
export reports, target-admission reasons, binding metadata and CLI warnings (`--quiet` suppresses
the latter). Only explicitly requested portable output rejects these fidelity residuals. Invalid
geometry, cancelled work and impossible resource budgets still fail with their existing errors.
These export repairs add no resvg/usvg fork or workspace patch for upstream fidelity limits.

## Validation of export repair commit

The following receipts belong to export repair commit
`783bacdb785bbefb7894ec7c9b782fd30041f356`, before the later base-branch integration below.

Validation used Rust 1.95.0 on Windows, with serialized Cargo commands. Local diagnosis receipts
remain under ignored `target/issue199-font-bounds-20261014` and `target/issue199-export-audit`.

- Render/export libraries: 3,222 passed, two existing audits skipped.
- Bindings library: 371 passed, including quality-reason projection and font-state isolation.
- Six focused integration binaries: 122 passed initially; two test setup errors were corrected
  and rerun successfully. The original Linux Flowchart glow and State occurrence failures pass.
  Font, glyph-bound, CLI output/quiet and strict-admission tests execute actual parsing and encoding.
- Five affected crates pass all-targets/all-features Clippy with warnings denied.
- Complete 37-family `structure,parity,parity-root` SVG comparison passes: 3,756 selected,
  3,729 rendered, 27 documented skips. No baseline, tolerance or accepted residual scope changed.
- Browser root-oracle contracts: 20 passed. Node API/wire contracts: 56 passed. Platform verifier
  contracts: 27 passed. Full painted-corpus and platform SDK builds remain CI-owned.
- Binding generation, binding-contract and native-ABI checks pass. All 45 feature-matrix builds
  and isolated consumers pass; representative artifact dependency closures pass. The first matrix
  attempt exhausted C: disk space; generated build caches moved to E: before the successful retry,
  without source changes.
- Registered native-memory protocol smoke passes both 1x/100x operation/zero boundaries and the
  zero-work measurement verifier with `CARGO_PROFILE_BENCH_OPT_LEVEL=0`, recorded in its report.
  The optimized attempt reached its 900-second prebuild timeout and produced no measurement.
  Neither run establishes optimized memory or latency improvement.
- Performance Python contracts: 217 passed. Legal generator/projection contracts: 18 passed.
- All 13 dependency license reports pass with the pinned cargo-about 0.9.2; 394 release legal
  projections and the third-party contract pass. No generated license content changed.
- Rustdoc fragments were refreshed with the existing generator; build and freshness checks pass.
  Capture hashes, staged whitespace and the 160-unit path budget pass. Affected Rust files pass
  formatting; the full invocation exceeds Windows command length and remains Linux CI-owned.

New-head Linux tests, the full browser painted corpus and native SDK target builds are not
claimed from these Windows checks. This audit changes no release recipe or dependency version.

## Integration with the updated theme base

PR #178 advanced to `95d0e452021edb3b6a7e1be25e5d5eddca382831` after the export repair
commit. PR #202 merges that frozen base while preserving both branches' contracts:

- The base's exact theme-variable owner predicate and locally resolved compatibility values
  combine with the export branch's atomic publication and detector mutation revisions.
- `SvgPipelineBuffer` owns the SVG and same-generation `ValidatedSvgXmlFacts`; the retired
  ordinary-budget tuple is removed. External processors invalidate facts even for equal bytes.
  Built-in Readable mutations collect fresh final facts instead of adding another XML traversal.
- Terminal resource/reference collections still move into the sealed report, including the
  fallback-CSS diagnostic. Publication observes cancellation after the final observations.
- XYChart retains compiled common CSS and actual font-write receipts while adopting the base's
  XML-safe stylesheet output. No comparator policy or layout golden changes.

The updated base separately imports a pinned upstream iterative XML parser into its private
XML owner, with source provenance, retirement criteria and generated legal entries. That base
change is unrelated to rasterizer fidelity; these export repairs do not patch resvg or usvg.
Historical pictures and the preceding measurements retain their frozen source labels.

Merged-tree validation uses the same Windows Rust 1.95.0 toolchain and serialized Cargo:

- Core/render/export/bindings all-feature library tests: 5,752 passed, two existing audits skipped.
  The public completion test compares identical output against the three-walk reference and
  observes one XML traversal for immutable Parity and two for Readable.
- The first merged library compile found old test-helper node types after the base's XML-owner
  migration. Helpers now use the same private XML owner; the retired ownership predicate and
  its obsolete-only test are removed rather than retained with warning suppression.
- Ten focused native/family integration binaries: 212 passed. Both originally failing Linux
  Flowchart-glow and State-occurrence tests pass. Actual encoding proves default residual output,
  quiet-aware CLI notices, strict rejection and shared native fonts.
- The full 37-family `structure,parity,parity-root` gate passes again: 3,756 selected,
  3,729 rendered and 27 documented skips. The cold release-tool build took 32m34s;
  corpus comparison then completed separately. No baseline or comparison allowance changed.
- Rustdoc freshness passes with the newly built all-feature CLI. Scoped formatting passes for
  142 merged Rust files; pinned upstream XML files retain the base's declared formatting exclusion.
- Five affected crates pass all-targets/all-features Clippy with `-D warnings`.
- With pinned cargo-about 0.9.2, all 13 merged dependency reports pass (3,776,566 bytes),
  as do 424 release projections, third-party metadata, 18 legal contracts, 36 representative
  artifact closures, binding-contract and native-ABI verification. The no-default-feature
  core/render build passes with the existing disabled-leaf warnings.

The broad feature matrix, compiled native-memory experiment, Node/browser and platform SDK
receipts above remain bound to their earlier source. They are not claimed as reruns of this merge.
New-head Linux, full painted-corpus and SDK owner lanes remain pending.
