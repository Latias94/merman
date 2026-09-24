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
- Consumer focus: five production Web WASM packages build; Web Node tests, Playground
  prepared/lint/build/eight browser smoke cases, and 255 VS Code tests pass. Those full
  WASM artifacts predate the latest renderer repairs and require rebuilding.
- Remaining parity gates: a fresh current-source comparison passes all 34 primary
  families outside Flowchart/State/Sequence. Flowchart has 64 distinct DOM blockers
  and 34 stale receipt fixtures; State has 12 DOM blockers; Sequence has 25 plus four
  stale receipts. Usecase is outside the primary matrix and was not selected by
  compare-all; its family tests and changed golden need separate verification.
  State compound self-loops now pass. Source reviews found further real gaps in
  compound child-publication order, State theme radius, Sequence note CSS weight,
  and Neo Flowchart shape geometry. Fix these before admitting measurement receipts.
- Artifact-size focus: production theme audit snapshots were removed in `6dc4c9f6f`
  after equivalent-output tests and isolated measurement. Both analysis/editor gzip
  and Brotli sizes improve, but all four compressed budgets still fail. Preserve
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
