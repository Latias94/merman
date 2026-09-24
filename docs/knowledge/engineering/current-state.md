---
type: Current State
status: active
---

# Current State

- Active Mermaid parity focus: implement the Mermaid 12.0.0 alignment plan on
  `refactor/mermaid-12-alignment`, targeting `v0.8.0-alpha.7` (unreleased main changes plus
  this upgrade). The working reference bundle and runtime projections now select 12.0.0;
  admission is still incomplete because the accepted SVG corpus and signed residual evidence
  have not yet been refreshed. Do not treat a version projection as completed parity.
- Golden refresh focus: the serial target-runtime generation is staging genuine Mermaid 12
  SVGs in `target/mermaid12-upstream-svgs-all-4` after correcting root background injection and
  Class duplicate parsing in the reference runner. The earlier all-3 output is superseded for
  affected families. Keep existing manifests at their actual
  identities until their new SVG bytes are reviewed and promoted. Do not relabel old output
  or refresh residual signatures without examining the new differences.
- Playground Compare now builds against Mermaid 12.0.0 with built-in ELK. The rebuilt
  production full WASM (`7096c4af12f0`) passed source-freshness and distribution checks;
  browser smoke verified Compare lifecycle, built-in ELK, Agentflow/Usecase, and automatic
  versus explicit default themes. The eight smoke cases pass after fixing the theme test's
  accessible button name. Targeted WASM tests pass 39/39; enabled C FFI and UniFFI new-family
  failure/reuse checks pass. Later Class/CSS production fixes require a final WASM rebuild.
  Flowchart/Sequence tests with ELK pass 178/178; Class/State and focused strict CSS checks pass
  after source-backed fixes. Reference/golden admission and all-package feature/size checks remain.
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
