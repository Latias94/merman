---
type: Verification Evidence
title: Theme acceptance integration target coverage
timestamp: 2026-09-15
related_plan: docs/plans/2026-08-09-001-portable-theme-architecture-convergence-addendum.md
git_branch: refactor/presentation-theme-model
git_commit: 3b0eab876
tags: theme,ci,acceptance,verification
---

# Result

Every integration target under `crates/merman-theme-acceptance/tests` is explicitly selected by
at least one owner workflow, with the required internal acceptance configuration and PNG feature
for visual retirement targets:

- `legacy_projection_retirement`
- `block_title_legacy_projection`
- `class_edge_label_background_legacy_projection`
- `flowchart_marker_legacy_projection`
- `c6_runtime`
- `preset_qualification`
- `route_cutover_runtime`
- `native_export_smoke`

The Flowchart marker target was added to both CI and Release Preflight by `5fe83ef17` after the
coverage audit found it was present but not selected. The renderer-owned `theme_support_discovery_test`
(49 tests) is now also explicitly run in both workflows by `3b0eab876`, so support classification
does not depend on the Linux full-workspace test alone.

# Validation

```text
python3 -m unittest scripts.test_release_workflow_security scripts.test_ci_plan
Ran 70 tests — OK

actionlint .github/workflows/ci.yml .github/workflows/release-preflight.yml
pass

python3 scripts/run_theme_acceptance.py nextest run --release --locked \
  -p merman-theme-acceptance --no-default-features \
  --features png,layout-cytoscape \
  --test flowchart_marker_legacy_projection --cargo-quiet
2 passed, 0 skipped
```

# Boundary

This proves workflow selection and the marker retirement owner test on the local host. It does not
claim all integration targets ran in this local command, Android emulator execution, or C7a
contract freeze.
