# ASCII Renderer Productization - Handoff

Status: Complete
Last updated: 2026-05-28

## Current State

The ASCII renderer productization lane is implemented and verified. It shipped:

- `crates/merman-ascii` as the terminal/text rendering crate.
- Tracked `mermaid-ascii` MIT license attribution and copied fixture provenance.
- Initial flowchart ASCII/Unicode rendering for boxed nodes and direct LR/TD edges.
- Initial sequence ASCII/Unicode rendering for participants, lifelines, solid/dotted messages,
  reverse messages, self messages, labels, and visible autonumber.
- Explicit unsupported-feature diagnostics and support matrices for flowchart and sequence output.
- Top-level `merman --features ascii` APIs under `merman::ascii`.
- `merman-cli render --format ascii|unicode` behind the CLI `ascii` feature.
- README, crate docs, CLI docs, changelog notes, tests, and closeout evidence.

## Final Task

- Task ID: ARP-090
- Owner: codex
- Goal: Close verified ASCII renderer lane.
- Validation:
  - `cargo fmt --all --check`
  - `cargo nextest run -p merman-ascii`
  - `cargo nextest run -p merman --features ascii`
  - `cargo nextest run -p merman-cli`
  - `cargo nextest run -p merman-cli --features ascii`
  - `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings`
  - `cargo clippy -p merman-cli --features ascii --all-targets -- -D warnings`
  - `cargo package -p merman-ascii --list --allow-dirty`
  - `git diff --check`
- Status: DONE

## Decisions

- ASCII rendering lives in `merman-ascii`, not `merman-render`, because it owns character-cell
  layout and output stability rather than SVG/DOM parity.
- The crate consumes `merman-core` typed render models and does not port the Go parser.
- Output support is intentionally subset-first with explicit unsupported-feature errors.
- Upstream `repo-ref/mermaid-ascii` remains a gitignored research reference; build/release
  evidence uses tracked fixtures and license files only.
- CLI support is opt-in through the `ascii` feature and `render --format ascii|unicode`.

## Follow-Ups

No required next task remains in this workstream. Follow-up candidates, if prioritized:

- Broaden flowchart support for subgraphs, labels, non-rect shapes, and more complex routing.
- Broaden sequence support for notes, boxes, activations, create/destroy, actor shapes, wrapping,
  and rich actor metadata.
- Add CJK/emoji placement coverage beyond current width-based sizing.
- Decide release packaging strategy for how many copied fixtures should ship in the published
  crate once output compatibility stabilizes further.
