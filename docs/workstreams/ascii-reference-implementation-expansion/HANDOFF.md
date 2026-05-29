# ASCII Reference Implementation Expansion — Handoff

Status: Active
Last updated: 2026-05-29

## Current State

This lane governs how `merman-ascii` should learn from `repo-ref/mermaid-ascii` and
`repo-ref/beautiful-mermaid` while preserving the model-driven boundary. Reference provenance is
tracked. classDiagram ASCII now renders boxes plus an expanded single-relationship subset from
`RenderSemanticModel::Class`, and ER ASCII now renders entity boxes plus a first single-relationship
subset from `RenderSemanticModel::Er`. XYChart ASCII now renders compact bars, lines, mixed plots,
horizontal bars, and Unicode/ASCII chart characters from `RenderSemanticModel::XyChart`.

## Active Task

- Task ID: ARI-050
- Owner: codex
- Files:
  - `crates/merman-ascii/src/lib.rs`
  - `crates/merman-ascii/src/xychart/*`
  - `crates/merman-ascii/tests/xychart_model.rs`
  - `crates/merman-ascii/README.md`
  - `docs/workstreams/ascii-reference-implementation-expansion/*`
- Validation: `cargo nextest run -p merman-ascii xychart`;
  `cargo nextest run -p merman-ascii`;
  `cargo fmt --all --check`; `cargo clippy -p merman-ascii --all-targets -- -D warnings`;
  `git diff --check`
- Status: DONE
- Review: self-review found no blocking findings; broader planner review can still inspect whether
  legends/color/full-size terminal plot layout should be split into follow-on work before closeout.
- Evidence: `EVIDENCE_AND_GATES.md`

## Decisions Since Last Update

- `beautiful-mermaid` is a reference implementation, not a spec.
- New ASCII diagram renderers must consume `merman-core` typed models.
- Do not port or duplicate `beautiful-mermaid`'s parser or SVG renderer into `merman-ascii`.
- Class, ER, and xychart are separate vertical slices.
- The ARI-020 class slice supports class boxes, members, methods, ASCII/Unicode borders, and one
  solid extension relationship.
- ARI-030 expands the single-relationship layout to extension labels, reverse extension
  orientation, aggregation, composition, dependency dotted arrows, and Unicode composition markers.
- Multiple relationships, relation layouts with unrelated classes, association/no-marker
  relationships, lollipop markers, namespaces, notes, and styling remain follow-on work with
  explicit diagnostics where the current slice encounters them.
- ARI-040 adds ER entity boxes, attributes, relationship labels, identifying/non-identifying line
  style, and common cardinality markers from typed `ErRelSpecRenderModel`.
- Multiple ER relationships, ER relationship layouts with unrelated entities, entity styling/classes,
  and richer ER graph placement remain follow-on work with explicit diagnostics where the current
  slice encounters them.
- ARI-050 adds a terminal-native XYChart scaling contract: fixed five-row vertical plots,
  three-character category bands, rounded bar heights, stair-step line overlays, ten-character
  horizontal value bars, inferred numeric x labels, and per-line trimming for stable snapshots.
- XYChart legends, ANSI/color output, multi-series spacing, and full-size terminal plot layout remain
  follow-on work.

## Blockers

- None for ARI-050.

## Next Recommended Action

Continue with ARI-060 for flow/state delta triage against `beautiful-mermaid`. A separate follow-on
should handle class/ER multi-relationship graph layout or richer XYChart legends/color if those
become the next priority.
