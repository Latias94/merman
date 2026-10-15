---
type: Verification Evidence
title: Theme retirement release checks at current HEAD
timestamp: 2026-09-15
related_plan: docs/plans/2026-08-09-001-portable-theme-architecture-convergence-addendum.md
git_branch: refactor/presentation-theme-model
git_commit: 9ab7e8e844c7bf9a1beaea1107987236be72928c
tags: theme,retirement,release,verification
---

# Scope

This record binds the release-mode theme retirement checks to the current source commit. It
covers the executable legacy projection boundaries and does not claim that C7a public discovery,
preset qualification, or the full cross-platform release matrix is complete.

# Checks

Command:

```text
CARGO_BUILD_JOBS=2 python3 scripts/run_theme_acceptance.py test -p merman-theme-acceptance \
  --release --locked --no-default-features --features png,layout-cytoscape \
  --test block_title_legacy_projection \
  --test class_edge_label_background_legacy_projection \
  --test flowchart_marker_legacy_projection \
  --test legacy_projection_retirement -- --test-threads 2
```

Results:

- `block_title_legacy_projection`: 3 passed, 0 failed, 0 ignored;
- `class_edge_label_background_legacy_projection`: 1 passed, 0 failed, 0 ignored;
- `flowchart_marker_legacy_projection`: 2 passed, 0 failed, 0 ignored;
- `legacy_projection_retirement`: 4 passed, 0 failed, 0 ignored.

The acceptance wrapper supplies the workspace-only `merman_internal_theme_acceptance` cfg; invoking
these integration targets directly without the wrapper is not equivalent. The run completed in
Release mode and produced no tracked working-tree changes.

# Interpretation

The current executable legacy projection inventory is closed for these retirement targets, including
Block, Class, and Flowchart marker boundaries. Independent historical retirement authority remains
compiled and verified. This evidence does not retire the historical ledger or promote public
qualification cells.
