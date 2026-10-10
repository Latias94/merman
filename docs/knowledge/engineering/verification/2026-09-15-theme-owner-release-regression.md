---
type: Verification Evidence
title: Complete theme owner Release regression at current HEAD
timestamp: 2026-09-15
git_branch: refactor/presentation-theme-model
git_commit: f9dbe0caee2ab25593d301f839b3f48f240917bd
tags: theme,release,regression,verification
---

# Scope

This record binds the complete theme owner regression to the current source revision after the
support-discovery workflow gate was added. It covers the Rust renderer, exporter, public facade,
and acceptance owner libraries; it is not a C7a contract-freeze or cross-host release claim.

# Check

```text
CARGO_BUILD_JOBS=2 python3 scripts/run_theme_acceptance.py nextest run --release --locked \\
  -p merman-theme-acceptance -p merman-render -p merman-export -p merman --lib --cargo-quiet

2942 tests run: 2942 passed, 2 skipped
```

The two skips are existing owner-test skips reported by nextest. No test failed. The run includes
route cutover, terminal evidence, resource admission, support discovery, authoring, export, and
acceptance proof paths selected by these four packages.

# Boundary

This is a clean current-worktree owner regression; it does not replace installed Web/Node,
Android, Python, Flutter, Typst, CLI archive, or browser matrix checks. Those remain separately
owned release gates.
