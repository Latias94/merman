---
type: Current State
status: active
---

# Current State

- Active Mermaid parity focus: the repository is pinned to Mermaid 11.16.0, new 11.16 families are
  detector-visible, `cynefin` now has a source-backed compatibility renderer slice with semantic
  and layout fixture admission, and `swimlane` plus all four railroad variants have source-backed
  semantic parsing/editor facts with parse-only fixture/golden admission. Primary SVG admission for
  the new 11.16 families remains deferred until upstream SVG baselines and compare commands exist.
  Shared frontmatter/config parsing now follows the 11.16
  same-indent delimiter rule and accepts 11.16 diagram namespaces in the local top-level
  frontmatter compatibility layer. Existing-family U4 deltas now include Pie highlight semantics,
  XYChart point labels/axis rotation, Architecture align layout hints, ER nullable/backtick/comma
  attribute parsing, State same-line composite diagnostics plus 11.16 State SVG DOM/layout
  alignment, Flowchart subgraph `direction TD` preservation, and TreeView 11.16 node
  annotations/box-drawing/icon DOM semantics.
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
