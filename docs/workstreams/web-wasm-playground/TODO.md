# Web WASM Playground - TODO

Status: Active
Last updated: 2026-06-01

## M0 - Scope And Evidence Freeze

- [x] WWP-010 [owner=planner] [deps=none] [scope=docs/workstreams/web-wasm-playground]
  Goal: Freeze problem, target state, non-goals, first proof target, and evidence anchors.
  Validation: DESIGN.md, MILESTONES.md, EVIDENCE_AND_GATES.md, WORKSTREAM.json, and CONTEXT.jsonl exist and agree.
  Evidence: docs/workstreams/web-wasm-playground/DESIGN.md
  Context: docs/workstreams/web-wasm-playground/CONTEXT.jsonl
  Handoff: DONE. First proof is the formal WASM crate plus wasm32 compile gate.

## M1 - Formal WASM Crate

- [x] WWP-020 [owner=codex] [deps=WWP-010] [scope=Cargo.toml,crates/merman-wasm,crates/merman-core,crates/roughr]
  Goal: Add a first-class `merman-wasm` crate over `merman-bindings-core` and make the core browser target compile.
  Validation: cargo check -p merman-wasm --target wasm32-unknown-unknown && wasm-pack build crates/merman-wasm --target web --out-dir ../../target/merman-wasm-pkg
  Review: Confirm the WASM crate stays transport-only and does not duplicate render behavior.
  Evidence: docs/workstreams/web-wasm-playground/EVIDENCE_AND_GATES.md
  Context: docs/workstreams/web-wasm-playground/CONTEXT.jsonl
  Handoff: DONE. Added `crates/merman-wasm`, fixed wasm randomness features, and passed wasm/browser package gates.

## M2 - TypeScript Web Package

- [x] WWP-030 [owner=codex] [deps=WWP-020] [scope=platforms/web]
  Goal: Add a Ratex-style TypeScript package that builds WASM, initializes it once, and exposes typed helpers over the JSON contract.
  Validation: npm run build --prefix platforms/web
  Review: Generated `pkg/` handling must avoid accidentally publishing or committing broken artifacts.
  Evidence: platforms/web build output and docs/workstreams/web-wasm-playground/EVIDENCE_AND_GATES.md
  Context: docs/workstreams/web-wasm-playground/CONTEXT.jsonl
  Handoff: DONE. Added `@merman/web`, build/prepack scripts, typed TS wrappers, and generated-artifact ignore rules. NPM publication remains deferred.

## M3 - Playground Integration

- [x] WWP-040 [owner=codex] [deps=WWP-030] [scope=playground]
  Goal: Move and harden `repo-ref/merman-page` as the live editor app, replacing the mock-primary WASM loader with the web package.
  Validation: npm run build --prefix playground
  Review: Browser smoke should prove the real WASM path renders an SVG, not mock output.
  Evidence: playground build output, screenshot or smoke notes, EVIDENCE_AND_GATES.md
  Context: docs/workstreams/web-wasm-playground/CONTEXT.jsonl
  Handoff: DONE. `playground` builds through `@merman/web`, loads real WASM, and renders the default flowchart SVG in browser smoke.

## M4 - GitHub Pages Build

- [x] WWP-050 [owner=codex] [deps=WWP-040] [scope=.github/workflows,playground,platforms/web,docs]
  Goal: Add a Pages workflow and static artifact verification so deploys fail when WASM is absent.
  Validation: local workflow-equivalent build command and dist WASM verifier.
  Review: CI must not depend on checked-in generated WASM artifacts.
  Evidence: workflow file, verifier output, EVIDENCE_AND_GATES.md
  Context: docs/workstreams/web-wasm-playground/CONTEXT.jsonl
  Handoff: DONE. Added a GitHub Pages workflow plus postbuild/static dist verifier; repository Pages
  settings should use GitHub Actions as the source if not already configured.

## M5 - Closeout

- [ ] WWP-060 [owner=planner] [deps=WWP-050] [scope=docs/workstreams/web-wasm-playground,docs/release]
  Goal: Close the lane or split npm publishing/raster/browser QA follow-ons.
  Validation: verify-rust-workstream records fresh final gate evidence.
  Review: review-workstream has no blocking findings.
  Evidence: docs/workstreams/web-wasm-playground/EVIDENCE_AND_GATES.md
  Context: docs/workstreams/web-wasm-playground/CONTEXT.jsonl
  Handoff: Summarize residual risks in HANDOFF.md.

## Follow-ons

- [x] WWP-070 [owner=codex] [deps=WWP-050] [scope=playground,docs/workstreams/web-wasm-playground]
  Goal: Add an optional Mermaid JS compare mode to the playground with lazy Mermaid loading and a side-by-side SVG view.
  Validation: npm run build --prefix playground, browser smoke that opens Compare mode and observes both Merman and Mermaid SVG panes.
  Review: Mermaid JS must not be part of the default page load path.
  Evidence: docs/workstreams/web-wasm-playground/MERMAID_COMPARE_MODE.md
  Context: docs/workstreams/web-wasm-playground/CONTEXT.jsonl
  Handoff: DONE. Side-by-side compare mode lazy-loads Mermaid JS only after opening Compare; overlay, source diff, pixel diff, and local benchmark controls remain deferred.
