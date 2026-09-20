---
type: "Work Progress"
title: "Mermaid 12 catalog, Web WASM, Playground and FFI parity checkpoint"
description: "Agentflow integration is staged through Web WASM, Playground goldens, native FFI and UniFFI metadata; Mermaid 12 admission, ELK/default-layout promotion and final version promotion remain incomplete."
timestamp: 2026-09-20T19:45:00Z
record_id: "46ce1a02-cf9e-4e33-ae42-8be866f4da6b"
producer_id: "codex-mermaid12"
run_id: "session-01a0bd10"
related_plan: "docs/plans/2026-09-20-1250-refactor-mermaid-12-alignment-plan.md"
git_branch: "refactor/mermaid-12-alignment"
git_commit: "00fe1723d"
supersedes: "4a08778c9aa445efba94ac3c650e5b69"
---

# Status

GOAL U1-U15 remains active and incomplete. Target alpha.7 is main unpublished work plus Mermaid 12
alignment. Package versions remain alpha.6 and the selected production reference remains 11.17.2 until
atomic U15 promotion. No release, push, PR or merge. The unrelated `crates/merman-core/src/tests/flowchart.rs`
formatting diff remains untouched and unstaged.

# Implemented

Commit `634adea3d` completed the current catalog and fixture projection:

- Added Agentflow family baseline and container/shape variant fixtures under `fixtures/agentflow/`.
- Generated semantic and layout goldens with `update-snapshots --diagram agentflow` and
  `update-layout-snapshots --diagram agentflow`.
- Regenerated the Web diagram catalog and Playground example catalog; the Web catalog now has 36 entries including Agentflow. Usecase remains a native parser/render implementation without public metadata admission.
- Removed stale 35-family assumptions from xtask, WASM editor coverage, Playground examples and benchmark
  tests; synthetic assembler tests now use a local selected count instead of the production family count.
- Added Agentflow to the Web smoke fixture matrix and catalog assertions.
- Fixed the package prepack check for npm JSON array output by using the existing `allowNpm11` parser mode.
- Corrected the feature-matrix product contract so the CLI default must equal its published recipe, including `layout-elk`.
- Explicitly keep Agentflow out of ASCII rendering; the new render-model variant is routed to the existing
  unsupported-diagram error path.

Commit `f4eedcb99` aligned UniFFI's flowchart capability assertion with the canonical registry: the public
`flowchart` variant has no detector, while `flowchart-v2` and `flowchart-elk` provide detection.

# Evidence

- `cargo nextest run -p merman-bindings-core`: 117 passed.
- `cargo nextest run -p merman-ffi`: 51 passed, including C consumer/header smoke and runtime catalog ABI.
- `cargo nextest run -p merman-uniffi`: 20 passed after the capability assertion fix.
- `cargo nextest run -p xtask web_catalog`: 3 passed.
- `cargo run -p xtask -- verify-web-diagram-catalog`: passed.
- `cargo run -p xtask -- verify-playground-example-catalog`: passed.
- Playground examples: 6 passed; benchmark corpus and evidence: 12 passed.
- Web WASM all profiles built successfully, package assembly completed, and `npm --prefix platforms/web run smoke`
  passed for full, analysis, render, editor and ascii packages with 36 diagrams plus DOM safety smoke.
- Web smoke verified dynamic catalog uniqueness, Agentflow metadata and source-backed flowchart capability facts.

# Next Action / Remaining Scope

1. Continue U9/U10/U12 source-backed ELK/default-layout and Usecase residual parity, then regenerate genuine
   target goldens under the Mermaid 12 baseline.
2. Audit remaining generated/native/platform projections for Mermaid source pin and license/feature closure;
   FFI/UniFFI runtime catalogs are dynamic, while protocol parser fixtures may retain arbitrary positive counts.
3. Finish Web Playground browser/benchmark admission and any editor/FFI source-position remapping gaps.
4. At U15, atomically promote Mermaid 12 source/reference hashes, default layout, generated snapshots,
   version projection to v0.8.0-alpha.7, and changelog. Do not promote versions early.

# Source References

Pinned Mermaid commit `98a0945418c76238f15df2afaddbba4272656c3b`; companion ELK references remain pinned in the
plan and generated source inventory. Agentflow source-backed evidence remains in
`target/mermaid-12-admission/agentflow-*-oracle.{cjs,json}`.