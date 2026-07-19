# ZenUML Upstream Coverage (Mermaid 11.16)

This file maps the local ZenUML corpus to the selected companion source. The selected graph is the
3.47.8 oracle; 3.50.1 is recorded separately until admission.

## Source mapping

| Local coverage | Oracle source | Gate |
| --- | --- | --- |
| `fixtures/zenuml/basic.mmd` | `src/g4/sequenceParser.g4` message/return rules | semantic + layout |
| participant annotators, aliases, colors | `src/parser/Participants.ts`, `src/parser/ToCollector.js` | participant topology |
| creation and assignments | `src/parser/Owner.js`, `src/parser/From.ts`, creation grammar | owner/return topology |
| nested calls and returns | `src/parser/Origin.js`, `src/svg/walkStatements.ts` | occurrence ownership |
| loops, alternatives, optional, parallel, critical, sections | `src/svg/buildFragmentGeometry.ts`, fragment grammar | fragment sections |
| try/catch/finally and references | `src/svg/walkStatements.ts`, `sequenceParser.g4` | fragment/ref topology |
| Unicode, emoji, strings, incomplete input | `sequenceLexer.g4`, parser recovery tests | exact spans/recovery |
| SVG participants/lifelines/messages | `src/svg/components/*.ts`, `src/svg/buildGeometry.ts` | structural SVG |

## Mermaid documentation fixtures

The following fixtures are retained as source-backed examples from
`repo-ref/mermaid/docs/syntax/zenuml.md`:

- `upstream_docs_zenuml_demo.mmd`
- `upstream_docs_zenuml_participants_declare_optional.mmd`
- `upstream_docs_zenuml_participants_annotators.mmd`
- `upstream_docs_zenuml_participants_aliases.mmd`
- `upstream_docs_zenuml_creation_new.mmd`
- `upstream_docs_zenuml_sync_message_method_calls.mmd`
- `upstream_docs_zenuml_nesting.mmd`
- `upstream_docs_zenuml_comments.mmd`
- `upstream_docs_zenuml_loops_while.mmd`
- `upstream_docs_zenuml_alt_if_else.mmd`
- `upstream_docs_zenuml_opt.mmd`
- `upstream_docs_zenuml_parallel_par.mmd`
- `upstream_docs_zenuml_try_catch_finally.mmd`
- `upstream_docs_zenuml_reply_assignments.mmd`
- `upstream_docs_zenuml_reply_return_keyword.mmd`
- `upstream_docs_zenuml_reply_annotator_return.mmd`

Each fixture has a semantic and layout artifact. Refresh artifacts only through the repository
snapshot command so generated provenance remains reproducible.

## Admission rule

Parser acceptance alone does not admit a version or claim browser parity. A companion update must
run the oracle/candidate corpus, compare semantic topology and source ranges, render structural
SVG, exercise invalid recovery and resource limits, and classify every delta from the pinned source.
The candidate may replace the oracle only when all required U1 gates pass.
