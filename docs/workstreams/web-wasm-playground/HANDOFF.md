# Web WASM Playground - Handoff

Status: Active
Last updated: 2026-06-01

## Current State

The lane is open. Scope is frozen around a browser WASM crate, TypeScript package, playground
integration, and Pages workflow. The formal WASM crate, TypeScript web package, live editor
integration, and Pages build gate are now in place.

## Active Task

- Task ID: WWP-060
- Owner: planner
- Files: `docs/workstreams/web-wasm-playground`, `docs/release`
- Validation: verify-rust-workstream records fresh final gate evidence
- Status: READY
- Review: pending
- Evidence: `docs/workstreams/web-wasm-playground/EVIDENCE_AND_GATES.md`

## Decisions Since Last Update

- Added `crates/merman-wasm` over `merman-bindings-core`.
- Added `platforms/web` as `@merman/web`, following the RaTeX package shape.
- Migrated `repo-ref/merman-page` into `playground` and replaced the mock WASM loader with
  `@merman/web`.
- Switched runtime timing in the browser render chain from `std::time` to `web-time` after browser
  smoke exposed a wasm `Instant::now()` panic.
- Added `.github/workflows/pages.yml` to rebuild generated WASM artifacts, build the Vite
  playground, verify `playground/dist`, and deploy via GitHub Pages.
- Added `playground/scripts/verify-dist-wasm.mjs` and wired it to `postbuild`/`verify:dist` so
  missing WASM fails local and CI static builds.
- Use `merman-bindings-core` as the browser backend boundary.
- Use `wasm-bindgen` plus a hand-written TypeScript wrapper instead of UniFFI or C ABI for browser consumers.
- Defer npm publishing and raster/PDF browser output until the core SVG/JSON package and playground work.
- `wasm-pack --out-dir` is relative to the crate root when a crate path is passed; use
  `../../target/merman-wasm-pkg` from the workspace command to keep generated artifacts under the
  root ignored `target`.

## Blockers

- No current WWP-060 blocker recorded.
- Repository settings may still need GitHub Pages source set to GitHub Actions before the first
  successful deployment.
- Tooling note: wasm-pack auto-install of `wasm-bindgen-cli` failed under Rust 1.87 unless the CLI
  was installed with `cargo install wasm-bindgen-cli --version 0.2.108 --locked`.
- Broader suite note: focused web gates pass. Full package suites still have unrelated existing
  failures in `merman-render` math width and `merman-core` snapshot parity; do not treat those as
  WWP-040 regressions without fresh baseline investigation.

## Next Recommended Action

- Run closeout for WWP-060: verify the recorded gate set, review the workstream, and either close
  the lane or split npm publishing, raster/PDF export, and broader browser QA into follow-ons.
