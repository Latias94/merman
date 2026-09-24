---
type: Current State
status: active
---

# Current State

- Active Mermaid parity focus: complete the Mermaid 12.0.0 alignment plan on
  `refactor/mermaid-12-alignment`, targeting unreleased `v0.8.0-alpha.7`. The selected
  reference/runtime bundle and admitted corpus now use 12.0.0 (37 families, 3,716 SVGs).
  Local SVG convergence and artifact-size admission remain incomplete.
- Integration focus: the September 24 strict run passed pre-test feature, dependency,
  generated, alignment, and legal gates. The complete workspace nextest run executed
  8,635 tests: 8,634 passed and the sole failure was an obsolete residual-catalog
  count. That assertion is corrected and all three catalog tests pass. Sequence
  drawing and shared line-hop repairs pass 61 focused tests and are committed.
  Reviewed P3 port ranges, compound boundary offsets, and Neo brace geometry are
  committed. Source-compatible stateful sorting passes all 420 kernel tests.
  The renderer unit failure was a minimum-width fixture error; its corrected
  regression passes. State label minimum-width propagation is committed and all
  41 State layout/SVG tests pass. All 63 reviewed provider/brace goldens are
  committed and the 12 snapshot/Usecase checks pass. Required doctests pass;
  final current-source SVG comparisons remain open.
- Latest verified boundary: ELK now keeps leaf label measurements as paint metadata
  instead of projecting them into provider margins. Equal-size pinned elkjs replay
  proves the removed 191px offset for hidden labels; non-empty group titles retain
  their layout role. All 1,481 all-feature ELK-adapter/renderer unit tests pass.
  Current-source family comparisons and changed layout snapshots still need refresh.
- Latest paint checkpoint: 310 independently reviewed layout snapshots are committed.
  Flowchart/State shadows, rounded radius, State HTML label boxes and Sequence paint
  ordering/palettes are corrected. The 1,604-test batch has 1,602 passes plus two
  source-verified fixture corrections whose reruns pass. P3 shared-property identity
  is the next kernel fix; its first 1,910-test run has one source-replayed old assertion
  pending correction. Fresh route snapshots and exact residual receipts remain open.
- Consumer focus: five production Web WASM packages build; Web Node tests, Playground
  prepared/lint/build/eight browser smoke cases, and 255 VS Code tests pass. Those full
  WASM artifacts were rebuilt before the latest paint/kernel fixes; a final full/render
  rebuild and consumer verification remain.
- Remaining parity gates: fresh current-source comparisons pass all 34 primary
  families outside Flowchart/State/Sequence. Flowchart has 56 distinct DOM blockers
  after the reviewed shape/label batches and 34 stale receipt fixtures; State has
  12 DOM blockers; Sequence has 25 plus three stale receipts. Usecase is outside the
  primary matrix and was not selected by compare-all; its family tests and changed
  golden need separate verification. State compound self-loops, effective theme
  radius, State label minimum width, and Sequence note CSS weight now have
  source-backed fixes. Remaining work is the held State route/measurement set,
  remaining route/measurement differences and stale receipt review. The Neo cylinder,
  document, tape, tag, bow-tie, card, window/divided/stacked rectangle, triangle and
  sloped/shaded geometry batches now have source-backed fixes. Follow-up review added
  the stacked-rectangle polygon intersection and removed two fixture-specific float
  substitutions. The fresh three-mode family comparison still fails; the 96-newshape
  comparison and current full-family reports remain diagnostic evidence, not admission.
- Artifact-size focus: production theme audit snapshots were removed in `6dc4c9f6f`
  after equivalent-output tests and isolated measurement. Both analysis/editor gzip
  and Brotli sizes improve, but the fresh assembled artifacts still exceed analysis
  gzip/Brotli by 37,226/37,900 bytes and editor by 7,351/19,502 bytes. Preserve
  capabilities and budgets. Typst measurement needs local Binaryen 131 on PATH.
- Stable focus: editor-language integration hardening spans SVG safety, platform binding lifecycle
  contracts, editor snapshot memory use, and release-gate coverage.
- Stable decisions: SVG text returned to browser-like surfaces must be validated before DOM
  insertion, copy, export, or preview replay; platform wrappers must document document-analysis
  facts and reusable-engine callback lifecycle; editor snapshots should share document text rather
  than copy every Markdown fence body.

# Citations

- [PR20 post-review refactor plan](../../plans/2026-07-04-005-refactor-pr20-post-review-refactor-plan.md)
- [LSP capability contract](../../lsp/CAPABILITIES.md)
- [Android JNI binding contract](../../bindings/ANDROID_JNI.md)
- [Flutter/Dart FFI binding contract](../../bindings/FLUTTER_DART_FFI.md)
- [Release package surfaces](../../release/PACKAGE_SURFACES.md)

- [Mermaid 12 reference admission checkpoint](progress/2026-09/2026-09-21T013936Z-mermaid-12-reference-projection-admission-0e9509533c9245849d1b61bc38b8a674.md)

- [Mermaid 12 Playground and binding checkpoint](progress/2026-09/2026-09-21T071341Z-mermaid-12-playground-and-binding-verification.md)

- [Mermaid 12 DOM and golden refresh checkpoint](progress/2026-09/2026-09-21T081300Z-mermaid-12-dom-and-golden-refresh.md)

- [Mermaid 12 family convergence checkpoint](progress/2026-09/2026-09-21T092000Z-mermaid-12-family-convergence.md)

- [Mermaid 12 integration closeout checkpoint](progress/2026-09/2026-09-24T035200Z-mermaid-12-integration-closeout.md)
