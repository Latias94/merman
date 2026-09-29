---
type: "Session Handoff"
title: "Selectable diagram families U3 verified and U4 in progress"
description: "Session Handoff for Selectable diagram families U3 verified and U4 in progress."
timestamp: 2026-09-26T05:30:22Z
record_id: "4deddc44edc24db0a96aa0a2eda90fc1"
producer_id: "codex-selectable-diagrams"
run_id: "session-01a0dbf7-70a2-73d2-86d3-c0b9e09dc0ad"
---

# Summary

Implement the complete selectable-diagram plan, U1-U8, on `feat/selectable-diagram-families`. The active goal in receiving session `01a0dbf7-70a2-73d2-86d3-c0b9e09dc0ad` remains active. Historical session `01a0d6a8-6993-7972-918a-6e9ac37ccbc0` is no longer authoritative. Ordinary compaction must not rerun historical recovery.

# Verified State

- Plan commit: `bba3ffc84`.
- U1: `2505ffa27`, static family identity separated from optional implementation callbacks. 109 baseline tests and 114 post-change tests passed.
- U2: `dc3dacca3`, 32 selectors across 16 crates; weak forwarding for optional dependencies; low-level defaults stay empty; facade/CLI/rustdoc defaults explicitly select all families; xtask/fuzz roots explicitly select all. Public `diagram_family_selectors()` derives selector mapping from the single core catalog.
- U3: `869aab484`, actual core module/grammar/model/trait/callback/resource gates; four optional direct dependencies; independent Flowchart/Swimlane language admission with shared implementation. Existing shared-language tests split into `tests/swimlane.rs` and `tests/flowchart_shared.rs`.
- All 32 core singleton `cargo check --lib --no-default-features --features diagram-* -j2` combinations passed. Receipts: `target/diagram-feature-graphs/singletons/results.json`. Kanban/Mindmap unused helper warnings were subsequently fixed and rechecked.
- Core no-family library plus new integration tests: 343 passed. Log `target/u3-no-family-test.log`.
- Flowchart+Gantt library plus new integration tests: 621 passed, nextest run `52b7ecb4-d997-4e45-a342-4a946efca35c`.
- Swimlane-only: 392 passed, run `b97385b5-8b12-4f73-80c2-464403009606`.
- Full core library plus new integration tests: 1564 passed, run `3fd8c153-69dd-4602-91f0-9c9fbd1f601c`.
- Existing full integration suites: 25 passed across 7 binaries, run `863c7eec-28b2-4e5e-b666-b6d84963d4ed`.
- No-family `cargo clippy -p merman-core --lib --no-default-features -j2 -- -D warnings` passed; 3 no-family doctests passed; scoped `cargo fmt` and `git diff --check` passed.
- Isolated consumer in ignored `target/diagram-feature-graphs/core-none` passed a runtime assertion that only Error infrastructure has parser callbacks, executable family lists are empty, and static selectors remain 32. Importing `merman_core::diagrams::sequence::SequenceDiagramRenderModel` intentionally failed with E0432; the temporary failing import was removed by restoring that task-owned fixture source. Run `c94692ec-cddb-4249-861c-fca18a536d72`.
- Active Cargo graph probes passed for no-family core, Gantt core, language-only facade, Flowchart+Gantt SVG, and widened core with Flowchart-only renderer. Use `cargo tree` active edges: unfiltered `cargo metadata` includes inactive optional nodes and must not be interpreted as activated dependencies.

# Open Threads

At capture, `/root/render_u4` owns U4 implementation and Cargo execution. Ownership is `crates/merman-render/**` and necessary facade render sources/tests; core is complete and must not be changed silently. `/root/language_surface_research` is read-only U5 research. Query live agent state before messaging or replacing either. Completed implementation handles catalog_u1, selectors_u2, and core_gates_u3 are historical for those units; do not retask them to another implementation unit.

U4 key evidence: gate family modules/layout payloads/artifact variants/SVG dispatch. Flowchart and Swimlane share both layouts under `any` while metadata controls language admission. Mindmap's only direct Flowchart dependency is label metrics in `flowchart/label.rs`: extract pure label helpers, keep shape/bounds helpers with Flowchart, preserve math-None to markdown fallback. SVG parity and Requirement currently import RoughJS helpers through State; import their actual `roughjs_common` owner. `validate_render_input` should check checkpoint, custom provenance, model/metadata pairing, local handler, then existing resource/backend planning. Architecture local handler availability is separate from optional Cytoscape availability. Widened core requires explicit unsupported fallback. Facade SVG module's three complexity reexports need corresponding family gates.

U5 key evidence: ASCII capability catalog currently starts from core typed families and needs local adapter availability; raw model API and metadata-bearing paths differ. Editor completion `diagram_header_items` and `template_items` must filter runnable parser availability while core static headers remain complete. Frontmatter templates contain Flowchart source and also need filtering. Analysis currently has no direct typed family imports. Do not infer ASCII maturity or editor functionality merely from parser availability.

U6 research completed read-only: artifact profile schema is v1 with 34 recipes. Migrate to v2 and required sorted logical `expected.diagram_families`; 30 language recipes explicitly select all, four rust-export recipes retain an empty set. Exclude infrastructure Error when deriving the parser family set. Keep `feature-surface-v1.json` runtime capability namespace separate. Migrate Rust artifact/feature/capability validators, CLI/FFI descriptor tests, Python `artifact_profile_recipe.py` and all filename/schema readers, web descriptor/legal/input-manifest scripts and tests, nix/flake, CLI/LSP dist recipes, and generated legal materials together. Actual artifact probes must run on target consumer/build, never xtask's linked full core. Bindings and Web already expose family capabilities; CLI/Typst only provide incomplete existing summaries and need exact-set evidence. Main script readers include verify_cli_release_archive (exact expected-key allowlist), verify_rustsec_exceptions (schema1), generate-rust-license-report, verify-third-party-licenses, release_surface_contract, cli_installation_contract, verify_cli_installation. Python/JS legal recipe hash/projection changes require regeneration.

U7 native final artifact measurement is entirely pending. Compare pre-change full (baseline `72c024776`), candidate full, candidate Flowchart+Gantt under controlled identical settings and runtime input. No native size result exists yet. U8 migration docs, updated ADRs, release checks, issue response draft, final simplification and independent code review remain pending. No PR, issue comment, publication, or version change is authorized by plan completion.

# Next Action

Integrate and verify U4 after its worker returns, then U5-U8 in dependency order. Serialize Cargo, reuse target, preserve unrelated changes. U1-U3 are local intermediate commits; do not advertise the partial selector surface as fully usable until renderer/language/contracts and native evidence are complete. Keep progress outside the plan. Only mark the goal complete after a fresh requirement-by-requirement audit.

# Citations

- Plan: `docs/plans/2026-09-26-0257-feat-selectable-diagram-families-plan.md`.
- Pinned upstream: Mermaid 11.17.2, `dcb694ddb58dc5ad3502e7e903cac05fd812eac3`; local reference checkout HEAD is newer and not the parity authority.
- Prior U1/U2 evidence shards: `docs/knowledge/engineering/logs/2026-09/`.
- Temporary diagnostic logs/scripts/consumer fixtures under `target/`; do not commit migration scripts or artifacts.
