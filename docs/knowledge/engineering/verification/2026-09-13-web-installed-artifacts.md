---
type: Verification Evidence
title: Web WASM artifact rebuild and package smoke for theme revision 82
timestamp: 2026-09-13
related_plan: docs/plans/2026-08-09-001-portable-theme-architecture-convergence-addendum.md
git_branch: refactor/presentation-theme-model
tags: theme,web,wasm,artifact,verification
---

# Scope

The five Web WASM artifacts were rebuilt from the current source after the support manifest
revision advanced to 82. The earlier smoke correctly rejected stale `full`, `analysis`, `editor`,
and `ascii` artifacts whose `metadata.rs` input digest no longer matched; rebuilding all package
profiles was required. No package was published.

# Build and package results

`npm run build:wasm` completed all five profile builds with wasm-pack and wasm-opt, and each
transaction published a fresh package artifact. `npm run build:packages` then assembled the
public package directories and provenance records. The package group contains:

- `@mermanjs/web`
- `@mermanjs/web-analysis`
- `@mermanjs/web-render`
- `@mermanjs/web-editor`
- `@mermanjs/web-ascii`

`npm run verify:packages` passed the package-group prepack checks. Every package directory contains
an artifact provenance record; generated TypeScript entries and runtime contract data agree with
the package descriptors.

# Consumer observations

The release-style `npm run smoke` command passed all five packages. The smoke loaded the actual
assembled package entries, validated runtime catalogs, rendered the supported diagram corpus, and
checked capability/output declarations:

| Package | Diagrams | Capabilities | Outputs |
| --- | ---: | --- | --- |
| web | 35 | analysis, ascii, editor, layout-cytoscape, layout-elk, math, svg | ascii, svg |
| web-analysis | 35 | analysis | none |
| web-render | 35 | layout-cytoscape, layout-elk, math, svg | svg |
| web-editor | 35 | analysis, editor | none |
| web-ascii | 35 | ascii | ascii |

The DOM safety smoke also passed. The ordinary Web contract suite remains green at 142/142,
including rejection of NaN and infinities before JSON serialization. TypeScript build and contract
checks previously reported 38 WASM exports, 48 runtime bindings, and 5 package entries.

The shared support vectors include the Class `EdgeLabelBackground.fill` Unsupported response at
revision 82. This run proves the rebuilt package resources and public package entries; it does not
replace browser qualification or artifact size-budget review.

# Reproduction

```text
npm run build:wasm --prefix platforms/web
npm run build:packages --prefix platforms/web
npm run smoke --prefix platforms/web
npm test --prefix platforms/web
```

Logs: `/tmp/theme-web-wasm-all-20260913.log`, `/tmp/theme-web-packages-20260913-final.log`,
`/tmp/theme-web-smoke-20260913-final.log`, and `/tmp/theme-web-contracts-20260913.log`.

Package outputs under `platforms/web/packages` are generated/ignored build material and were not
staged. Browser suites, Windows/Linux package execution, final size budget checks, and C7a contract
freeze remain open.

# Release profile gates

After the rebuild, the representative artifact dependency closure verifier passed every selected
profile, including the five Web profiles; the feature matrix verifier validated 34 exact artifact
profiles, 15 packages, 110 capability leaves, 17 feature allowlists, 70 forwarding edges, and 14
transport engines. These checks used the current source and lockfile and found no dependency
closure drift.

Logs: `/tmp/theme-artifact-closures-20260913.log` and `/tmp/theme-feature-matrix-20260913.log`.
