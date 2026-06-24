# ADR 0071: Editor-Facing Parser and Semantic Seam

- Status: accepted
- Date: 2026-06-24

## Context

Merman already uses a mixed parser portfolio across diagram families. Some families are parser
generator backed; others are hand-written because their syntax shape fits that better. That is a
reasonable implementation choice.

The problem now is not parser technology selection by itself. Editor-facing consumers need stable
spans, recoverable partial results, and semantic identity. The current fence-local structure scans
in `merman-lsp` are useful as a bootstrap, but they are not a product-grade contract.

## Decision

- Keep parser technology family-local.
- Define a span-rich semantic seam between family parsers and downstream consumers.
- Treat recoverable partial parsing as a first-class contract for editor inputs.
- Classify parser facts by projection role. Entity facts may feed completion, references, and
  outline surfaces; outline facts may feed document symbols and hover without becoming completion
  candidates; payload facts preserve source spans for lint and future semantic consumers without
  being projected into the LSP migration index.
- Route analysis, lint, and LSP features through semantic facts instead of raw-text structure scans.
- Keep render-model generation as a downstream projection, not the public parsing contract.

Temporary raw-text scans may remain during migration, but only as compatibility shims. They are not
the target architecture.

As of 2026-06-24, the migration shim is centralized as `merman-analysis::FenceTextIndex`. LSP
consumes that shared index and no longer owns separate completion, outline, navigation, and rename
scans. This is an intermediate consolidation step; family parsers still need to produce the
span-rich facts that will replace the shim.

`FenceTextIndex` must respect semantic roles when it projects parser facts. It should not treat
every parser-produced span as a graph-node completion id. For example, ER attribute names can be
outline facts, while attribute types, keys, and comments can be payload facts for lint without
polluting node-id completion.

## Consequences

- `merman-core` gets a stronger contract for semantic facts and source locations.
- `merman-analysis` and `merman-lsp` can stop rediscovering structure from raw text on the
  supported families.
- Parser refactors become local to each family instead of forcing a repository-wide parser-generator
  rewrite.
- Recovery and span coverage become part of the test surface, which raises the parser maintenance
  bar.

## Alternatives Considered

### Single parser-generator rewrite

Rejected. A global migration would spend a lot of effort without solving recovery or semantic
indexing by itself.

### Continue heuristic downstream scans

Rejected. Heuristic scans are brittle for incomplete buffers and weaken locality for parser bugs.

### Separate editor-only parser stack

Rejected. That would duplicate behavior and split the public parsing surface.
