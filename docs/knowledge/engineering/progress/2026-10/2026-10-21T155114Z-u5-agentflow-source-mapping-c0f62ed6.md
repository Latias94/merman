---
type: Work Progress
title: Agentflow canonical ownership and shared source mapping
description: Separate authored Agentflow facts from computed DOM state and preserve original source positions in lean parsing.
timestamp: 2026-10-21T15:51:14Z
record_id: c0f62ed6f8154ff89cab071244457cb3
producer_id: codex-root
run_id: semantic-capability-boundaries-u5
related_plan: docs/plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md
git_branch: refactor/semantic-capability-boundaries
git_commit: 6bc29b741ed1dfd9f9b0e89aaaed87a4ec03f468
---

# Ownership

Agentflow now has one `AgentflowDiagramModel` containing graph meaning, authored
styles, labels, security-normalized links/callbacks, warnings and diagnostics.
`AgentflowDiagramRenderModel` pairs it with computed DOM ordinals. The full model
does not clone the canonical graph. Borrowed serializers preserve the published
full field shape and order; compatibility JSON projects directly from canonical
facts without serializing and discarding the editor or authored-style owner.

Ordinary parsing requests `EditorFactsDemand::Omit`; combined editor requests
capture facts while using the same construction. The existing 54 Agentflow tests
retain their assertions. Four new family tests cover authored ownership, security
and limits, the full wire round trip, and requested editor capture. Together with
the previously migrated families, 32 of 34 diagram selectors now have canonical
callbacks. Class and GitGraph are not part of this completed slice.

# Source Position Repairs

The lean facade previously failed to map structured parse errors and warning
spans back to the original input. New tests observed incorrect frontmatter slices
before the fix: nextest `4189b186-cb0d-41e8-93d2-a0e87a925d7c` failed both source
span assertions. Move the existing full-path remapping operations into their
shared `PreprocessedSource` owner, then reuse them from both facades. Preserve
the special warning insertion fix at the remapped span end.

An independent correctness review also found that lean Agentflow selected the
comment-stripped buffer. Two additional tests failed before the correction:
nextest `a1cce031-0ba1-435d-9466-302e25ca9cf8` dropped a quoted `%%` line and
reported a diagnostic on line 5 instead of line 6. Both facades now use one
source-selection helper. It preserves the existing Agentflow/EventModeling
predicate, effective `maxTextSize`, UTF-16 bound and one-time entity preparation.
The selected buffer retains its original edit map, including BOM and CRLF edits.
Independent re-review found the reported issue resolved.

# Verification

All Cargo runs were serialized in the existing F-drive checkout and target, with
two build jobs and two nextest threads. No worktree or build directory was added.

| Selection | Result | Nextest run ID |
| --- | --- | --- |
| Agentflow lean core library | 255 passed | `824f539c-6802-4781-b408-33529d923cd8` |
| Full core library, Agentflow positions and projection contracts | 2,133 passed | `2c237573-9683-408c-9442-055b958be7ac` |
| Canonical union excluding only Class and GitGraph | 460 passed | `e218b006-6621-4343-8427-f7ecae2cdb36` |
| Agentflow-only renderer library and Agentflow SVG | 1,592 passed, 1 skipped | `3e7a17e9-1801-405a-99b8-9a67b4bb620a` |
| Agentflow/Flowchart/Swimlane renderer library and both SVG targets | 1,957 passed, 1 skipped | `04bf43b7-c0d2-46c2-9fa8-d1b15c494eb9` |

Production full-core Clippy with `-D warnings` passed. Rust formatting and
whitespace checks are required before this slice's commit. Results refer to this
slice; the later GitGraph oracle proof tests are excluded from its commit.

Agentflow-only renderer library compilation initially exposed 22 pre-existing
test feature errors: shared Flowchart implementation modules compiled tests that
required the disabled Flowchart parser. Gate only those parser-dependent test
helpers/modules with `diagram-flowchart`; retain the pure geometry, budgets,
cancellation and deep traversal tests. The isolated all-library run above proves
the retained tests still execute. A first mixed test recipe omitted Swimlane and
failed 12 Swimlane fixtures (`44296b3b-a6da-4b0f-a3ba-06c1c9cb696d`); the corrected
three-family recipe above passed without changing their assertions.

The five simplify lenses completed: reuse, quality, efficiency and test pruning
reported no actionable findings; comment pruning removed two private module
introductory lines. Preserve the public API documentation and the non-obvious
compatibility JSON ordering comment. A cosmetic private filename rename and
generic serialization/projection frameworks were not introduced. Proof tests
were not weakened or deleted.

# Integration And Remaining Work

A fresh `git fetch origin` left `origin/main` at
`3119c614bf8c43d49cfe17ded0a53ec2e7fa983c`, already a parent of the current
`6bc29b741` merge commit. Remote main has no additional commits to merge. New
maintenance branches do not change this refactor's plan or its current base.

The four feature names were reviewed against their actual owners. `presentation`
accurately names full Mermaid visual preparation without an output backend.
`compat-json` would be clearer as `mermaid-json`. `operation-deadlines` currently
selects the Web monotonic-clock adaptation rather than disabling native deadlines;
`web-clock` is a clearer candidate. `editor-facts` has useful intended ownership,
but its compile-time isolation is unfinished: production types and combined
parser registration still follow older dependencies. Runtime Omit/Capture
evidence is not proof of compile-time omission. Handle the names and real
consumer/dependency migration together; this slice does not rename the features.

U5 still needs Class, GitGraph and actual editor isolation. U6-U9 retain their
consumer, package, strict parity, comparable artifact-size and final-review
requirements. Check Agentflow plus `layout-elk` independently in the feature
matrix; this receipt does not claim that selection or Linux-specific validation.
No final size, allocation or speed improvement is claimed here. No push, PR,
publication or release was performed, and the overall goal remains active.
