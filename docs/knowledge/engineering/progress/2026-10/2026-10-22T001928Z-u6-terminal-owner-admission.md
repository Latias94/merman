---
type: Work Progress
title: Share terminal family owners and admit Class topology construction
description: Move remaining terminal backing knowledge into core families and expose incremental Class topology admission without claiming the public ASCII boundary is finished.
timestamp: 2026-10-22T00:19:28Z
record_id: b5b6a143-f47a-4f65-8c66-3e923a26cf02
producer_id: codex-root
run_id: semantic-capability-boundaries-u6
related_plan: docs/plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md
git_branch: refactor/semantic-capability-boundaries
git_commit: 774b24b2d
---

# Remote Synchronization

Fetched `origin` again on 2026-10-22 and checked ancestry before merging. Remote
`main` remains `71da4cf6fbbf4fcf27fb0465cc9169f6fd6ebd8b`, already an ancestor of
`774b24b2d`. The branch is 58 commits ahead and zero behind; `merge origin/main`
returned `Already up to date.` No source, local changes, or branch was discarded.
There is no additional upstream delta requiring a plan change.

# Family Ownership

Journey, Timeline, and Packet full carriers now own one public `semantic` payload
plus their existing private compatibility-output state. Borrowed and mutable
field access uses Deref; flat serde output and empty compatibility sentinels
remain intact. Journey and Timeline full wire input still defaults omitted
collections, while their canonical input still requires those fields. Packet
requires its row collection in both forms. Focused wire tests cover that
distinction instead of broadening canonical acceptance.

Gantt, Kanban, and Mindmap expose borrowed family terminal views. Presentation-only
constructors preserve detached full-model fields; ASCII adapters import base
views or canonical types. Gantt retains explicit full visible-end overrides.
Kanban canonical nodes expose their original authored record, including unknown
metadata. Group indices store source positions rather than larger node or task
views. Mindmap callbacks keep checkpoints and work charges before canonical
node scans, including empty child lists, and before edge visits. Full flat edges
do not acquire an extra canonical node scan. No whole-model conversion or second
family parse is introduced.

Moving an owned field out of the three full carriers now requires explicit
access through `semantic`; exhaustive full-carrier literals also change. U8
must document this Rust source migration. These changes do not claim Rust source
compatibility merely because existing wire shapes are retained.

# Class Admission

Implemented in the focused local commit `c64e8abfc`.

`ClassDiagramModel::topology_with_admission` uses the same constructor as ordinary
and full parsing. The callback admits requested container slots, actual ancestor
and collision steps, and copied or generated UTF-8 bytes before that work occurs.
Namespace member vectors admit geometric growth rather than reserving for each
individual member. Zero-unit entry and exit calls permit a caller-owned resource
context to replay its first terminal error. Units describe requested storage and
construction work, not allocator bytes or RSS. Six focused core tests cover
reservation rejection, exact and one-less budgets, traversal, collisions, text
length, and cancellation.

The actual public canonical ASCII dispatch is still pending. This constructor
API alone does not prove that topology admission is wired into every output
operation. The eventual caller must build topology once, before layout candidate
selection, and keep it alive through primary output and complete fallback using
the same operation control and render-wide resource context.

# Review And Verification

Independent read-only reviews of Class admission, Mindmap extraction, and the
other five family adaptations report zero reproducible findings. Reviews compare
the pre-change implementation at `774b24b2d`; they do not substitute for execution.
The Class ASCII tests additionally bind the constructor to a real controlled
`ResourceContext`: exact work succeeds, one less rejects, and a pre-existing
resource terminal survives later cancellation without charging more work. The
six-fixture, two-charset output comparison now constructs canonical topology
through that callback. Its later renderer uses a separate test ledger; this is
not evidence of shared whole-render/fallback accounting or the public dispatch.

| Selection | Result | Nextest run ID |
| --- | --- | --- |
| core and ASCII, all features, lib and tests | 3,702 passed, zero skipped | `f21d742c-231b-4da1-9291-3bdb0fcec93f` |
| Class ASCII admission and full/canonical output after test additions | Three passed; 2,942 unrelated library tests intentionally filtered | `139dc8ec-266c-429c-97d3-987228638312` |
| core, only the seven changed family selectors, no presentation, compatibility JSON, or editor facts | 275 passed, zero skipped | `0efec41e-2074-436a-a0df-6aee0b153117` |
| Thirteen Class/Gantt/Journey/Kanban/Mindmap/Packet/Timeline SVG and layout binaries, including ELK | 377 passed, zero skipped | `5519f6db-210f-47c0-b242-24be5f6c8187` |

The initial all-feature run precedes two test additions and the updated Class
output test; their three focused cases are covered by the following run. No
production changes follow the full run. Lean core emits fifteen production
warnings and twenty-two unit-test warnings, including twelve duplicates; it is
not a strict-lint claim. The standalone all-family/ELK renderer selection, with
math and editor facts disabled, emits eighty core warnings and twenty-one
renderer warnings; it passes tests but is also not a strict-lint claim. Strict
all-target Clippy for full-feature core and ASCII passes with
`--keep-going -- -D warnings`. No broad lint allowance or ignored test is added.
Package formatting, `git diff --check`, and the scoped progress-memory validator
pass. The memory validator reports existing stale-rollup and unregistered
external-workstream advisories; no unrelated memory migration is performed.
Execution logs are retained under `D:/merman-u5-temp/u6-family-terminal-owners-*`
and `D:/merman-u5-temp/u6-class-terminal-admission-nextest.log`.

Root runs Cargo serially, with two build jobs,
incremental compilation disabled, and only the shared
`D:/merman-u5-temp/verification-target`. Source and Git remain in the F drive
checkout; no new worktree or C drive target is created.

# Remaining Integration

The ASCII normal manifest still enables presentation and compatibility JSON;
the public entry still takes the full enum, and most complete fallback paths
still use legacy JSON projections. Next implement a consumer-owned borrowed
family union and borrowed source metadata, with family-owned complete streaming
fallback serializers. Preserve authored constraints, unknown metadata, ordering,
terminal normalization, cancellation, and bounded intermediate admission. Do not
fabricate presentation/theme evidence or replace complete fallback with a
partial primary-text reflow.

A reduced primary view is not a complete fallback record. In particular, State
terminal nodes and edges do not carry all authored document and style evidence.
Keep a family-owned projection borrowed from the same original owner instead of
reconstructing missing authored fields from primary layout inputs. Reuse the
bounded serializer and flattened-document traversal already present in core;
do not introduce another canonical master model or a JSON value-tree bridge.

Canonical facade/prepared artifacts, explicit full host adapters, U8 naming and
package alignment, and U9 final correctness, ABI, allocation, and matched stripped
size evidence remain required. The full U1-U9 goal stays active. Nothing is
pushed or published.
