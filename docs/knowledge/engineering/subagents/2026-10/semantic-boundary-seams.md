---
type: "Subagent Finding"
title: "Semantic boundary implementation seams"
description: "Subagent Finding for Semantic boundary implementation seams."
timestamp: 2026-10-20T10:02:20Z
record_id: "01f9e1f02414421a8489565b8149bfb5"
producer_id: "codex-frankorz"
run_id: "semantic-capabilities-20261020"
related_plan: "docs/plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md"
git_branch: "refactor/semantic-capability-boundaries"
git_commit: "e8a205267"
---

# Finding

Read-only mapping for approved U3–U6 after PR #225. These are source observations and
preservation obligations, not completed implementation or benchmark results.

# Evidence

Ordinary JSON and typed parsing are already separate routes. Wasted editor work occurs
inside shared family construction:

| Family | Owner | Eager work |
| --- | --- | --- |
| Sequence | `diagrams/sequence/parse.rs` | `into_editor_facts_and_actions` always invokes the collector |
| State | `diagrams/state/parse.rs` | `into_editor_facts_and_document` creates editor events, collections and symbols |
| Class | `diagrams/class/parse.rs` | shared lexical tape always enters its editor collector |
| ER | `diagrams/er.rs` | `ErSyntax::into_editor_facts_and_actions` always invokes `ErEditorFactCollector` |
| Pie | `diagrams/pie.rs` | statement loop appends common, section and recovery facts |

The first four ordinary `into_parse_error` routes also materialize recovery facts before
discarding them. Keep grammar event tapes, action arenas, DB validation, warnings and
resource charges. Pie negative-value validation, canonical deduplication, common diagnostics
and `first_error` remain semantic work. Many other families remain eager; five-family U3
evidence is not all-family completion. Flowchart/Swimlane ordinary parsing already avoids
facts. Architecture typed parsing avoids them, while its JSON route still projects them.

Diagnostics migration has two hidden dependencies: `EditorParseSourceMap::remap_facts`
can append recovery diagnostics for spans crossing preprocessing edits, and
`flowchart_parse_diagnostic` uses `expected_syntax` to refine EOF insertion spans.

`StateDb::into_model_controlled` consumes the DB but clones records, notes, relations,
styles, links, direction and accessibility strings. Finish `build_layout_data_typed` first,
then destructure and move. Preserve state order, class insertion order, duplicate link
order, `Links::One/Many`, and root-only first-document-wins lookup. Borrowing that lookup's
keys can remove copies; destination maps/vectors remain necessary allocations.

`build_layout_data_typed` constructs required semantic nodes/edges, note groups, parent/color
facts, generated-id collision checks and group-description validation. It is not coordinate
layout. Preserve identity normalization and display sanitization at their current semantic
points. Typed State owns one flat document; explicit JSON projection and complexity accounting
must retain repeated per-state documents, foreign-id checks, cancellation and iterative cleanup.

# Recommendation

Private closed projection demand belongs to the operation owner. Branch before collectors
allocate; making append methods no-ops still constructs their arguments. Keep one grammar
construction and count actual collector invocation/items, with assertions on captured meaning.

The shared core boundary should be raw configuration/runtime inputs and a preprocessed source
handoff, rather than complete `Engine`/`ParseMetadata`. Semantic metadata must not pretend to
contain materialized effective theme configuration. Full presentation owns initialization,
frozen binding/paint evidence, legacy registry visibility and canonical-model sidecars.

Gate definitions, fields, callback types/macros and dependency edges as well as calls. Keep
generic iterative JSON ownership primitives, cheap appearance selection and source CSS/security
admission shared. Full operation ordering remains lazy initialized-result read after preprocess
errors, source filtering, detector checkpoint, rematerialization/replay, authored appearance
reapply, layout selection, paint freeze and binding freeze/seal. Preserve PR #225 cached errors,
pristine override/provenance checks and re-stamped assignment epochs; final JSON equality alone
does not preserve same-value ownership mutations.

Full custom precedence remains custom render, custom semantic JSON fallback, builtin typed,
then missing-typed/unsupported errors. Preserve overlay provenance and initialized detector
visibility. Lean parsing must not install or silently fall back to legacy adapters.

# Disposition

Implementation remains pending. Finish U2 and freeze clean adjacent evidence before U3,
then State transfer and the larger capability split. The approved plan remains authoritative.
Scout agents ran no builds or tests and made no implementation/performance claims.

# Citations

- `docs/plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md`
- `crates/merman-core/src/lib.rs`, `parse_pipeline.rs`, `family.rs`, `diagram/mod.rs`, `config/mod.rs`
- `crates/merman-core/src/diagrams/sequence/parse.rs`, `state/parse.rs`, `class/parse.rs`, `er.rs`, `pie.rs`
- `crates/merman-core/src/diagrams/state/db.rs`, `state/render_model.rs`
- `docs/knowledge/engineering/sessions/2026-10/semantic-boundary-u7.md`
