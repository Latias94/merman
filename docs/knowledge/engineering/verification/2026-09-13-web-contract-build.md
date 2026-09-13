---
type: Verification Evidence
title: Web TypeScript and package contract verification for theme revision 82
timestamp: 2026-09-13
related_plan: docs/plans/2026-08-09-001-portable-theme-architecture-convergence-addendum.md
git_branch: refactor/presentation-theme-model
tags: theme,web,wasm,verification
---

# Scope

The Web workspace was checked against the current source revision after the support manifest
advanced to revision 82. This run validates the TypeScript package entries and package assembly
contracts; it does not claim that fresh browser/WASM artifacts were built.

# Results

- `npm test` passed **142/142** Web contract tests, including non-finite number rejection for
  ordinary options and theme authoring input.
- `npm run build:ts` completed the generated entry build, contract check, and TypeScript compiler.
  The contract check reported 38 WASM exports, 48 runtime bindings, and 5 package entries.
- `npm run verify:packages` passed the prepack check for all 5 package artifacts.
- The Web workspace remained unchanged except for ignored build output. No package was published.

The existing source-level tests already exercise explicit `null` clearing separately from NaN,
Infinity, and negative Infinity. The latter are rejected before JSON serialization can turn them
into protocol `null` values. Revision 82 support vectors are shared by the Web smoke implementation,
but this run did not rebuild or install a Web/WASM package, so that installed consumer remains an
open C7a artifact check.

# Commands

```text
npm test --prefix platforms/web
npm run build:ts --prefix platforms/web
npm run verify:packages --prefix platforms/web
```

Logs: `/tmp/theme-web-contracts-20260913.log`, `/tmp/theme-web-build-ts-20260913.log`, and
`/tmp/theme-web-prepack-20260913.log`.
