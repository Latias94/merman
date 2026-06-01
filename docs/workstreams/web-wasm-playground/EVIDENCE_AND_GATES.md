# Web WASM Playground - Evidence And Gates

Status: Active
Last updated: 2026-06-01

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
