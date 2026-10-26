---
type: Work Progress
title: Capture canonical editor snapshots without presentation
description: Retain canonical results and requested recovery facts from one family construction.
timestamp: 2026-10-21T18:46:48Z
record_id: 8b0b69817d944a389168ab9f7c5abd89
producer_id: codex-root
run_id: semantic-capability-boundaries-u5
related_plan: docs/plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md
git_branch: refactor/semantic-capability-boundaries
git_commit: 7c6914251de521e29f55cbb5129e0269e5dee6ca
---

# Contract And Implementation

`SemanticParser` now exposes auto-detected and known-type snapshot capture, with
controlled variants. `SemanticSnapshotCapture` retains preprocessing/configuration
failures and panics before metadata exists. A `SemanticDiagramSnapshot` retains
canonical meaning or the original family error/panic, requested parser facts,
source-configuration evidence and incomplete-directive recovery state. Cooperative
cancellation remains the outer error channel, including after family construction.
Each operation uses the parser's runtime policy and the existing restoring context.

All 34 selected diagram families, including shared selectors and Railroad dialects,
thread private editor demand through the same successful grammar/construction.
`SemanticConstruction` moves the canonical result and optional facts together.
Strict semantic parsing requests Omit; it does not collect editor observations.
Capture does not use a JSON master, second recovery parser or presentation setup.

Family-specific recovery behavior remains local: State database failures retain
Complete facts while grammar failures recover; Sequence preserves its distinct
grammar/database diagnostics; Architecture preserves first syntax-error priority
and later symbols; C4/Timeline retain ordered multiple issues; TreeView preserves
its construction-failure diagnostic without a source span. Empty Pie/Radar/XYChart
capture retains the default model and an available empty journal. EventModeling
and Ishikawa move journals instead of cloning them.

Both full and canonical capture use `PreprocessedSource::finish_editor_semantic_facts`.
The existing remap/drop rules, global expected syntax, family semantics and directive
prefix ordering are unchanged. The full source-map tests retain their assertions.
The README documents the source-build API and independent feature recipe.

This unit enables editor capture without presentation; it does **not** complete
the separate no-editor compilation boundary. Editor definitions, family observers,
some failure fields, lexer hints and preprocessing/YAML evidence still need positive
`editor-facts` cfg isolation. Full legacy snapshot APIs remain available as before.

# Evidence And Review

The public Class test first failed to compile because the lean snapshot API was
absent (E0432/E0599); its log is `editor-facts-class-snapshot-red.log` in the local
`D:/merman-u5-temp/` evidence directory. Initial authoring mistakes are not counted
as semantic RED evidence. Sixteen family proofs and four public tests supplement
the existing proof suite without changing its assertions, stack sizes or budgets.
The existing formal fixture audit now compares canonical/full editor facts exactly,
using the same fixture configuration and fixed runtime policy.

A new State collision fixture initially used a bare identifier that failed in the
lexer. It was corrected to the existing quoted-alias database collision, with a
specific database-error assertion. The production Complete/Recovered contract was
not weakened. The preprocessing-failure fixture uses the existing directive nesting
budget rather than assuming that an ignored invalid directive is an error.

The five simplification lenses completed: reuse 0 findings; quality 2 applied
(direct combined-result construction and removal of a forwarding method); efficiency
1 finding across 2 locations applied (no empty failure-journal allocation for State
or Architecture Omit); tests 0; comments 1 removed; 0 skipped. Protected proof tests
were retained. Architecture reuses `SemanticConstruction<ArchitectureDb>` rather than
adding a generic failure framework. State boxes its failure journal only for Capture.
Canonical snapshot outcomes remain inline with narrowly reasoned large-enum lint
allowances, avoiding a new successful-capture Box owner.

Independent shared-API correctness and family-failure adversarial reviews both
returned no findings after inspecting actual callers and original implementations.
Reviewers were read-only; root owned integration, Cargo and Git. Their coverage is
this unit, not final U1-U9 acceptance or a completed no-editor compilation audit.

All final commands used the existing F-drive checkout/shared target, serial Cargo,
two build jobs and two nextest threads. Final results after simplification:

| Recipe | Result | Nextest run |
| --- | --- | --- |
| Core `--no-default-features --features all-diagrams,editor-facts --lib --test semantic_editor_snapshot` | 503 passed | `0601d849-e95e-42c4-8d17-b1e765bc177e` |
| Core `--all-features --lib --test parse_projection_contract --test deep_lifecycle --test editor_semantic_facts --test semantic_editor_snapshot` | 2,203 passed | `e1e23356-b45b-4e7c-a06e-5f0758454c0c` |
| Core `--no-default-features --features editor-facts --lib --test semantic_editor_snapshot` | 235 passed | `bc0ae813-9720-4572-ab27-1a6bba20f912` |

All selected tests passed, with zero skips. Production `cargo clippy --locked -p
merman-core --all-features --lib -- -D warnings`, `cargo fmt --all -- --check` and
`git diff --check` passed. Narrow-feature builds retain existing cfg-specific
warnings and are not claimed as strict lint passes. Logs are the local
`editor-facts-*-final-nextest.log`, `editor-facts-full-clippy-final.log` and
`editor-facts-fmt-final.log`. No timing, allocation-total or final-byte improvement
is claimed by this unit.

# Remote Integration And Continuity

Fetched origin and merged its three new documentation commits through
`71da4cf6fbbf4fcf27fb0465cc9169f6fd6ebd8b` in merge
`7c6914251de521e29f55cbb5129e0269e5dee6ca`. The delta covers release policy and
changelog/backport evidence only. It changes neither Rust/Cargo/runtime contracts
nor the approved refactor plan. Existing in-progress source changes were preserved.

Next: complete true `editor-facts` definition/field/callback/collector/lexer and
preprocessing isolation, then migrate actual analysis/editor/ASCII consumers.
U8 product/generated/legal work and U9 integrated correctness, stripped-byte and
removed-work evidence remain required. The goal stays active. No push, PR,
publication, release or new worktree occurred.
