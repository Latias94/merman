# ASCII Color Role API - Handoff

Status: Closed
Last updated: 2026-05-30

## Current State

The workstream is closed. ADR 0067 accepted the public color role API shape and the
`AsciiRenderOptions` migration. ACR-030 implemented the shared foreground-color substrate:
public color types, color options, role-aware `Canvas` storage, and forced ANSI/HTML finalizers.
ACR-040 assigned flowchart semantic roles for nodes, groups, edges, labels, arrowheads, and routed
junctions. ACR-050 split broader family adoption into smaller lanes in `FAMILY_ADOPTION_PLAN.md`.
ACR-051 added the shared role-aware trim substrate in `Canvas` plus role-bearing relation graph
lines. ACR-052 adopted semantic roles for class and ER boxes, relation lines, markers, labels, and
junctions. ACR-053 adopted semantic roles for XYChart titles/text, axes, bars, and line plots using
`ChartSeries(index)` for plotted data. ACR-054 adopted semantic roles for sequence participants,
lifelines, activations, messages, notes, boxes, and control frames. ACR-060 added the flowchart
foreground style-mapping subset for `classDef`, `class`, inline `style`, and `linkStyle`.
Default plain output remains unchanged.

## Final Task State

- Task ID: ACR-060
- Owner: codex
- Files: `crates/merman-ascii/src/graph`, `crates/merman-ascii/tests/flowchart_model.rs`,
  `crates/merman-ascii/FLOWCHART_SUPPORT.md`
- Validation: `cargo nextest run -p merman-ascii --test flowchart_model flowchart_style_color`
- Status: DONE
- Review: Do not silently misrepresent unsupported CSS properties
- Evidence: `EVIDENCE_AND_GATES.md`

## Decisions Since Last Update

- Default output should remain plain text and byte-for-byte compatible.
- `Auto` color mode should be opt-in because it depends on environment detection.
- The first implementation should be foreground-only roles; background/fill is a follow-on.
- `AsciiColorRole` should be non-exhaustive.
- `AsciiColorTheme` should have private fields and builder methods.
- Mermaid style mapping should not be bundled with the first role-canvas slice.
- ADR 0067 accepts a pre-1.0 `AsciiRenderOptions` migration: add color fields, keep `Copy`, add
  builder methods, and mark the struct `#[non_exhaustive]`.
- ACR-030 keeps diagram layout role-agnostic. The graph renderer now uses `Canvas::finish_with_options`
  only at final output boundaries, while transformed intermediate canvases still use plain
  finalization.
- ACR-040 moved flowchart drawing to semantic role helpers. `OutputTransform` now preserves canvas
  roles before redrawing transformed labels/titles.
- Flowchart roles cover node text, node borders, group borders/titles, edge lines, edge labels,
  arrowheads, and route junctions.
- ACR-050 decided to split broader family adoption. Class and ER share relation graph string boxes
  and layered `Canvas` routing; sequence and XYChart use different string/char-grid output paths.
- The next substrate now exists in `Canvas::finish_trimmed_with_options` and
  `RelationGraphLine`; class and ER now use it for colored output while preserving the old plain
  rendering path.
- XYChart now uses role-aware chart lines and cells. Titles, tick labels, category labels, and value
  suffixes use `Text`; axis glyphs use `ChartAxis`; bars and line plots use `ChartSeries(index)`.
- Sequence now uses role-aware row buffers. Participant/note/box/control borders use
  `SequenceFrame`, inactive lifelines use `SequenceLifeline`, active lifelines use
  `SequenceActivation`, message labels use `EdgeLabel`, message lines use `EdgeLine`, arrowheads
  use `EdgeArrow`, and message junctions use `Junction`.
- Flowchart Mermaid style mapping now resolves `classDef`, `class`, inline node/subgraph `style`,
  and `linkStyle` into direct foreground colors. Supported style properties are `color` for
  text/title/edge labels and `stroke` for node/subgraph borders plus edge lines/arrows. `fill` and
  backgrounds remain documented no-ops.

## Blockers

- None.

## Next Recommended Action

- Split a dedicated background/fill styling lane if that becomes a product goal. No further work is
  required inside `ascii-color-role-api`.
