# ZenUML Upstream Coverage (Mermaid@11.12.2)

This document tracks which upstream ZenUML examples/tests are covered in `merman`.

ZenUML is an external diagram upstream and is rendered via browser-only `@zenuml/core`, so `merman`
does **not** maintain upstream SVG baselines for ZenUML. Coverage is snapshot-only:

- semantic snapshots under `fixtures/zenuml/*.golden.json`
- layout snapshots under `fixtures/zenuml/*.layout.golden.json`

Pinned baseline version: Mermaid `@11.12.2` (see `repo-ref/REPOS.lock.json`).

Pinned ZenUML implementation reference: `repo-ref/zenuml-core` (see `repo-ref/REPOS.lock.json`).

## Mermaid syntax docs

Source: `repo-ref/mermaid/docs/syntax/zenuml.md`

Status: not fully imported yet. Phase-1 fixture coverage is intentionally small until the
translator is expanded.

Planned import strategy:

- Import small, reviewable batches of examples from the docs file.
- Each fixture name should encode the upstream section/topic and remain stable.
- Extend the translator (`crates/merman-core/src/diagrams/zenuml.rs`) only when a new fixture is
added, so regressions remain attributable.

## Current fixtures

- `fixtures/zenuml/basic.mmd`
  - Scope: message arrows + titles/accessibility pass-through.
  - Gate: semantic snapshot + layout snapshot.

