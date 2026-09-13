---
type: Verification Evidence
title: Installed Node N-API theme authoring at 9d7b356b0
timestamp: 2026-09-13
related_plan: docs/plans/2026-08-09-001-portable-theme-architecture-convergence-addendum.md
git_branch: refactor/presentation-theme-model
git_commit: 9d7b356b08f6363b417064fbc06e625cc361c1e0
tags: theme,node,artifacts,verification
---

# Scope

The macOS ARM64 N-API candidate was rebuilt from `9d7b356b08f6363b417064fbc06e625cc361c1e0`
with `CARGO_BUILD_JOBS=2`. The primary worktree had no tracked modifications; existing
untracked maintainer knowledge directories were preserved. This is an installed-package
observation, not a clean-checkout qualification or C7a completion claim.

The loader and platform package were assembled, verified, packed with `npm pack`, and
installed together using `npm install --ignore-scripts --no-audit --no-fund` into a new
temporary project. The public ESM entrypoint resolved from that project executed the
production native addon. No test transport was substituted.

# Observations

`smoke-installed-package.mjs` passed for `@mermanjs/node` version `0.8.0-alpha.6`,
target `darwin-arm64`. Its theme authoring result contained:

```json
{
  "schema_version": 1,
  "shared_vectors": 2,
  "family_isolation_checks": 3,
  "rule_override_checks": 1,
  "cold_spec_checks": 1,
  "preset_export_checks": 3,
  "support_queries": 18,
  "authoring_diagnostics": 6,
  "resource_limit_checks": 2,
  "json_operations": 23,
  "svg_renders": 23
}
```

The 18 support queries compare both synchronous and asynchronous responses with the nine
shared support vectors, including claim revision 81. Diagnostic checks compare the shared
error envelopes. The SVG count excludes the initial basic rendering smoke.

| Packed artifact | Bytes | SHA-256 |
| --- | --- | --- |
| `mermanjs-node-0.8.0-alpha.6.tgz` | 112726 | `4acf37960679c5965aaf329a3292ed708ef836a198741477ca8559e8bea84fc1` |
| `mermanjs-node-darwin-arm64-0.8.0-alpha.6.tgz` | 10528943 | `066fce1cbdd6b3ebcc999f1a5c01352d835a7b6ab86441c18f3246707d3c3e50` |

# Reproduction

```text
CARGO_BUILD_JOBS=2 node platforms/node/scripts/build-candidate.mjs --candidate napi --target darwin-arm64
node platforms/node/scripts/assemble-packages.mjs --target darwin-arm64 --output-root <packages>
node platforms/node/scripts/verify-packages.mjs --packed-root <packages>
npm pack <packages>/node --json --pack-destination <tarballs>
npm pack <packages>/darwin-arm64 --json --pack-destination <tarballs>
```

Create a new consumer project, install both local tarballs, then invoke the repository's
installed-package runner:

```text
node platforms/node/scripts/smoke-installed-package.mjs --project <consumer> --version 0.8.0-alpha.6 --target darwin-arm64
```

Running `node --test platforms/node/scripts/theme-authoring-smoke.mjs` only loads an
exported helper module. It does not call `runThemeAuthoringSmoke`, import an installed
Merman package, or exercise its transport. Earlier conversational claims based on that
command are superseded by this actual installed N-API observation. This record does not
establish a current Node-WASM installation result.

These results do not qualify profiles, populate public catalog cells, retire family bridges,
or close the separate WASM artifact-size gate recorded in the September 12 artifact report.
