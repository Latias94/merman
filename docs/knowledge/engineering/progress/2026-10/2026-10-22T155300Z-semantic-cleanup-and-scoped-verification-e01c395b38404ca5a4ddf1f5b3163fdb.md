---
type: "Work Progress"
title: "Semantic cleanup and scoped verification"
description: "Canonical Sequence naming and package guidance are repaired; lean negative imports pass, while strict stopped on stale WASM provenance."
timestamp: 2026-10-22T15:53:00Z
record_id: "e01c395b38404ca5a4ddf1f5b3163fdb"
producer_id: "codex-root"
run_id: "semantic-capability-boundaries-u9"
related_plan: "docs/plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md"
git_branch: "refactor/semantic-capability-boundaries"
git_commit: "f78b854d87078534505d9f039c2f22be2ee2223f"
supersedes: "34028a242a204d90b4686b45e5dbd3a8"
---

# Verified State

The objective remains the complete approved U1-U9 plan, without push, PR, merge or release.
Independent completion review found and prompted removal of the explicitly transitional
`SequenceDiagramRenderModel` alias. All tracked Rust consumers now name `SequenceDiagramModel`.
The package-surface guide now records Web API 8 and math-free generic Web products.
The existing verification owner now runs isolated lean core doctests rather than relying on
workspace feature union to prove definitions are unavailable.

Fresh checks of this cleanup:

- Isolated three-family core doctest: 8/8 passed, including capability-negative imports.
- Sequence-related core/render/ASCII nextest: 623/623 passed; 8,427 unrelated tests filtered.
- Affected core/render/ASCII/xtask all-target/all-feature Clippy with warnings denied: passed.
- VS Code extension build and tests: 255/255 passed.
- Format and diff whitespace checked; the pre-existing untracked fixture remains untouched.

# Integrated Run Boundary

The external strict run ended with exit 1, not a complete pass. Its log is
`target/semantic-integrated-strict-external.log`.

It passed all 661 feature-matrix builds, 27 exact artifact recipe checks, generated/legal gates,
13,307 workspace nextest tests, doctests, SVG DOM modes, Web builds/tests/smoke and the
79-test non-Chromium browser suite. The Chromium suite ended at 187 passed, one failed and
nine not run. Its mounted-diagram beforeAll rejected stale WASM input provenance because the
agent changed Sequence source names while the browser run was still executing. This is an
invalidated artifact check, not a demonstrated renderer behavior regression. Rebuild the
affected Web artifacts and rerun the complete mounted-diagram spec; preserve the original
failed log. VS Code was not reached inside strict, but passed separately afterward.

The old post-strict and host waiters (PIDs 59916/54688; sessions 17947/96918) were verified
to have no active build children and stopped before source cleanup. Their queued parser,
CLI, size and host checks therefore did not execute. Do not assume those queues remain active.

# Evidence And Execution

New allocation registration and a simple serial wrapper around the existing memory driver live at
`F:/codex-builds/merman-semantic-capability-boundaries/evidence/renewed-20261022/`.
This registration retains the original caps, seven lanes, six scales, five repeats,
generic_v2 timing protocol and max32 budget. No fresh allocation or timing sample has run yet.
Historical raw evidence deleted by cargo clean remains unavailable, not reconstructed.

Clean historical checkouts were prepared under
`F:/codex-builds/merman-semantic-capability-boundaries/worktrees/` for U3 allocation head,
U4 baseline, U3 timing head and integrated f78b854d8. Existing checkout pointers were not moved.
Two additional detached timing-control checkouts contain uncommitted benchmark-only adapters;
their common pipeline aliases the historical/full engine API through a tiny per-side support
module. They are not yet measured or admitted and need alignment to the cleanup commit.

The maintainer questioned xtask overvalidation. The verified strict distribution was 512 diagram
singletons and 108 capability-leaf checks out of 661 build cases; exact identical command keys
were already deduplicated. Do not respond by weakening correctness. Prefer scoped final cleanup
checks and preserve unaffected completed evidence rather than restarting the entire strict matrix.
No broad xtask redesign is part of this cleanup.

# Next Action

Commit the verified cleanup, pin that source for final size/host evidence, rebuild Web output
and rerun mounted-diagram tests. Then execute the remaining parser differential, CLI process,
size, installed-host and adjacent evidence gates serially. Keep failed, inconclusive and
platform-deferred results explicit. The goal is not complete.
