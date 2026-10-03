---
type: "Work Progress"
title: "Mermaid 12 first implementation checkpoint"
description: "Work Progress for Mermaid 12 first implementation checkpoint."
timestamp: 2026-09-20T05:46:52Z
record_id: "9c1028518e1842a08b5469d9b807f17a"
producer_id: "codex-mermaid12"
run_id: "session-01a0bd10"
---

# Summary

The maintainer explicitly started the Mermaid 12 implementation goal. The goal remains active;
the full plan is not complete. Work is on `refactor/mermaid-12-alignment`, based on
`54d257aa8be7ae9ab1332d1b257205b60b43dbbd`. All current changes belong to this task; no commits yet.

# Details

- U1: installed an isolated reference graph under ignored
  `target/mermaid-12-admission/reference`, using Mermaid 12.0.0, CLI 11.17.0,
  Tidy plugin 1.0.1, ZenUML plugin 1.0.1 / core 3.50.1, Puppeteer 25.6.0.
  `npx --yes npm@12.0.2 install --ignore-scripts ...` succeeded. The exact official npm audit
  signatures command succeeded with zero invalid/missing entries. Raw output is
  `target/mermaid-12-admission/npm-signatures.json` (53 verified entries).
- The staged Mermaid package tree SHA-256 is
  `df542ed953a0a169733d7a3d8f339a964d7ae19b196c83254ebfc4599904d333`.
  `target/mermaid-12-admission/target-bundle.json` is an incomplete temporary projection input:
  only release identity and reference workspace were updated from the old descriptor. It must
  NOT be promoted or claimed as a fully admitted graph.
- Both installed CLI 11.16 and staged CLI 11.17 unconditionally load/register the external ELK
  adapter in `src/index.js`. The staged CLI still brings the 0.2 line. Therefore reference
  generation must converge on the existing scripted renderer for Mermaid 12; an npm override
  alone does not produce the standard built-in ELK behavior.
- Root changed only `crates/xtask/src/cmd/generate.rs`: removed forced `theme: default` in the
  scripted JSON/JS path and the explicit mmdc `-t default` arguments. The mmdc path still has its
  own defaults and must not be considered fixed for v12. No extra runtime selection is implemented.
- Extracted the actual updated scripted renderer into ignored `target/mermaid-12-admission/render.cjs`
  and exercised it against the verified staged graph. Default Flowchart, explicit
  classic/Dagre Flowchart, upstream Agentflow basic example, and a Usecase example all rendered
  successfully. Outputs live beside that script. This small probe used the installed Microsoft
  Edge executable; it is development evidence, not a promoted primary fixture baseline.
- A Puppeteer pinned headless browser download is running in exec session `2997` (no output yet).
  Node 24.21.0 is installed. Default npm is 11.12.1; official admission was invoked via npm 12.0.2.
  Puppeteer 25 `executablePath()` is asynchronous. Old cache contains Chromium shell 131 only;
  system Edge is available. Do not start duplicate browser downloads without checking session 2997.
- U2 agent `/root/config_v12` (idle) implemented initialization delta preservation,
  `config/appearance.rs`, safe source-layer resolution, theme sentinel handling and obsolete
  detector/effect removal. Typed historical explicit entrypoints remain supported. Files are
  the core config, lib, parse_pipeline, detect, family, plus new appearance integration tests.
- `cargo nextest run -p merman-core --test appearance_resolution --test source_presentation`
  passed all 9 tests. A focused lib run passed 39 of 42; three existing tests in
  `crates/merman-core/src/tests/detect.rs` still assert the old renderer-selected `class` / `state`
  IDs. Update them to target-source behavior and explicit layout selection, not old defaults.
- U2 production generated defaults/themes are still 11.17.2. Existing `gen-default-config` and
  `gen-theme-snapshot` hardcode the selected runtime, package hash and source provenance. They
  need a narrow staging reference input before generating target data; do not hand-write a v12
  default table. Agent offered to own these two generator files but has NOT yet been assigned.
  Root owns `generate.rs` and `mermaid_reference.rs`; no registry changes have landed yet.
- U4 agent `/root/layout_audit` (idle) implemented the six missing layered strategies within
  `merman-elk-layered`, including pipeline dispatch, necessary options/error exports, work controls,
  30 actual elkjs 0.9.3 phase-two oracle cases and execution tests. It verified algorithm bodies
  against the pinned Java v0.9.1 source and the published JS; historical source baseline remains
  `62d5909f96fad541bc101ad52dabaece6b7eab7e`, elkjs source `a8304cf79fde75bc2ab1a89d28320f53f8637436`.
- The first Rust compilation found a StretchWidth type-inference issue; the agent fixed it.
  The next focused run compiled, then passed 35 of 37 executed tests, with 3 unrun after failure.
  Failures: `p2layers::elkjs_0_9_3_cases::position_spans_interactive` produced
  `[[1],[3],[0],[2,6],[4],[5]]`, expected `[[1],[3],[0],[6,2],[4],[5]]`;
  `p2layers::execution_tests::all_six_layerers_execute_the_complete_pipeline_with_self_loops`
  failed because the final LongestPath output had no `layer_index` on A. Investigate whether
  finalization intentionally clears layer indexes before changing the assertion.
  These failures have NOT yet been sent to the agent. Do not accept U4 as complete.
- U4 agent warns that `importer.rs` nested options still need U5 adapter ownership. Do not assume
  root layering/preset options propagate automatically to children.
- No other plan units are complete; formal bundle, reference locks, feature defaults and legal
  materials are untouched. Do not claim full target parity from these initial changes.

# Next Action

1. Resume `/root/layout_audit` with the two exact test failures, keeping ownership in its crate.
2. Resume `/root/config_v12` to update stale detector tests and add a narrow staging input to
   the existing default/theme generators. Reuse the selected-bundle shape and exact package-tree
   checks; require explicit staging output paths and preserve default pinned validation. Do not
   create a second admission system. Main can supply an extracted release/runtime descriptor API
   if needed; coordinate file ownership before edits.
3. Main finish U1 scripted-renderer convergence, built-in/external layout projection support and
   bounded registration inventory. No TypeScript interpreter. Use the existing target samples.
4. Run Rust checks serially. After focused tests pass, obtain independent source-backed code review,
   then make focused Conventional Commits. Preserve the accepted plan and all unrelated changes.

# Citations

- [Implementation plan](../../../../plans/2026-09-20-1250-refactor-mermaid-12-alignment-plan.md)
- `.agents/skills/align-mermaid-release/SKILL.md`
- `docs/release/MERMAID_UPGRADE_PLAYBOOK.md`
