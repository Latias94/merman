# ZenUML Minimum Slice (Headless Compatibility, Phase 1)

This document defines the initial, test-driven minimum slice for ZenUML support in `merman`.

ZenUML is an upstream Mermaid “external diagram” rendered via browser-only `@zenuml/core`. `merman`
is pure Rust and headless, so Phase 1 implements a conservative compatibility mode by translating a
small ZenUML subset into Mermaid `sequenceDiagram` syntax.

Baseline references:

- Mermaid: `@11.12.2` (`repo-ref/mermaid`, see `repo-ref/REPOS.lock.json`)
- ZenUML core: `v3.45.4` (`repo-ref/zenuml-core`, see `repo-ref/REPOS.lock.json`)

## Supported (current)

- Header:
  - `zenuml` (case-insensitive), optionally preceded by empty lines.
- Empty lines and whitespace-only lines are ignored.
- Metadata directives (passed through to the sequence parser):
  - `title ...`
  - `accTitle ...`
  - `accDescr ...`
- Messages (translated to Mermaid sequence arrows):
  - `A->B: message` → `A->>B: message`
  - `A-->B: message` → `A-->>B: message`
  - label is optional (`A->B` is allowed)

## Output shape (Phase 1)

- Diagram type: `zenuml` (metadata stays `zenuml` for detection/UX).
- Semantic model/layout/rendering: delegated to the `sequenceDiagram` pipeline after translation:
  - semantic parser: `crates/merman-core/src/diagrams/sequence.rs`
  - layout: `crates/merman-render/src/sequence.rs`
  - SVG: `crates/merman-render/src/svg/parity/sequence.rs`

## Not yet implemented (upstream-supported)

- Full ZenUML grammar (participants, blocks, activation, loops/alt/opt, notes, annotations, etc.).
- Upstream SVG parity-gating (ZenUML rendering is browser-only upstream).

## Alignment goal

Phase 1 is an incremental compatibility slice. The long-term goal is either:

1. A broader translation layer (still rendering via the existing Mermaid `sequenceDiagram` stack),
   or
2. A full headless ZenUML port (semantics + layout + rendering) behind an explicit feature flag.

