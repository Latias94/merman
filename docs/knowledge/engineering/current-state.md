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
  SVGs in `target/mermaid12-upstream-svgs-all-3`. Keep existing manifests at their actual
  identities until their new SVG bytes are reviewed and promoted. Do not relabel old output
  or refresh residual signatures without examining the new differences.
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
