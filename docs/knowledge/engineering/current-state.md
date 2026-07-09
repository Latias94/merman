---
type: Current State
status: active
---

# Current State

- Active Mermaid parity focus: the repository is pinned to Mermaid 11.16.0, new 11.16 families are
  detector-visible, and `swimlane`, `cynefin`, plus all four railroad variants now have
  source-backed semantic parsing/editor facts while render admission remains deferred until typed
  layout/render paths are ported. Shared frontmatter/config parsing now follows the 11.16
  same-indent delimiter rule and accepts 11.16 diagram namespaces in the local top-level
  frontmatter compatibility layer.
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
