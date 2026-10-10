---
type: Verification Evidence
title: Platform binding verification at current HEAD
timestamp: 2026-09-15
git_branch: refactor/presentation-theme-model
git_commit: db2ba4b51da5160044fbe4fccb5fbcab1bd65a5a
tags: theme,bindings,verification,release
---

# Check

```text
python3 scripts/verify-platform-bindings.py

Flutter Native Assets theme authoring passed: 2 materializations, 22 support queries,
5 errors, 3 budgeted operations per consumer
Android Rust transport target checks passed
Flutter/Dart package checks passed
Flutter Native Assets smoke passed
Platform binding verification completed.
```

The verifier exercised the current generated bindings and native transport contracts, including
Flutter authoring/support/resource vectors, Android Rust target checks, generated ABI freshness,
Dart analysis/format, and the installed Flutter smoke commands. It exited with status 0 and left no
tracked working-tree changes.

# Boundary

This host verification does not replace device execution on every Android ABI, browser matrix
qualification, or C7a public catalog promotion.
