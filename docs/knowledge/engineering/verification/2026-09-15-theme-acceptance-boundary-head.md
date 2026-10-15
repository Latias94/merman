---
type: Verification Evidence
title: Theme acceptance public boundary at current HEAD
timestamp: 2026-09-15
git_branch: refactor/presentation-theme-model
git_commit: ad1900f83b138d6c04e7246cb97d4be6b8be7db8
tags: theme,api,boundary,verification
---

# Check

```text
python3 scripts/verify_theme_acceptance_boundary.py

merman: private feature and acceptance modules excluded
merman-render: private feature and acceptance modules excluded
merman-export: private feature and acceptance modules excluded
external consumer resolves production APIs with every producer feature enabled
external consumer cannot import all listed acceptance-only types
```

The external Cargo consumer compiled against the production API surface while imports of acceptance
features, route receipts, raster receipts, and retired HostTheme/PresentationProfile types failed as
expected. This confirms that workspace-only acceptance machinery remains outside the published Rust
surface at this HEAD.

# Boundary

This is an API-boundary check only. It does not qualify presets, publish artifacts, or close the C7a
rollout and contract-freeze gates.
