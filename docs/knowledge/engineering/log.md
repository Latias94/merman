---
type: Engineering Log
---

# Log

## 2026-07-09

- Completed the Mermaid 11.16 `swimlane` semantic parser/editor-facts slice: `swimlane-beta`
  reuses Flowchart grammar and LSP facts, defaults layout to `swimlane`, and remains render-unadmitted
  until a typed render parser/layout port exists.
- Completed the Mermaid 11.16 `cynefin` semantic parser/editor-facts slice: `cynefin-beta` now
  parses source-backed domains, items, transitions, title/accessibility fields, duplicate-domain
  replacement, and self-loop filtering while render admission remains deferred.
- Completed the Mermaid 11.16 railroad semantic parser/editor-facts slice: IR, EBNF, ABNF, and PEG
  variants now parse into the shared AST JSON and remain render-unadmitted until layout/render
  admission is ported.
- Completed the U3 frontmatter/config semantics slice: detector and preprocess frontmatter
  handling now require matching delimiter indentation, and top-level diagram namespace compatibility
  includes the 11.16 config namespaces covered by the generated defaults.

## 2026-07-04

- Consolidated editor-language hardening around SVG DOM safety, VS Code preview refresh
  reliability, reusable binding lifecycle docs, editor snapshot text sharing, workflow path gates,
  and web script argument validation.
- Public platform docs now treat document analysis/facts and reusable-engine callback lifecycle as
  part of the wrapper contract instead of implementation trivia.

## 2026-06-18

- Verified source-backed Flowchart ELK probes are green.
- Ported compound parent-end external dummy net-flow handling in `merman-elk-layered` closer to ELK
  `calculateNetFlow` behavior.
- Added regression coverage for parent-end external dummy net-flow behavior and existing compound
  metadata tests still pass.
- Ported inside-self-loop handling so ELK `insideSelfLoops.activate` nodes create nested graphs and
  `inside_self_loops_yo` edges are imported into the source node nested graph.
- Added regression coverage for inside-self-loop nested graph creation and kept source-backed probe
  coverage green.
- Verified `cargo test -p merman-elk-layered --tests`, `cargo test -p merman-layout-elk --tests`,
  `cargo run -p xtask -- check-flowchart-elk-source-backed-probes`, and `cargo fmt --all`.
