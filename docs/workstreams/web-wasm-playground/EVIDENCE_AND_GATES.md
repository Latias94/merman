# Web WASM Playground - Evidence And Gates

Status: Closed
Last updated: 2026-08-05

## Smallest Current Repro

Before implementation, the browser compile probe failed on randomness dependencies after adding the
`wasm32-unknown-unknown` target:

```bash
cargo check -p merman-bindings-core --target wasm32-unknown-unknown
```

Observed blockers:

- `uuid` requires a wasm-compatible randomness feature for v4 IDs.
- `roughr -> rand -> getrandom@0.2` requires wasm JavaScript randomness support or an alternative
  deterministic path.

## Gate Set

### WWP-020 Targeted Gate

```bash
cargo check -p merman-wasm --target wasm32-unknown-unknown
wasm-pack build crates/merman-wasm --target web --out-dir ../../target/merman-wasm-pkg
```

This proves the formal WASM crate and its transitive render path can compile for browsers and emit
a wasm-bindgen web package.

### WWP-030 Package Gate

```bash
npm install --prefix platforms/web
npm run build --prefix platforms/web
npm run prepack --prefix platforms/web
(cd platforms/web && npm pack --dry-run)
```

This proves the TypeScript wrapper and generated WASM package are aligned.

### WWP-040 Playground Gate

```bash
npm run build --prefix playground
```

This proves the live editor can ship as a static app.

### WWP-050 Pages Gate

```bash
npm ci --prefix platforms/web
npm run build --prefix platforms/web
npm run prepack --prefix platforms/web
npm ci --prefix playground
npm run build --prefix playground
npm run verify:dist --prefix playground
```

This proves the Pages workflow can rebuild generated WASM artifacts locally, build the static
playground, and fail the deploy artifact if the WASM binary or JS shim is absent.

### Broader Closeout Gate

Use focused gates plus relevant Rust package checks instead of `cargo nextest run --workspace`;
the workspace has many parity lanes and broad fixture gates that are unrelated to the web packaging
surface.

### Review Gate

Run `review-workstream` before accepting task or lane completion. Record blocking findings, missing
gates, and residual risks here or link to the review note.

## Evidence Anchors

- `docs/workstreams/web-wasm-playground/DESIGN.md`
- `docs/workstreams/web-wasm-playground/TODO.md`
- `crates/merman-wasm`
- `platforms/web`
- `playground`
- `.github/workflows/pages.yml`

## Notes

Fresh verification is required before marking a task, Codex goal, or lane complete.

## Evidence Log

### 2026-06-01 - WWP-020 Formal WASM Crate

Changes:

- Added `crates/merman-wasm` as a workspace crate.
- Exposed `renderSvg`, `parseJson`, `layoutJson`, `validate`, version helpers, diagram list, and
  theme list through `wasm-bindgen`.
- Enabled wasm-compatible randomness for `uuid` and `roughr -> rand -> getrandom`.

Commands:

```bash
cargo check -p merman-wasm
cargo check -p merman-wasm --target wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.108 --locked
wasm-pack build crates/merman-wasm --target web --out-dir ../../target/merman-wasm-pkg
cargo nextest run -p merman-wasm
cargo fmt --check
```

Results:

- `cargo check -p merman-wasm` passed.
- `cargo check -p merman-wasm --target wasm32-unknown-unknown` passed after dependency feature
  adjustments.
- First `wasm-pack build` compiled Rust but failed while auto-installing `wasm-bindgen-cli` because
  wasm-pack invoked `cargo install` without `--locked` and pulled `time@0.3.47`, which requires
  Rust 1.88. Installing `wasm-bindgen-cli 0.2.108` with `--locked` fixed the tooling issue.
- `wasm-pack build crates/merman-wasm --target web --out-dir ../../target/merman-wasm-pkg` passed.
  It emitted a non-blocking license-file warning.
- `cargo nextest run -p merman-wasm` passed: 3 tests.
- `cargo fmt --check` passed.

### 2026-06-01 - WWP-030 TypeScript Web Package

Changes:

- Added `platforms/web` as `@merman/web`.
- Added `build:wasm`, `build:ts`, prepack verification, and generated `pkg/` cleanup.
- Added TypeScript helpers for WASM initialization, options JSON serialization, SVG rendering,
  parse/layout JSON, validation, version checks, supported diagrams, and themes.
- Ignored generated `platforms/web/dist`, `platforms/web/pkg`, and local node modules.

Commands:

```bash
npm install --prefix platforms/web
npm run build --prefix platforms/web
npm run prepack --prefix platforms/web
cd platforms/web && npm pack --dry-run
cargo fmt --check
git diff --check
```

Results:

- `npm install --prefix platforms/web` passed and produced `platforms/web/package-lock.json`.
- `npm run build --prefix platforms/web` passed; it generated wasm-bindgen output and TypeScript
  declarations.
- `npm run prepack --prefix platforms/web` passed.
- `npm pack --dry-run` from `platforms/web` passed and listed `dist`, `pkg/merman_wasm.js`, and
  `pkg/merman_wasm_bg.wasm` in the tarball.
- `cargo fmt --check` and `git diff --check` passed.

### 2026-06-01 - WWP-040 Playground Integration

Changes:

- Moved the `repo-ref/merman-page` live editor into `playground`.
- Replaced the mock-primary WASM loader with the `@merman/web` TypeScript package.
- Added ASCII rendering to the binding surface and exposed `renderAscii` through WASM/TypeScript.
- Switched timing in the WASM render chain from `std::time` to `web-time` for browser-compatible
  `Instant`/`Duration` support.
- Kept generated playground artifacts out of git via `.gitignore` and `tsBuildInfoFile` settings.

Commands:

```bash
cargo check -p merman-wasm --target wasm32-unknown-unknown
npm run build --prefix platforms/web
npm run prepack --prefix platforms/web
npm run build --prefix playground
cargo nextest run -p merman-wasm -p merman-bindings-core
cargo nextest run -p dugong -p manatee
cargo fmt --check
git diff --check
```

Browser smoke:

- Local preview: `http://127.0.0.1:4173/merman/`
- Headless Chrome/CDP loaded the preview, observed `.wasm` and generated JS resource requests, and
  confirmed `.preview-container svg` was present.
- Passing probe: `svgPresent=true`, `svgNodeCount=95`, requested
  `merman_wasm-CW0mGF3B.js` and `merman_wasm_bg-BccmGt3e.wasm`.
- Screenshot evidence: `target/playground-preview/smoke.png`.

Results:

- First browser smoke failed with `std::time::Instant::now()` panicking on
  `wasm32-unknown-unknown` (`time not implemented on this platform`).
- Adding `web-time 1.1.0` to the browser render dependency chain fixed the runtime panic.
- `cargo check -p merman-wasm --target wasm32-unknown-unknown` passed.
- `npm run build --prefix platforms/web`, `npm run prepack --prefix platforms/web`, and
  `npm run build --prefix playground` passed. Vite reported only bundle-size/plugin-timing
  warnings.
- `cargo nextest run -p merman-wasm -p merman-bindings-core` passed: 15 tests.
- `cargo nextest run -p dugong -p manatee` passed: 278 tests.
- `cargo fmt --check` and `git diff --check` passed.

Residual notes:

- An earlier broad `cargo nextest run -p merman-wasm -p merman-bindings-core -p merman-render -p
  dugong -p manatee` timed out before producing a useful result.
- `cargo nextest run -p merman-render` still fails in
  `math::tests::node_katex_math_renderer_measures_sanitized_flowchart_browser_shell` with
  `matrix width = 282.265625`.
- `cargo nextest run -p merman-core` still fails the snapshot fixture
  `flowchart/stress_flowchart_edge_label_position_064.mmd` because node `labelType` differs
  (`markdown` vs expected `text`).
- Those two failures are outside the WWP-040 WASM/playground integration path and need separate
  baseline triage before being used as regressions for this lane.

### 2026-06-01 - WWP-050 GitHub Pages Build

Changes:

- Added `.github/workflows/pages.yml`.
- Added `playground/scripts/verify-dist-wasm.mjs`.
- Wired the verifier into `playground` as `postbuild` and `verify:dist`.
- Updated workstream context, handoff, TODO, milestones, and journal notes for WWP-050.

Commands:

```bash
npm ci --prefix platforms/web
npm run build --prefix platforms/web
npm run prepack --prefix platforms/web
npm ci --prefix playground
npm run build --prefix playground
npm run verify:dist --prefix playground
```

Negative verifier probe:

```bash
# Temporarily move playground/dist/assets/*.wasm away, run:
npm run verify:dist --prefix playground
# Then restore the WASM file.
```

Results:

- The first local workflow-equivalent run failed at `npm ci --prefix playground` with a Windows
  `EPERM unlink` on `lightningcss.win32-x64-msvc.node` because the local Vite preview was still
  running and held a native module file lock.
- After stopping the preview process, the full workflow-equivalent command passed.
- `npm run build --prefix playground` now runs the postbuild verifier and passed.
- `npm run verify:dist --prefix playground` passed and reported:
  `assets/merman_wasm_bg-BccmGt3e.wasm` and `assets/merman_wasm-CW0mGF3B.js`.
- The negative verifier probe failed as expected with exit code 1 when the generated `.wasm` file
  was temporarily absent, then the file was restored.

Residual notes:

- First pushed Pages run reached the static artifact gates and failed at `Configure Pages` because
  GitHub Pages was not enabled for the repository. Enabled Pages with `build_type=workflow` through
  the GitHub API, then updated Pages actions to their Node 24 compatible major versions.
- `npm ci --prefix playground` reported two moderate npm audit findings in the playground
  dependency tree; this did not block the Pages artifact gate.
- Vite still reports the existing large chunk warning for the playground bundle.

### 2026-06-01 - WWP-070 Mermaid Compare Mode

Changes:

- Added `mermaid@11.15.0` as a playground dependency.
- Added a lazy Mermaid JS renderer wrapper for side-by-side browser comparison.
- Extracted the SVG pan/zoom surface into a reusable `SvgViewport` component.
- Added a `Compare` preview tab with Merman and Mermaid JS panes, render timing, copy SVG, export
  SVG, and export PNG actions.
- Documented the comparison design in `MERMAID_COMPARE_MODE.md`.

Commands:

```bash
npm run build --prefix playground
```

Browser smoke:

- Started the Vite playground at `http://127.0.0.1:5173/`.
- Loaded the default diagram, opened the `Compare` tab, and confirmed two `.preview-container svg`
  elements were present.
- Confirmed Mermaid JS was not loaded before opening `Compare`, and was loaded after opening it.
- Captured screenshot evidence at `target/playground-compare-smoke.png`.

Results:

- `npm run build --prefix playground` passed, including the postbuild WASM verifier.
- Headless Chrome smoke passed with `totalSvgCount=2`, `hasMerman=true`, `hasMermaid=true`,
  `loadedMermaid=true`, and no console errors.
- Vite still reports the existing large chunk warning. Mermaid JS is dynamically imported, but its
  own optional diagram chunks are large when the compare mode is used.

### 2026-06-01 - WWP-080 Local Render Bench Panel

Changes:

- Added a toolbar `Bench` dialog for current-diagram local browser timing.
- Added `bench-runner.ts` with warmup and measurement loops over Merman WASM and Mermaid JS.
- Added `@radix-ui/react-checkbox` because the existing shadcn checkbox component was not yet
  backed by a package dependency.
- Changed SVG viewport transforms from `translate3d` plus `will-change-transform` to plain 2D
  transforms with rounded pan offsets to avoid browser layer-rasterization blur while zooming.

Commands:

```bash
npm run build --prefix playground
```

Browser smoke:

- Started the Vite playground at `http://127.0.0.1:5173/`.
- Opened the toolbar `Bench` dialog.
- Ran a short bench with warmup `1` and measure `5`.
- Confirmed Merman and Mermaid JS rows were present with median/p95 timing columns and zero console
  errors.
- Confirmed the active preview viewport no longer used `will-change-transform` or `translate3d` in
  its transform path.
- Captured screenshot evidence at `target/playground-bench-smoke.png`.

Results:

- `npm run build --prefix playground` passed, including the postbuild WASM verifier.
- Headless Chrome smoke passed with Merman and Mermaid JS timing rows, no console errors, and
  `previewHasWillChange=false`, `previewHasTranslate3d=false`.
- Vite still reports the existing large chunk warning.

### 2026-06-29 - Editor Core Browser Language API

Changes:

- Added stateless `@mermanjs/web` editor APIs backed by `merman-editor-core` for diagnostics,
  code actions, completion, hover, document symbols, single-document symbol search, definition,
  references, prepare-rename, rename, semantic-token legend, and semantic tokens.
- Wired the playground Monaco language service to the browser editor APIs for diagnostics,
  completion, hover, code actions, document symbols, definition, references, rename, and semantic
  tokens. Static snippets and lexical tokenization remain loading/fallback behavior.
- Updated the published browser-full WASM package budget as a regression guard for the intentional
  default editor API expansion.

Commands:

```bash
npm run build --prefix platforms/web
npm run smoke --prefix platforms/web
npm run prepack --prefix platforms/web
npm run build --prefix playground
npm run verify:dist --prefix playground
```

Results:

- `npm run build --prefix platforms/web` passed and rebuilt the default `browser-full` package.
- `npm run smoke --prefix platforms/web` passed with `diagrams=25`, `render=true`, `ascii=true`,
  `core_full=true`, and `ratex_math=false`.
- `npm run prepack --prefix platforms/web` passed against `docs/release/WASM_SIZE_BUDGETS.json`.
- `npm run build --prefix playground` passed, including the postbuild WASM verifier.
- `npm run verify:dist --prefix playground` passed and found the generated WASM binary and JS shim.

### 2026-06-29 - WWP-060 Closeout

Changes:

- Closed the web/WASM/playground lane in the workstream metadata and handoff docs.
- Marked WWP-060 done and split residual npm publishing, raster/PDF export, and broader browser QA
  into explicit follow-on work instead of keeping them inside the lane.

Commands:

```bash
git diff --check
```

Results:

- Closeout docs are internally consistent.
- Remaining work is documented as follow-on scope, not as active lane debt.

## Review And Verification Closeout - 2026-06-29

Closeout review result:

- Workstream compliance: no blocking findings. The lane stayed within browser WASM, TypeScript
  package, playground integration, Pages workflow, and shared editor-core browser API scope.
- Code quality: no blocking findings. The editor-core extraction kept protocol adapters thin and
  the browser API remained JSON-based and browser-only.
- Missing gates: none for this closeout. The fresh focused web/package gates are already recorded
  above.

Verification claim:

- Verified claim: the lane is closed; residual npm publishing, raster/PDF export, and broader
  browser QA are explicit follow-on candidates rather than active lane debt.

### 2026-08-05 - R16 Editor Worker Artifact Decision

Changes:

- Replaced the VM/transpile semantic-token harness with importable TypeScript protocol, client,
  runtime, and Monaco adapter tests.
- Bound read-only Worker queries to URI/version, retained one in-flight plus one latest source
  snapshot, added request deadlines and bounded tombstones, and validated all 11 query result
  shapes before Monaco projection.
- Kept message envelopes and request payloads exact while projecting away unknown nested result
  fields, matching the forward-compatible diagnostics contract.
- Added an on-demand dual-build measurement for the complete and editor Worker artifacts. Browser
  measurement is deliberately not a CI gate.
- Removed the duplicate JSON Schema projection; `contract.mjs` plus the receipt `schemaVersion` is
  the single executable evidence contract.

Commands:

```bash
npm run test:editor-worker --prefix playground
npm run test:editor-artifact-decision --prefix playground
npm run test:build-graph --prefix playground
npm run build --prefix playground
npm --prefix playground/tests test -- monaco.worker.spec.ts --project=chromium-desktop
npm run measure:editor-artifacts --prefix playground
```

Results:

- Native TypeScript Worker tests passed: 53 tests.
- Pure artifact-decision tests passed: 14 tests.
- Focused build-graph tests passed: 41 tests.
- Production Playground build and dist verification passed with Vite 8.1.5.
- Real Chromium Monaco Worker smoke passed: 5 tests, including all 35 semantic-token family
  baselines, request-local rename failure, Retry recovery, and Benchmark isolation.
- The fresh same-commit measurement ran against commit
  `16ddd1d94536514dcd962b03b998c6efecae7146` with a clean worktree and four balanced AB/BA
  blocks. Its receipt is authoritative.
- Full and editor Workers were exactly equivalent across 35 families and 11 query kinds: 385 cells
  per variant, zero mismatches, and aggregate SHA-256
  `e52016004129f4a12c0b316be1890f614898003afc1318fd543c4f07b674596c` for both.
- All six cold/warm primary latency comparisons passed the joint 5% and 20 ms limit.
- The editor split failed the selection rule because cold transfer was 7,495,593 bytes versus
  6,133,137 bytes for full, and peak memory was 30,852,334 bytes versus 30,739,906 bytes for full.
- The authoritative result therefore retains `@mermanjs/web` for both the main renderer and the
  language Worker. The complete receipt is checked in at
  [`editor-artifact-receipt-v1.json`](./editor-artifact-receipt-v1.json).

### 2026-08-05 - Canonical Opaque Artifacts And Authoritative Build Graphs

Changes:

- Replaced six repeated engine/bootstrap/page/output inventories with one validated opaque-realm
  artifact plan consumed by the builder, verifier, CSP injector, Vite inputs, dist verifier, and
  generated browser projections.
- Split Compare, opaque Benchmark Mermaid, and trusted Benchmark Merman into separate static
  projection leaves. Opening Compare no longer transfers the Benchmark bootstrap.
- Replaced the handwritten extension/alias resolver with TypeScript configuration and module
  resolution. Source ownership includes type-only edges without reimplementing package exports,
  paths, or extension rules.
- Added one Vite manifest adapter for entries, static/reachable closure, emitted files, and asset
  ownership. Optional-feature, realm-isolation, dist, and Playwright helpers consume that adapter.
- Removed engine sentinels, source-marker/version/package searches, legacy output lists, duplicated
  manifest traversal, and parser-oriented resolver tests after their structural/behavioral evidence
  passed.
- Added an emitted-JavaScript AST gate for engine module requests because Rolldown metadata alone
  does not report computed `import(url)` calls. Bootstrap blob imports remain separately digest-bound.
- Injected the Playground pause-coordinator capability into the browser Benchmark factory. Benchmark
  feature and corpus closures now fail if either can reach the Compare artifact root.
- Added the checked [validation migration ledger](../../../playground/scripts/validation-migration-ledger.json),
  mapping each removed or retained gate to its stable invariant and proving tests.
- Added explicit ESLint and Node TypeScript project coverage for `scripts/**/*.mjs`.

Commands:

```bash
npm run test:build-graph --prefix playground
npm test --prefix playground
npm run build:prepared --prefix playground
npm --prefix playground/tests test -- benchmark.controller.spec.ts benchmark.realm.spec.ts --project=chromium-desktop
npm --prefix playground/tests test -- playground.smoke.spec.ts --project=chromium-desktop --grep "Compare owns one local Mermaid realm"
npm run record:zenuml-browser-admission --prefix playground
npm run verify:zenuml-browser-admission --prefix playground
```

Results:

- Hermetic artifact/source/Vite/CSP/package policy suite passed: 35 tests.
- Complete prepared unit suite passed, including 53 Worker, 54 realm, and 98 benchmark tests.
- Vite 8.1.5 production build and plan-driven dist verification passed. The manifest contains
  distinct `opaque-compare-artifact` and `opaque-mermaid-artifact` activation roots; all optional
  workbenches remain outside the initial static closure.
- Chromium Benchmark controller and realm flows passed: 9 tests, including reversible settings,
  retained-result reruns, lifecycle invalidation, denied authority, cold/warm reuse,
  poisoning/replacement, and hidden-realm behavior.
- Chromium Compare activation and coherent publication passed without fetching its artifact before
  user activation: 1 test.
- ZenUML browser admission evidence was regenerated from the final source list and verified.

### 2026-08-05 - Right-Sized Browser Verification And Mobile Interaction Coverage

Changes:

- Replaced the duplicated full desktop/mobile Playwright matrix with three explicit ownership
  lanes: mandatory Chromium desktop coverage, mandatory focused Firefox/WebKit smoke coverage, and
  an on-demand Chromium mobile-interaction suite.
- Added one linear non-Chromium smoke flow that covers production WASM startup, the initial render,
  dialog focus restoration, Compare realm publication, system-theme changes, and Compare realm
  cleanup on a persisted BFCache `pagehide` event.
- Added focused portrait, shortened-viewport, and landscape mobile contracts for compact toolbar
  controls, workspace navigation, dialog scroll ownership, Preview touch pan/zoom/fit, and
  document overflow.
- Made benchmark settings reversible and retained their selected inputs after a run, so users can
  return to the configuration step without discarding the last result.
- Narrowed Playwright WebKit error normalization to the exact `SecurityError` name,
  `The operation is insecure.` message, and injected `web-inspector://bootstrap.js` stack tuple.
  Other init-script and application-origin errors remain failures.
- Removed stale mobile branches from desktop scenarios and removed an unused ZenUML test alias.
- Added [MOBILE_QA.md](./MOBILE_QA.md) for the remaining real-device iOS Safari and Android Chrome
  checks instead of representing emulation as complete device evidence.

Commands:

```bash
npm run test:browser:chromium:desktop:built --prefix playground
npm run test:browser:smoke:non-chromium:built --prefix playground
npm run test:browser:mobile:built --prefix playground
npm run test:browser:typecheck --prefix playground
npm run record:zenuml-browser-admission --prefix playground
npm run verify:zenuml-browser-admission --prefix playground
```

Results:

- The previous mandatory configuration discovered 88 full Chromium cases: 44 desktop cases plus
  the same 44 cases under mobile emulation.
- The new mandatory configuration discovers 45 cases: 43 Chromium desktop cases and one focused
  smoke case in each of Firefox and WebKit. This removes 43 duplicated mandatory selections, a
  48.9% reduction, while preserving browser-engine coverage.
- The focused on-demand mobile lane contains 3 interaction cases and completed 3/3 in 6.4 seconds.
- Firefox and WebKit smoke completed 2/2 in 12.6 seconds.
- Chromium desktop completed 43/43 in 2.7 minutes. An earlier run exposed one stale focus
  assumption in manual tab activation; the scenario was corrected, passed in isolation, and then
  passed again as part of this complete final run.
- No exact historical wall-clock comparison is claimed. Reconstructing a self-consistent old
  generated browser artifact would have required a second ad hoc validation/build path, so the
  comparison intentionally uses reproducible Playwright discovery counts plus current measured
  lane times.
