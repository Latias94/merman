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
- `fixtures/zenuml/upstream_docs_zenuml_demo.mmd`
  - Source: `repo-ref/mermaid/docs/syntax/zenuml.md` (“Demo”).
- `fixtures/zenuml/upstream_docs_zenuml_participants_declare_optional.mmd`
  - Source: `repo-ref/mermaid/docs/syntax/zenuml.md` (“Participants / Declare participant (optional)”).
- `fixtures/zenuml/upstream_docs_zenuml_participants_annotators.mmd`
  - Source: `repo-ref/mermaid/docs/syntax/zenuml.md` (“Participants / Annotators”).
- `fixtures/zenuml/upstream_docs_zenuml_participants_aliases.mmd`
  - Source: `repo-ref/mermaid/docs/syntax/zenuml.md` (“Participants / Aliases”).
- `fixtures/zenuml/upstream_docs_zenuml_creation_new.mmd`
  - Source: `repo-ref/mermaid/docs/syntax/zenuml.md` (“Messages / Creation message”).
- `fixtures/zenuml/upstream_docs_zenuml_sync_message_method_calls.mmd`
  - Source: `repo-ref/mermaid/docs/syntax/zenuml.md` (“Messages / Sync message”).
- `fixtures/zenuml/upstream_docs_zenuml_nesting.mmd`
  - Source: `repo-ref/mermaid/docs/syntax/zenuml.md` (“Nesting”).
- `fixtures/zenuml/upstream_docs_zenuml_comments.mmd`
  - Source: `repo-ref/mermaid/docs/syntax/zenuml.md` (“Comments”).
- `fixtures/zenuml/upstream_docs_zenuml_loops_while.mmd`
  - Source: `repo-ref/mermaid/docs/syntax/zenuml.md` (“Loops / while”).
- `fixtures/zenuml/upstream_docs_zenuml_alt_if_else.mmd`
  - Source: `repo-ref/mermaid/docs/syntax/zenuml.md` (“Alt / if-else”).
- `fixtures/zenuml/upstream_docs_zenuml_opt.mmd`
  - Source: `repo-ref/mermaid/docs/syntax/zenuml.md` (“Opt”).
- `fixtures/zenuml/upstream_docs_zenuml_parallel_par.mmd`
  - Source: `repo-ref/mermaid/docs/syntax/zenuml.md` (“Parallel / par”).
- `fixtures/zenuml/upstream_docs_zenuml_try_catch_finally.mmd`
  - Source: `repo-ref/mermaid/docs/syntax/zenuml.md` (“Try/Catch/Finally”).
