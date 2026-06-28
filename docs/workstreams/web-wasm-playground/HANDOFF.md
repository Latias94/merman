# Web WASM Playground - Handoff

Status: Closed
Last updated: 2026-06-29

## Current State

The lane is closed. Scope was frozen around a browser WASM crate, TypeScript package, playground
integration, and Pages workflow. The browser WASM crate, TypeScript web package, live editor
integration, Pages build gate, and shared editor-core browser API surface are now in place.

## Closed Task

- Task ID: WWP-060
- Owner: planner
- Files: `docs/workstreams/web-wasm-playground`, `docs/release`
- Validation: `verify-rust-workstream` and focused web/package gates recorded in `EVIDENCE_AND_GATES.md`
- Status: DONE
- Review: no blocking findings recorded in the closeout pass
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
- GitHub Pages has been enabled for the repository with `build_type=workflow`.
- Tooling note: wasm-pack auto-install of `wasm-bindgen-cli` failed under Rust 1.87 unless the CLI
  was installed with `cargo install wasm-bindgen-cli --version 0.2.108 --locked`.
- Broader suite note: focused web gates pass. Full package suites still have unrelated existing
  failures in `merman-render` math width and `merman-core` snapshot parity; do not treat those as
  WWP-040 regressions without fresh baseline investigation.

## Next Recommended Action

- Lane closed.

## Follow-Ons

Split these into separate workstreams when they become concrete:

- npm publishing
- raster/PDF export
- broader browser QA
