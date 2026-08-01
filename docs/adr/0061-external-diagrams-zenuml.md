# ADR 0061: Source-Backed ZenUML Support

- Status: accepted
- Date: 2026-02-10
- Last amended: 2026-07-19

## Context

Mermaid registers ZenUML as an external diagram. Its syntax, semantic model, and renderer are owned
by `@mermaid-js/mermaid-zenuml` and `@zenuml/core`, not by Mermaid's built-in Sequence family.
Treating ZenUML as Sequence syntax discarded participant annotations, starters, groups, fragments,
assignments, creation, expressions, source ranges, and ZenUML's own layout topology. It also split
detection, editor behavior, and rendering across unrelated implementations.

The selected Mermaid reference bundle records three distinct dependency facts:

- the exact ZenUML Core version resolved by the Mermaid workspace is the behavior oracle;
- the highest stable version satisfying the plugin's published range is an admission candidate;
- a newer stable release outside that range is only a separately scoped port candidate.

For the current reference graph, the compatibility matrix in
`tools/upstreams/ZENUML_CORE_ADMISSION.json` selects ZenUML Core `3.50.1`. The matrix retains the
workspace oracle and records parser, renderer, security, resource, host-integration, and corpus
evidence. A semver range alone is never sufficient evidence for selecting a candidate.

The selected Mermaid plugin calls ZenUML Core's `renderToSvg` and returns native SVG. Executable
corpus evidence shows that this SVG can pass the same strict inline publication boundary used for
other Mermaid output. The old assumption that ZenUML required a `foreignObject` compatibility path
was incorrect for the selected graph.

## Decision

ZenUML is a first-class, family-owned Merman diagram:

1. `merman-core` owns a grammar-derived lexer, parser, syntax tree, semantic construction, recovery,
   exact source ranges, editor facts, and complexity model based on the selected ZenUML Core source.
2. One `ZenumlDiagramRenderModel` feeds detection, analysis, editor services, layout, resource
   enforcement, and rendering. ZenUML is never translated through Sequence JSON or a line parser.
3. `merman-render` owns typed ZenUML layout and SVG topology for participants, lifelines, messages,
   creation, returns, occurrences, groups, references, dividers, comments, and fragment sections.
   Source-backed ZenUML assets are vendored with their license.
4. Browser comparison registers the exact plugin graph from the generated Mermaid reference
   bundle in an opaque-origin execution realm. The parent accepts output only after the shared
   strict SVG validator succeeds; rejection is terminal for that operation.
5. The Rust implementation ports observable headless behavior, not incidental JavaScript
   containers, framework state, or browser caches. No separate Cargo feature is introduced merely
   because Mermaid loads the browser plugin lazily.

## Evidence Contract

An admitted ZenUML graph must prove all of the following before selection:

- positive and malformed grammar corpus behavior with recoverable diagram identity;
- family-owned semantic, editor, completion, rename, and structure behavior;
- headless topology, labels, colors, geometry, and resource limits;
- cold and reused browser registration, strict native-SVG publication, and failure recovery;
- exact oracle/candidate package provenance and an explained result for every matrix delta.

The generated bundle, package locks, source checkouts, and admission report must agree. Updating a
package constant without regenerating and verifying this graph is an invalid upgrade.

## Consequences

- ZenUML behavior can evolve without weakening Sequence or maintaining a compatibility parser.
- Parser, LSP, headless rendering, and Playground support report the same capability identity.
- Browser execution still carries third-party-code risk; opaque execution and strict parent-side
  publication are both required, and neither is described as absolute network isolation.
- Future ZenUML Core releases must pass the same admission workflow. An outside-range major is not
  selected under the label "latest" without explicit behavior-port work.

## Rejected Alternatives

1. Translate ZenUML into Sequence.
   This loses language and layout semantics and creates two owners for one family.
2. Extend a regex or line heuristic.
   Recovery and nested fragments require a grammar and exact source ownership.
3. Embed JavaScript in the Rust renderer.
   Headless Rust needs its own typed semantic and geometry artifacts; browser reference execution
   remains evidence, not the production Rust implementation.
4. Accept arbitrary rich HTML or bypass SVG validation for ZenUML.
   The selected producer emits native SVG, so a broader publication format would add attack surface
   without a current producer.
