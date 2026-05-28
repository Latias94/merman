# ASCII Renderer Productization - TODO

Status: Active
Last updated: 2026-05-28

## M0 - Scope And Evidence Freeze

- [x] ARP-010 [owner=codex] [deps=none] [scope=docs/adr/0065-ascii-output-boundary.md,docs/workstreams/ascii-renderer-productization]
  Goal: Freeze the product boundary, crate direction, third-party attribution strategy, and first
  implementation slices.
  Validation:
  - Workstream docs exist and agree.
  - ADR records the ASCII output boundary.
  - `WORKSTREAM.json` points to the authoritative docs.
  Evidence: `docs/adr/0065-ascii-output-boundary.md`,
  `docs/workstreams/ascii-renderer-productization/DESIGN.md`.
  Handoff: DONE.

## M1 - Crate And Provenance Foundation

- [x] ARP-020 [owner=codex] [deps=ARP-010] [scope=Cargo.toml,Cargo.lock,crates/merman-ascii]
  Goal: Add the `merman-ascii` crate skeleton with public options/errors and tracked
  `mermaid-ascii` attribution.
  Validation:
  - `cargo fmt --all --check`
  - `cargo check -p merman-ascii`
  - `cargo nextest run -p merman-ascii`
  Review: `review-workstream` before accepting completion.
  Evidence: `crates/merman-ascii/README.md`, third-party notice/license file, crate smoke tests.
  Handoff: DONE.

- [x] ARP-030 [owner=codex] [deps=ARP-020] [scope=.gitattributes,crates/merman-ascii/tests/testdata]
  Goal: Copy the necessary `mermaid-ascii` graph and sequence golden fixtures into tracked testdata
  with source commit provenance.
  Validation:
  - `cargo nextest run -p merman-ascii fixture_inventory`
  - `git diff --check`
  Review: Confirm copied fixture headers or inventory docs cite upstream URL, commit `6fffb8e`, and
  MIT license.
  Evidence: tracked testdata and fixture inventory test.
  Handoff: DONE.

## M2 - Flowchart Vertical Slice

- [x] ARP-040 [owner=codex] [deps=ARP-020,ARP-030] [scope=crates/merman-ascii/src/text.rs,crates/merman-ascii/src/canvas.rs,crates/merman-ascii/src/graph,crates/merman-ascii/src/lib.rs]
  Goal: Port the graph canvas, text width, layout, routing, and drawing primitives enough to render
  simple flowcharts from an internal ASCII graph model.
  Validation:
  - `cargo nextest run -p merman-ascii graph::`
  - `cargo nextest run -p merman-ascii graph_golden`
  Review: `review-workstream` with attention to output stability and routing limits.
  Evidence: graph golden tests and focused unit tests.
  Handoff: DONE.

- [x] ARP-050 [owner=codex] [deps=ARP-040] [scope=crates/merman-ascii/src/graph,crates/merman-ascii/tests,crates/merman-ascii/FLOWCHART_SUPPORT.md,crates/merman-ascii/README.md]
  Goal: Adapt `FlowchartV2Model` into the ASCII graph renderer and document the first supported
  feature matrix.
  Validation:
  - `cargo nextest run -p merman-ascii flowchart`
  - `cargo check -p merman-ascii`
  Review: Verify the adapter consumes `merman-core` semantics and does not parse Mermaid text.
  Evidence: flowchart adapter tests and compatibility table.
  Handoff: DONE.

## M3 - Sequence Vertical Slice

- [x] ARP-060 [owner=codex] [deps=ARP-020,ARP-030] [scope=crates/merman-ascii/src/sequence.rs,crates/merman-ascii/src/lib.rs,crates/merman-ascii/tests/sequence_model.rs,crates/merman-ascii/SEQUENCE_SUPPORT.md,crates/merman-ascii/README.md]
  Goal: Port the sequence layout and drawing algorithm for participants and basic messages.
  Validation:
  - `cargo nextest run -p merman-ascii sequence`
  - `cargo nextest run -p merman-ascii sequence_golden`
  Review: Confirm unsupported sequence constructs degrade explicitly.
  Evidence: sequence golden tests and feature matrix updates.
  Handoff: DONE.

## M4 - Public API And Host Integration

- [ ] ARP-070 [owner=unassigned] [deps=ARP-050,ARP-060] [scope=crates/merman-ascii,crates/merman]
  Goal: Expose ASCII rendering through a stable library API and opt-in `merman` feature.
  Validation:
  - `cargo check -p merman --features ascii`
  - `cargo nextest run -p merman-ascii`
  - `cargo nextest run -p merman --features ascii`
  Review: Public API review for semver stability, error shape, and unsupported-feature reporting.
  Evidence: API tests and README examples.
  Handoff: Final status must be DONE, DONE_WITH_CONCERNS, BLOCKED, or NEEDS_CONTEXT.

- [ ] ARP-080 [owner=unassigned] [deps=ARP-070] [scope=crates/merman-cli,README.md,CHANGELOG.md]
  Goal: Add CLI output support or explicitly split CLI integration into a follow-on if library API
  stabilization needs more time.
  Validation:
  - `cargo nextest run -p merman-cli`
  - `cargo check -p merman-cli --features ascii`
  - `git diff --check`
  Review: CLI behavior must not change existing SVG/raster defaults.
  Evidence: CLI tests and docs.
  Handoff: Final status must be DONE, DONE_WITH_CONCERNS, BLOCKED, or NEEDS_CONTEXT.

## M5 - Verification And Closeout

- [ ] ARP-090 [owner=planner] [deps=ARP-070] [scope=docs/workstreams/ascii-renderer-productization]
  Goal: Run fresh focused gates, record evidence, and close this lane or split remaining unsupported
  Mermaid features into follow-ons.
  Validation:
  - `cargo fmt --all --check`
  - `cargo nextest run -p merman-ascii`
  - `cargo nextest run -p merman --features ascii`
  - `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings`
  - `git diff --check`
  Review: `verify-rust-workstream` followed by `close-workstream`.
  Evidence: `docs/workstreams/ascii-renderer-productization/EVIDENCE_AND_GATES.md`.
  Handoff: Final status must be DONE, DONE_WITH_CONCERNS, BLOCKED, or NEEDS_CONTEXT.
