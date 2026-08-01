---
type: Current State
status: active
---

# Current State

- Active Mermaid parity focus: the repository is pinned to Mermaid 11.16.0. The primary SVG matrix
  contains 35 source-backed families; `zenuml` remains the sole compatibility-only family. The
  11.16 TreeView, Ishikawa, EventModeling, Venn, Swimlane, four Railroad dialects, Wardley, and
  Cynefin families now have typed semantics, editor facts where upstream provides them, layout,
  SVG rendering, pinned baselines, and executable comparison facts. Shared frontmatter/config
  parsing follows the 11.16 same-indent delimiter rule and projects configuration namespaces from
  the family catalog.
- Golden refresh focus: regenerate 11.16 baselines after source-backed code changes. Known upstream
  regressions such as Mermaid issue #7954 must be classified separately from local drift.
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
