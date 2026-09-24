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
  generated, alignment, and legal gates. Its obsolete circle assertion is fixed.
  The affected renderer run passed 1,650 of 1,654 tests; the four failures were resolved
  by corrected test contracts and source-backed golden refreshes. Ten focused unit
  tests and all three ELK-enabled layout snapshot tests now pass, as do fmt and Clippy.
  Remaining workspace tests and doctests still need completion.
- Consumer focus: five production Web WASM packages build; Web Node tests, Playground
  prepared/lint/build/eight browser smoke cases, and 255 VS Code tests pass. Those full
  WASM artifacts predate the latest renderer repairs and require rebuilding.
- Remaining parity gates: Class passes all 750 mode comparisons. Flowchart has 68
  distinct DOM blockers and 34 stale receipt fixtures; State has 18 DOM blockers;
  Sequence has 25. Timeline passes all 279 comparisons after one source-reviewed
  exact browser wrapping receipt was added.
  Shared rounded-corner line-hop filtering and Sequence per-line height accumulation
  are confirmed source-semantic gaps being repaired before residual admission.
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
