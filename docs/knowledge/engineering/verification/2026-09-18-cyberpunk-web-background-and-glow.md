# Cyberpunk public Web background and glow verification

Date: 2026-09-18.
Production source baseline: `0f8ffe7580cd53006420a7898d3ebb8fca9f5a9b`.
Scope: the implemented background and Flowchart node/edge effects in U9 of
`docs/plans/2026-09-16-1618-refactor-theme-product-boundaries-plan.md`.

## Change

Strengthened the existing Web package smoke around `exportThemePreset("cyberpunk")`:

- The exported recipe retains the dark base, centered radial illumination with the
  farthest-corner radius, cyan stops, two perpendicular 40px grid tiles, and
  back-to-front Screen layer order.
- JSON export/import through the public runtime reproduces preset-selected SVG.

Added a Chromium test through the production Playground's public theme selection.
It checks computed gradient colors, centers, radius, grid coordinates, tile sizes,
blend modes, layer order, and the links from painted rectangles to paint servers.
It checks the node and edge blur graphs on actual node rectangles and edge paths
(rejecting a filter attached only to a label), then disables all filters and restores each terminal class separately. Each restoration must change
painted screenshot pixels. This guards against unused filter definitions being
mistaken for a visible effect.

No production code, dependency, public contract, or qualification scope changed.

## Verification

The old local Web WASM failed freshness verification because its input manifest
still referenced a deleted source file. It was not used for the results below.

Freshly built and checked, with `CARGO_BUILD_JOBS=1` for WASM:

- `platforms/web`: `node scripts/build-wasm.mjs --package full`.
- `platforms/web`: `npm run build:ts` and `npm run build:packages`.
- `platforms/web`: `node scripts/verify-wasm-inputs.mjs --package full` — passed.
- `platforms/web`: `node scripts/smoke.mjs --package-id full` — passed; reports
  35 diagram variants. The public contract check covered 40 WASM exports,
  50 runtime bindings, and five package entry declarations.
- `playground`: `npm run build` — passed, including built-distribution validation.
- `playground/tests`: `npm run typecheck` — passed.
- `playground/tests`: `npm run test:desktop -- theme-cyberpunk.spec.ts` — 1/1 passed.
- `playground`: `./node_modules/.bin/eslint tests/theme-cyberpunk.spec.ts` — passed.
- `node --check platforms/web/scripts/smoke.mjs` and `git diff --check` — passed.

The browser test saves `public-cyberpunk.png` under its Playwright output directory.
The screenshot was also inspected: grid, cyan borders, labels, node glow and the
connecting edge are visible. Pixel comparison establishes that the edge filter
also changes the rendered image; it does not measure perceptual similarity.

During test development, an expectation incorrectly assumed alpha had been split
into `stop-opacity`. Chromium correctly kept it in `stop-color: rgba(...)`; the
assertion now checks that representation and the separate opacity multiplier.
A variable rename also accidentally changed an unrelated measurement protocol
literal; diff review caught it, the literal was restored, and full smoke was rerun.
Neither issue required a production change.

## Limits

This is one native-label Flowchart scene on Chromium, not complete Cyberpunk visual
qualification. It does not establish text glow, HTML-label effects, Sequence or XY
Chart effects, PNG/PDF scene fidelity, another browser, or all Web package profiles.
Package assembly copied the available other profiles; only `full` was freshly
rebuilt and verified here. U6, U7, U8 and overall U9 remain open. Public catalog
qualification cells and C7a contract status are unchanged.
