# Architecture Diagram Admission Contract

This document defines the admitted Architecture parser, Cytoscape/FCoSE layout, and SVG contract.

Baseline: Mermaid `11.16.0` at `7c0cafcf42e76bfaf79d0cbbd12edb986612f014`.

Upstream references:

- Parser/AST bridge: `repo-ref/mermaid/packages/mermaid/src/diagrams/architecture/architectureParser.ts`
- DB/model: `repo-ref/mermaid/packages/mermaid/src/diagrams/architecture/architectureDb.ts`
- Upstream tests: `repo-ref/mermaid/packages/mermaid/src/diagrams/architecture/architecture.spec.ts`

## Supported (current)

- Header:
  - `architecture-beta`
  - Allows empty lines above the header.
  - Allows `title ...` directly on the header line: `architecture-beta title sample title`
- Title and accessibility (common parser terminals):
  - `title ...` (stops at `%%` comment)
  - `accTitle: ...` (stops at `%%` comment)
  - `accDescr: ...` (stops at `%%` comment)
  - `accDescr { ... }` multi-line block, ends at first `}`
- Statements:
  - `group <id>(<icon>)?[<title>]?( in <parent>)?`
  - `service <id>(<icon>)|<quoted iconText>?[<title>]?( in <parent>)?`
  - `junction <id>( in <parent>)?`
  - Edge (colon form):
    - `<lhsId>{group}?:<L|R|T|B> <|>? (-- | -[Title]-) <|>? <L|R|T|B>:<rhsId>{group}?`
- Inline comments:
  - Trailing `%% ...` is ignored unless inside quotes.

## Output Shape

- `type`, `title`, `accTitle`, `accDescr`
- `groups[]`, `nodes[]`, `services[]`, `junctions[]`, `edges[]`
- `config`

## Layout And SVG Admission

- Service bounds follow Cytoscape's body/label union and final expansion phases; compound group
  bounds are derived from children with labels before final root viewport emission.
- Because upstream Architecture label groups do not expose `data-id`, the semantic-label adapter
  binds labels to the path in their direct owning edge group and rejects repeated-text ambiguity.
- `stress_architecture_batch3_parallel_edges_and_labels_057` is the signed label canary and requires
  exact geometry; Architecture has no semantic-label residual entries.
