---
title: "ASCII Tail Enhancements - Plan"
type: "refactor"
date: "2026-06-30"
artifact_contract: "ce-unified-plan/v1"
artifact_readiness: "implementation-ready"
product_contract_source: "ce-plan-bootstrap"
execution: "code"
origin: "direct user request after ASCII reference advantage work"
---

# ASCII Tail Enhancements - Plan

## Goal Capsule

- **Objective:** Finish the remaining high-value ASCII renderer enhancements after the reference-advantage work, focused on Class/ER dense relation routing, Sequence layout expression, verification clarity, and deletion of obsolete code.
- **Authority:** Mermaid semantics and `merman-core` typed render models outrank reference output; `beautiful-mermaid` remains capability prior art; `docs/rendering/ASCII_SUPPORT_MATRIX.md` and `crates/merman-ascii/ASCII_GAP_REGISTRY.md` define the shipped support boundary.
- **Execution profile:** Bounded Rust refactor in `merman-ascii` with focused fixture updates and documentation alignment.
- **Stop conditions:** Pause if an enhancement requires byte-perfect reference mimicry, hidden participants, fake topology, browser-only geometry, or a rewrite outside the Class/ER relation graph and Sequence layout surfaces.
- **Tail ownership:** Every behavioral change updates tests and support docs in the same change set, and the final cleanup pass removes abandoned helpers, stale allowances, or duplicated paths introduced by earlier attempts.

---

## Product Contract

### Summary

The broad ASCII reference-advantage track is now mostly absorbed into runtime capability metadata, public support docs, XYChart value disclosure, Class/ER relation improvements, and Sequence wrapper padding.
The remaining work should not repeat that broad plan.
It should deepen the two places where terminal output still has visible tail risk: Class/ER topologies that summarize before a readable routed grid is proven impossible, and Sequence wrapper combinations where boxes, controls, lifecycles, notes, and mirrored actors can still become hard to scan.

### Problem Frame

Class and ER now share the `relation_graph` seam, including self loops, parallel lanes, independent components, barycenter ordering, and structured `relations:` fallback.
That is the right architecture, but the current fallback boundary is still coarse in two ways.
Some decisions collapse into a general crossing or box-overlap summary, and family adapters receive `LayeredRelationSummaryReason` while currently discarding it at the final summary-row seam.

Sequence output has also moved to a better state: empty boxes are diagram-wide regions, and sequence boxes keep inner padding around participant rows and control frames.
The next useful work is expression quality for dense combinations, not a parser rescue.
The renderer should keep wrapper boundaries stable when nested controls, actor boxes, lifecycle rows, notes, and mirrored participants appear together.

The cleanup pass matters because the recent fearless-refactor path intentionally moved fast through shared seams.
Any remaining `#[allow(dead_code)]`, `_reason` placeholders, duplicated spacing constants, or debug-only helpers must either become real supported seams or be removed.

### Requirements

**Class/ER relation topology**

- R1. Class and ER must keep routing readable dense topologies that fit the configured grid budget instead of falling back from a coarse crossing decision when a deterministic reorder or route candidate can avoid the collision.
- R2. Layered relation fallback reasons must distinguish crossing, route or overlay collision, box overlap, and grid budget well enough for seam tests and support documentation.
- R3. Class and ER adapters must use `LayeredRelationSummaryReason` as a real policy input or documented diagnostic, not discard it as `_reason`.
- R4. Relation enhancements must preserve family semantics: class markers, endpoint labels, ER cardinalities, identifying relationships, multiline labels, colors, and wide-cell labels remain visible.

**Sequence layout expression**

- R5. Sequence boxes and control frames must keep explicit padding and stable borders across nested controls, participant boxes, lifecycle-created actors, mirrored participants, self messages, and notes.
- R6. Dense Sequence output must prefer predictable alignment over maximum compactness when those conflict.
- R7. Unsupported actor presentation metadata remains explicit; the renderer must not invent hidden participants or infer ownership from surrounding messages.

**Verification, docs, and cleanup**

- R8. Local semantic fixtures and focused seam tests must cover each routed-vs-summary and wrapper-boundary decision before broad snapshots change.
- R9. Capability records, support matrix, and gap registry must describe the shipped boundary after each enhancement lands.
- R10. Obsolete code, stale `#[allow(dead_code)]` allowances, unused helper paths, and dead-end experimental fixtures must be removed or converted to test-only code before the work is complete.

### Acceptance Examples

- AE1. Given a readable Class or ER multi-parent topology, the renderer produces a routed grid and no `relations:` section.
- AE2. Given a dense Class or ER topology that cannot be routed without collisions or exceeding the grid budget, the renderer emits `relations:` and a seam test proves the exact fallback reason.
- AE3. Given a Class or ER summary fallback, the family adapter no longer ignores the summary reason behind an `_reason` parameter.
- AE4. Given a Sequence diagram with a participant box around a nested control frame, the outer box, inner frame, participant borders, and lifelines remain visually separated.
- AE5. Given lifecycle-created actors inside a Sequence control or box region, creation rows and later lifelines align with the same participant centers.
- AE6. Given mirrored Sequence actors are enabled with nested wrappers, the bottom participant row stays aligned and does not merge with an outer box border.
- AE7. Given cleanup is complete, every touched `#[allow(dead_code)]` is removed, narrowed to `#[cfg(test)]`, or documented by a real production caller.

### Scope Boundaries

In scope:

- Shared `relation_graph` planning, routing, fallback diagnostics, and Class/ER adapter use of fallback reasons.
- Sequence row planning and wrapper rendering for boxes, controls, lifecycles, notes, self messages, and mirrored actors.
- Local semantic fixtures, seam tests, capability records, support matrix, and gap registry updates.
- Cleanup of obsolete code directly related to the touched ASCII surfaces.

Deferred:

- New Mermaid families beyond the current ASCII support matrix.
- Browser-pixel layout parity, SVG theme parity, or `beautiful-mermaid` byte-level fixture admission.
- A new public Sequence layout option unless tests prove the current default cannot represent the desired behavior.
- Full namespace container rendering for Class diagrams.

Outside this product's identity:

- Copying reference parsers into `merman-ascii`.
- Replacing typed-model rendering with source-text parsing inside ASCII adapters.
- Hiding dense topology behind a fake routed drawing that loses relation meaning.

---

## Planning Contract

### Key Technical Decisions

- KTD1. Try deterministic relation candidates before summary.
  The current barycenter ordering is a good baseline, but the next topology pass should evaluate a small candidate set before declaring a crossing summary.
- KTD2. Score relation candidates on meaning-preserving constraints.
  Prefer zero box overlap, zero overlay collision, lower crossing count, lower grid cell count, then stable source order.
- KTD3. Make fallback reasons part of the seam contract.
  `LayeredRelationSummaryReason` should be specific enough for tests, docs, and family adapters; a generic summary is still the visible output, but the planner reason should not be lost.
- KTD4. Keep Sequence as a row pipeline with explicit wrappers.
  The current event-row planner, control-frame renderer, and sequence-box renderer are understandable seams; deepen them with interval and width planning before considering a larger rewrite.
- KTD5. Cleanup follows green behavior gates.
  Delete obsolete helpers only after the replacement behavior is covered; test-only APIs should be narrowed with `#[cfg(test)]` rather than kept as production dead code.
- KTD6. Reference projects remain evidence, not oracles.
  `beautiful-mermaid` may suggest examples for Class, ER, and Sequence, but local fixtures assert Mermaid-visible meaning rather than copied spacing.

### High-Level Technical Design

Class/ER work flows through the shared layered scene:

1. Build family boxes and relation edges.
2. Generate candidate layer orders and route policies.
3. Score candidates before summary.
4. Draw the selected scene into a box snapshot.
5. Detect route, overlay, and box collisions with a precise `LayeredRelationSummaryReason`.
6. Render either the routed grid or the structured relation summary.

Sequence work stays in the existing wrapper order unless tests prove otherwise:

1. Plan participant centers, visible actors, lifecycles, and message rows.
2. Render control frames around row spans.
3. Render sequence boxes with explicit outer padding and label width accounting.
4. Finish through the same plain, ANSI, or HTML output path.

### Assumptions

- `crates/merman-ascii/src/capability.rs` already exposes the broader ASCII capability record and should be updated rather than duplicated.
- `relations:` is valid supported output for dense Class/ER cases; the target is better boundary judgment, not elimination of summaries.
- `cargo nextest` is the preferred Rust test runner for this repository.
- Documentation remains English, while user-facing task summaries can be Chinese.

### System-Wide Impact

This plan touches shared ASCII seams that both Class and ER depend on.
Small route-policy changes can change multiple local semantic fixtures at once, so the implementation must stage relation planner tests before family snapshots.
Sequence changes are narrower but user-visible because the same row pipeline serves Mermaid Sequence and ZenUML sequence-like output.

### Risks & Dependencies

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Candidate scoring overfits one dense topology | Other Class/ER diagrams become wider or less stable | Add seam tests for routed, summarized, cyclic, multi-parent, and disconnected cases before changing scoring |
| More routing attempts make output too large | Terminal output exceeds grid limits or becomes unreadable | Keep grid budget as a hard gate and score lower cell count after correctness |
| Fallback reason granularity leaks into user output | Docs overpromise internal planner detail | Keep visible summary stable; expose detailed reasons only in tests, support docs, and future diagnostics |
| Sequence wrapper fixes create border merges elsewhere | Dense diagrams look worse despite passing one fixture | Cover box plus control plus lifecycle plus mirror combinations, not only one happy path |
| Cleanup removes a helper still used by a feature-gated path | Build breaks under a feature combination | Run targeted package tests plus binding-feature tests before landing |

---

## Implementation Units

### U1. Characterize the remaining Class/ER route boundary

- **Goal:** Lock the expected routed-vs-summary boundary before planner changes.
- **Requirements:** R1, R2, R4, R8
- **Dependencies:** None
- **Files:** `crates/merman-ascii/tests/class_model.rs`, `crates/merman-ascii/tests/er_model.rs`, `crates/merman-ascii/tests/testdata/local-semantic/class/`, `crates/merman-ascii/tests/testdata/local-semantic/er/`, `crates/merman-ascii/src/relation_graph/layered/scene.rs`, `crates/merman-ascii/ASCII_GAP_REGISTRY.md`
- **Approach:** Add focused seam tests and local semantic fixtures for the remaining topology categories: readable multi-parent layouts, cyclic dense layouts that should summarize, disconnected components that must not share one grid budget, and multiline or wide labels on both routed and summary paths.
  Keep exact snapshots only where the shape is the behavior; otherwise assert visible endpoints, markers, labels, and absence or presence of `relations:`.
- **Test Scenarios:** Multi-parent Class and ER examples route without `relations:`; dense collision examples summarize; `LayeredRelationSummaryReason` is asserted at the seam for crossing, box overlap, and grid budget; wide labels remain visible on both paths.
- **Verification:** The tests describe the current boundary and fail for the specific false-summary or false-routing cases targeted by later units.

### U2. Add deterministic relation candidate scoring

- **Goal:** Reduce unnecessary Class/ER summary fallback by evaluating more than one stable layered layout candidate.
- **Requirements:** R1, R2, R4, R8
- **Dependencies:** U1
- **Files:** `crates/merman-ascii/src/relation_graph/layered/boxes.rs`, `crates/merman-ascii/src/relation_graph/layered/scene.rs`, `crates/merman-ascii/src/relation_graph/layered/route.rs`, `crates/merman-ascii/src/relation_graph/layered/lanes.rs`, `crates/merman-ascii/tests/class_model.rs`, `crates/merman-ascii/tests/er_model.rs`
- **Approach:** Factor the current barycenter ordering into a candidate generator.
  Produce a small deterministic set: original stable order, parent barycenter order, child barycenter refinement, and mirrored tie-break variants where they can reduce crossings.
  Score candidates by hard validity first, then crossing count, route or overlay collision count, cell count, and stable source order.
  Keep summary fallback when no candidate is readable or the selected candidate exceeds `max_grid_cells`.
- **Test Scenarios:** Previously readable multi-parent cases still route; new dense-but-readable fixtures route; cyclic all-to-all style fixtures still summarize; disconnected components do not affect each other's grid budget.
- **Verification:** Class and ER route more readable topologies without increasing summary false negatives for collision-prone diagrams.

### U3. Split and consume relation fallback reasons

- **Goal:** Turn `LayeredRelationSummaryReason` into a real planner diagnostic instead of a discarded adapter parameter.
- **Requirements:** R2, R3, R4, R8, R9
- **Dependencies:** U1, U2
- **Files:** `crates/merman-ascii/src/relation_graph/layered/scene.rs`, `crates/merman-ascii/src/relation_graph/layered/route.rs`, `crates/merman-ascii/src/relation_graph.rs`, `crates/merman-ascii/src/class/render.rs`, `crates/merman-ascii/src/er/render.rs`, `crates/merman-ascii/src/capability.rs`, `docs/rendering/ASCII_SUPPORT_MATRIX.md`, `crates/merman-ascii/ASCII_REFERENCE_COMPARISON.md`
- **Approach:** Preserve the current visible `relations:` summary shape, but refine the internal reason variants where the planner can tell them apart.
  At minimum, separate grid budget, crossing, box overlap, and overlay or route collision when tests can prove the distinction.
  Rename `_reason` parameters in Class and ER adapters and use the reason for focused assertions, capability limits, or future diagnostic text.
- **Test Scenarios:** Box-overlap fallback remains distinct from grid-budget fallback; overlay-driven fallback does not masquerade as a crossing; Class and ER adapter tests prove the reason reaches family code.
- **Verification:** No fallback reason is silently discarded at the shared seam.

### U4. Improve Sequence wrapper expression

- **Goal:** Keep Sequence boxes, control frames, lifecycles, notes, and mirrored participants visually separated in dense diagrams.
- **Requirements:** R5, R6, R7, R8
- **Dependencies:** None
- **Files:** `crates/merman-ascii/src/sequence/plan.rs`, `crates/merman-ascii/src/sequence/control.rs`, `crates/merman-ascii/src/sequence/boxes.rs`, `crates/merman-ascii/src/sequence/layout.rs`, `crates/merman-ascii/src/sequence/text.rs`, `crates/merman-ascii/tests/sequence_model.rs`, `crates/merman-ascii/tests/testdata/local-semantic/sequence/`
- **Approach:** Add dense wrapper fixtures first.
  Then extract the box/control padding and width decisions into a small planning helper if the current inline calculations cannot explain the behavior.
  Preserve row-pipeline ownership: event rows own lifelines and messages, control frames own row spans, and sequence boxes own outer participant or diagram regions.
  Prefer one extra column or row of padding over border merging.
- **Test Scenarios:** Nested control frames inside a participant box remain separated; box labels wider than participant spans expand the box without shifting lifelines; lifecycle-created actors render inside control or box regions without losing centers; mirrored participants align under boxed diagrams; self messages with notes inside wrappers remain readable.
- **Verification:** Dense Sequence fixtures render as stable, readable terminal diagrams with no merged `|+`, `||`, or border-label collisions.

### U5. Align support docs and capability records

- **Goal:** Keep public support claims synchronized with the new topology and Sequence boundaries.
- **Requirements:** R2, R3, R5, R9
- **Dependencies:** U2, U3, U4
- **Files:** `crates/merman-ascii/src/capability.rs`, `docs/rendering/ASCII_SUPPORT_MATRIX.md`, `crates/merman-ascii/ASCII_GAP_REGISTRY.md`, `crates/merman-ascii/ASCII_REFERENCE_COMPARISON.md`, `crates/merman-ascii/README.md`, `crates/merman-ascii/SEQUENCE_SUPPORT.md`
- **Approach:** Update only the claims that changed.
  If Class/ER now route a category, move it from remaining pressure into supported semantics.
  If a category still summarizes, record the exact reason and keep `Summary` as supported output.
  If Sequence wrapper behavior improves, update the support matrix and Sequence support doc with the new tested boundary.
- **Test Scenarios:** Capability tests still match support levels; comparison docs do not claim byte parity with `beautiful-mermaid`; gap registry remaining pressure points to current open work only.
- **Verification:** Runtime capability metadata, public docs, and gap registry tell the same story.

### U6. Delete obsolete code and stale scaffolding

- **Goal:** Finish with a smaller, clearer ASCII codebase after the behavior changes land.
- **Requirements:** R10
- **Dependencies:** U2, U3, U4, U5
- **Files:** `crates/merman-ascii/src/canvas.rs`, `crates/merman-ascii/src/graph/routing/plan.rs`, `crates/merman-ascii/src/relation_graph.rs`, `crates/merman-ascii/src/class/render.rs`, `crates/merman-ascii/src/er/render.rs`, `crates/merman-ascii/src/sequence/boxes.rs`, `crates/merman-ascii/src/sequence/control.rs`
- **Approach:** Review each remaining `#[allow(dead_code)]`, `_reason`, duplicated spacing calculation, and test-only helper in touched modules.
  Remove truly unused functions.
  Move test-only helpers behind `#[cfg(test)]`.
  Collapse duplicated constants or wrapper-width calculations only when the resulting helper has a clear owner.
  Do not delete a compatibility path until a targeted test proves the replacement covers it.
- **Cleanup Candidates:** `Canvas::write_text`, plain/trimmed finish helpers that are only used by tests, `PlannedRouteSegment` if it remains an unused route classification, `relation_graph` test-only helper allowances, Class/ER `_reason` placeholders, and any temporary dense-fixture scaffolding from U1-U4.
- **Test Scenarios:** The package builds without the reviewed dead-code allowances; all tests that previously used test-only helpers still pass through `#[cfg(test)]` APIs or public behavior.
- **Verification:** The final diff contains no abandoned experimental code and no broad helper introduced only for one fixture unless that helper is now part of a documented seam.

---

## Verification Contract

| Gate | Applies to | Done signal |
| --- | --- | --- |
| `cargo fmt --check --all` | All units | Rust formatting is stable. |
| `cargo nextest run -p merman-ascii class er --status-level fail` | U1, U2, U3 | Class/ER planner, family adapters, and semantic fixtures pass. |
| `cargo nextest run -p merman-ascii sequence --status-level fail` | U4 | Sequence wrapper and lifecycle fixtures pass. |
| `cargo nextest run -p merman-ascii --status-level fail` | U1-U6 | Full ASCII package behavior remains green. |
| `cargo nextest run -p merman-bindings-core --features ascii ascii_capabilities ascii_supported_diagrams metadata_json_helpers_return_json_contracts --status-level fail` | U5 | Capability metadata remains compatible with bindings. |
| `git diff --check` | All units | No trailing whitespace or patch artifacts remain. |

---

## Definition of Done

- Class/ER routed-vs-summary behavior is covered by seam tests and local semantic fixtures before and after planner changes.
- Readable dense Class/ER topologies route when a deterministic candidate can avoid collisions inside the grid budget.
- Collision-prone Class/ER topologies still summarize honestly, with a precise internal fallback reason.
- Class and ER adapters no longer discard `LayeredRelationSummaryReason` behind `_reason`.
- Sequence dense wrapper combinations preserve padding, participant alignment, lifecycle rows, notes, and mirrored actors.
- Capability records, support matrix, gap registry, and reference comparison agree on the shipped boundary.
- Cleanup candidates in touched ASCII modules are removed, narrowed to `#[cfg(test)]`, or justified by production callers.
- All verification gates relevant to touched files pass.

### Per-Unit Done Signals

| Unit | Done signal |
| --- | --- |
| U1 | The current Class/ER boundary is locked by tests that fail for the targeted false-summary and false-routing cases. |
| U2 | Candidate scoring routes more readable topologies without routing diagrams that should summarize. |
| U3 | Fallback reasons are specific, tested, and consumed by Class/ER family code or docs. |
| U4 | Dense Sequence wrapper fixtures show stable borders and participant centers. |
| U5 | Public support claims match runtime capability metadata and the gap registry. |
| U6 | Reviewed dead code and stale scaffolding are gone or test-gated. |

---

## Appendix

### Sources / Research

- `docs/plans/2026-06-29-001-refactor-ascii-reference-advantage-plan.md`
- `crates/merman-ascii/ASCII_GAP_REGISTRY.md`
- `crates/merman-ascii/ASCII_REFERENCE_COMPARISON.md`
- `docs/rendering/ASCII_SUPPORT_MATRIX.md`
- `crates/merman-ascii/src/capability.rs`
- `crates/merman-ascii/src/relation_graph.rs`
- `crates/merman-ascii/src/relation_graph/layered/boxes.rs`
- `crates/merman-ascii/src/relation_graph/layered/scene.rs`
- `crates/merman-ascii/src/relation_graph/layered/route.rs`
- `crates/merman-ascii/src/class/render.rs`
- `crates/merman-ascii/src/er/render.rs`
- `crates/merman-ascii/src/sequence/plan.rs`
- `crates/merman-ascii/src/sequence/control.rs`
- `crates/merman-ascii/src/sequence/boxes.rs`
- `crates/merman-ascii/src/sequence/layout.rs`
- `crates/merman-ascii/tests/class_model.rs`
- `crates/merman-ascii/tests/er_model.rs`
- `crates/merman-ascii/tests/sequence_model.rs`
