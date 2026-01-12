# ADR 0032: GitGraph Parser Technology (Mermaid@11.12.2 parity)

- Status: Accepted
- Date: 2026-01-12
- Baseline: Mermaid `@11.12.2`

## Context

Mermaid’s `gitGraph` diagram uses:

- The `@mermaid-js/parser` Langium-based grammar to produce a typed AST
  (`packages/mermaid/src/diagrams/git/gitGraphParser.ts`), and
- A DB/state machine (`packages/mermaid/src/diagrams/git/gitGraphAst.ts`) that:
  - maintains commit/branch state (`head`, `currBranch`, `branches`, `commits`)
  - normalizes/sanitizes ids and labels at DB-time
  - emits warnings for duplicate commit ids
  - enforces operation constraints via user-visible error messages

`merman` must be headless and pure Rust, while preserving Mermaid’s observable behavior.

## Decision

Implement `gitGraph` parsing in `merman-core` as a line-oriented parser plus a DB-like post-pass
state machine:

- Parse the header (`gitGraph`, optional direction `LR|TB|BT`, optional `:`).
- Parse subsequent statements per line:
  - `commit`, `branch`, `checkout`/`switch`, `merge`, `cherry-pick`
  - key/value arguments with flexible ordering (`id`, `msg`, `tag`, `type`, `order`, `parent`)
- Apply DB logic that mirrors Mermaid’s `gitGraphAst.ts`:
  - id generation, parent linking, branch switching
  - merge commit creation with two parents
  - cherry-pick constraints for merge commits (`parent` required and validated)
  - warnings and error messages aligned with upstream tests

## Rationale

- `gitGraph` is inherently sequential/stateful; a DB-like state machine is the clearest parity
  target.
- The syntax is mostly line-based; a dedicated parser avoids introducing a second heavyweight
  parsing framework for a relatively small surface area.
- Parity is enforced primarily by porting Mermaid’s upstream tests (`gitGraph.spec.ts`).

## Consequences

- The Rust implementation must explicitly encode the state transitions that Mermaid performs in its
  DB, including subtle error surfaces.
- Any future changes in Mermaid’s Langium grammar or DB semantics should be reflected by expanding
  the ported test suite and adjusting the parser/DB accordingly.

## Revisit criteria

Reconsider adopting a shared grammar toolchain for `gitGraph` if Mermaid significantly expands the
syntax surface, or if we need a unified typed AST representation across multiple diagrams.

