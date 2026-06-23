---
title: "refactor: ASCII Capability Parity and State Adapter"
type: "refactor"
date: "2026-06-23"
origin: "direct user request; repo-ref/beautiful-mermaid and repo-ref/mermaid-ascii comparison"
---

# refactor: ASCII Capability Parity and State Adapter

## Summary

This plan turns the current ASCII support surface into an accurate, reference-backed capability
contract, then closes the largest practical family gap by adding a `stateDiagram` ASCII adapter on
top of the existing graph renderer.

It complements `docs/plans/2026-06-23-002-refactor-ascii-architecture-deepening-plan.md`. That plan
owns deep graph routing, terminal-cell, relation, sequence-row, and shape-planning refactors. This
plan owns capability truthing, reference comparison, and the first additional diagram family.

---

## Problem Frame

`repo-ref/mermaid-ascii` is narrow but strong inside its chosen scope: graph/flowchart and simple
sequence output with copied fixture parity. `merman-ascii` already meets and extends that scope with
Flowchart, Sequence, Class, ER, and XYChart support.

`repo-ref/beautiful-mermaid` is broader. Its ASCII renderer covers Flowchart, State, Sequence,
Class, ER, and XYChart, with a diagram-family dispatch layer and family-local renderers. Compared
with it, the most visible `merman-ascii` family gap is `stateDiagram`: `merman-core` already exposes
a typed `StateDiagramRenderModel`, but `crates/merman-ascii/src/lib.rs` still returns
`AsciiError::UnsupportedDiagram` for `RenderSemanticModel::State`.

The second gap is truthfulness. The shipped docs and gap registry still contain stale sequence
claims such as unsupported nested control blocks and wrapped actor labels, while recent code and
tests have moved ahead. Future ASCII work needs a single, current comparison contract so refactors
do not chase old unsupported entries or overclaim parity.

---

## Capability Comparison

| Diagram family | `mermaid-ascii` | `beautiful-mermaid` | Current `merman-ascii` | Plan stance |
| --- | --- | --- | --- | --- |
| Flowchart / graph | Strong copied fixture scope for graph layout and sequence-adjacent CLI behavior | Supported with richer shape registry | Supported with copied fixture parity plus Mermaid typed-model extensions | Keep under existing architecture-deepening plan; document exact capability boundary |
| Sequence | Basic participants, aliases, messages, autonumber | Broad control blocks, notes, actor presentation, activations | Broad subset with notes, boxes, lifecycle, activations, control blocks, actor types | Fix stale support docs; keep actor links/properties as explicit gap |
| State | Not a core shipped family | Supported | Typed model exists in `merman-core`, ASCII unsupported | Add supported subset through a state-to-graph adapter |
| Class | Not in v1 fixture gate | Supported | Supported subset | Keep documented as merman extension beyond `mermaid-ascii` |
| ER | Not in v1 fixture gate | Supported | Supported subset | Keep documented as merman extension beyond `mermaid-ascii` |
| XYChart | Not in v1 fixture gate | Supported | Supported subset | Keep existing support; defer richer plot/legend work |

The plan does not attempt byte-for-byte compatibility with `beautiful-mermaid`. It uses that project
as architecture and capability prior art, while Mermaid semantics and the typed models in
`merman-core` remain the local source of truth.

---

## Requirements

**Capability Contract**

- R1. The ASCII README, support matrices, and gap registry must describe shipped behavior, not stale
  workstream assumptions.
- R2. Reference-project comparison must distinguish three scopes: copied `mermaid-ascii` fixture
  parity, `beautiful-mermaid` capability prior art, and `merman-ascii` typed-model behavior.
- R3. Implemented sequence features must be removed from open gap entries only when support docs and
  tests show their current shipped boundary.
- R4. Remaining unsupported behavior must stay explicit through `AsciiError::UnsupportedFeature` or
  a documented unsupported-family boundary.

**State Diagram ASCII**

- R5. `render_model` must route `RenderSemanticModel::State` to a state ASCII renderer instead of
  returning `UnsupportedDiagram`.
- R6. The initial state renderer must support simple states, start/end pseudo states, labeled
  transitions, root direction, descriptions, and composite-state boxes where they map cleanly to the
  shared graph renderer.
- R7. State notes, links, class/style metadata, dividers, and uncommon state node shapes must be
  rendered through existing graph/text/color primitives, accepted as documented omitted metadata, or
  rejected with a precise unsupported feature.
- R8. The state adapter must preserve the model-driven ASCII boundary: no state Mermaid parsing is
  copied into `merman-ascii`.

**Verification**

- R9. New state tests must parse Mermaid text through `merman-core`, render with `merman-ascii`, and
  assert the public `render_model` path.
- R10. Capability docs must name the validation gates for `mermaid-ascii` copied fixtures,
  state-model tests, and full package tests.

---

## Scope Boundaries

In scope:

- Documentation and registry cleanup for ASCII capability comparison.
- A new `stateDiagram` ASCII adapter that converts `StateDiagramRenderModel` into the existing
  graph rendering model when semantics are representable.
- Focused tests for simple state diagrams, labels, pseudo states, composite states, and explicit
  unsupported state features.
- README matrix updates so users can see where `merman-ascii` is ahead of `mermaid-ascii`, behind
  `beautiful-mermaid`, or deliberately different.

Deferred:

- Deep graph route-plan and terminal-cell refactors owned by
  `docs/plans/2026-06-23-002-refactor-ascii-architecture-deepening-plan.md`.
- Full `beautiful-mermaid` state parity, including all visual shape variants and note placement
  nuance.
- New ASCII families beyond state diagrams.
- Richer XYChart legends and scalable terminal plot area.

Out of scope:

- Copying reference parser code into `merman-ascii`.
- Pixel-perfect output matching with `beautiful-mermaid`.
- Hiding unsupported state semantics by approximating links, note groups, or divider behavior
  without a documented rule.

---

## Key Technical Decisions

- KTD1. Add state support as an adapter, not a second graph renderer. State diagrams already arrive
  as a typed layout-ready model, and the existing graph renderer owns text boxes, directions,
  groups, labels, arrows, and color roles.
- KTD2. Treat `beautiful-mermaid` as a capability map, not an oracle. Its broad family support helps
  prioritize state, but local Mermaid model semantics decide the implementation boundary.
- KTD3. Keep state unsupported cases precise. Replacing `UnsupportedDiagram { state }` with a broad
  renderer must not silently drop dividers, group endpoints, or unsupported shapes. State note
  edges are representable as open terminal connectors once their internal note node is collapsed
  into the visible note group. State links are interaction metadata and can be accepted when their
  omission is documented. State style metadata can map only the foreground subset that the graph
  renderer can express.
- KTD4. Do documentation truthing first. A stale gap registry makes later fearless refactors riskier
  because implementers cannot tell whether a gap is real, solved, or intentionally deferred.
- KTD5. Preserve the copied `mermaid-ascii` fixture gate. Broader state/class/ER/XYChart support is
  a merman extension and should not alter the v1 copied-fixture contract.

---

## Implementation Units

### U1. Reconcile ASCII capability docs and gap registry

- **Goal:** Make the published capability surface match current code and the reference comparison.
- **Requirements:** R1, R2, R3, R4, R10
- **Dependencies:** None
- **Files:**
  - `crates/merman-ascii/README.md`
  - `crates/merman-ascii/SEQUENCE_SUPPORT.md`
  - `crates/merman-ascii/FLOWCHART_SUPPORT.md`
  - `crates/merman-ascii/ASCII_GAP_REGISTRY.md`
  - `crates/merman-ascii/V1_MERMAID_ASCII_COVERAGE.md`
  - `crates/merman-ascii/tests/testdata/mermaid-ascii/README.md`
- **Approach:** Update the shipped matrix and gap registry from observed code behavior. Close or
  revise stale sequence gaps for nested control blocks, wrapped actor labels, and supported actor
  types; keep actor links/properties and genuinely unsupported presentation metadata open.
- **Patterns to follow:** Existing package matrix in `crates/merman-ascii/README.md`; registry row
  style in `crates/merman-ascii/ASCII_GAP_REGISTRY.md`; copied fixture gate language in
  `crates/merman-ascii/V1_MERMAID_ASCII_COVERAGE.md`.
- **Test scenarios:**
  - The README matrix explains that state is the next supported family once U2-U4 land.
  - The sequence support doc no longer lists features that current tests prove supported.
  - The gap registry distinguishes solved, narrowed, and still-open sequence gaps.
  - The v1 copied-fixture contract remains graph/flowchart plus sequence only.
- **Verification:** A reviewer can compare the README and gap registry against current tests without
  finding contradicted unsupported claims.

### U2. Introduce a state-to-graph adapter boundary

- **Goal:** Create a state adapter that maps representable `StateDiagramRenderModel` nodes, groups,
  and edges into `AsciiGraph`.
- **Requirements:** R5, R6, R7, R8
- **Dependencies:** U1
- **Files:**
  - `crates/merman-ascii/src/lib.rs`
  - `crates/merman-ascii/src/state/mod.rs`
  - `crates/merman-ascii/src/state/adapter.rs`
  - `crates/merman-ascii/src/graph/model.rs`
  - `crates/merman-ascii/src/graph/mod.rs`
  - `crates/merman-core/src/diagrams/state/render_model.rs`
- **Approach:** Add `render_state` and route `RenderSemanticModel::State` through it. The adapter
  should map `rect`, `rectWithTitle`, `stateStart`, `stateEnd`, and `roundedWithTitle` into current
  graph node/group shapes. It should reject state shapes or metadata that cannot be represented
  honestly yet. State note groups may be collapsed into terminal note nodes when their text and
  note relationship are preserved. State links may be accepted as omitted metadata because URLs and
  tooltips have no terminal topology. State `classDef`, `class`, and `style` foreground colors may
  map through shared graph style declarations.
- **Patterns to follow:** Flowchart adapter in `crates/merman-ascii/src/graph/adapter.rs`; graph
  model constructors in `crates/merman-ascii/src/graph/model.rs`; state typed model fields in
  `crates/merman-core/src/diagrams/state/render_model.rs`.
- **Test scenarios:**
  - `stateDiagram-v2\nA --> B: go` renders through `render_model`.
  - `stateDiagram-v2\n[*] --> A\nA --> [*]` renders start/end pseudo states instead of failing as
    an unsupported diagram.
  - A state with a description renders the description as the node label or a stable multiline
    label.
  - A composite state with a child node renders as a graph group when the model provides group
    membership.
  - Unsupported state divider behavior returns `UnsupportedFeature` with `diagram_type: "state"`,
    while state notes render as terminal note nodes, state links are accepted as omitted metadata,
    and foreground state styles map to node/group text and border colors.
- **Verification:** `render_model` no longer returns `UnsupportedDiagram { diagram_type: "state" }`
  for the supported state subset.

### U3. Add parser-backed state ASCII tests

- **Goal:** Cover the public state ASCII path with fixtures that reflect Mermaid typed-model
  semantics, not hand-built adapter internals only.
- **Requirements:** R5, R6, R7, R9, R10
- **Dependencies:** U2
- **Files:**
  - `crates/merman-ascii/tests/state_model.rs`
  - `crates/merman-ascii/src/state/adapter.rs`
  - `crates/merman-ascii/src/lib.rs`
  - `crates/merman-ascii/README.md`
  - `crates/merman-ascii/ASCII_GAP_REGISTRY.md`
- **Approach:** Follow the existing family model-test pattern: parse Mermaid text with
  `Engine::parse_diagram_for_render_model_sync`, render via `render_model`, and assert deterministic
  ASCII/Unicode output or explicit unsupported errors.
- **Patterns to follow:** `crates/merman-ascii/tests/flowchart_model.rs`;
  `crates/merman-ascii/tests/class_model.rs`; `crates/merman-ascii/tests/xychart_model.rs`.
- **Test scenarios:**
  - Simple two-state transition with a label.
  - Root `LR` and default `TB` directions.
  - Start and end pseudo states.
  - State alias/description syntax from `crates/merman-core/src/tests/state.rs`.
  - Composite state group with one or more child states.
  - State notes, links, and foreground style metadata render through the public parser-backed path,
    while unsupported divider metadata stays explicit.
- **Verification:** `cargo nextest run -p merman-ascii state` exercises the new state family, and
  `cargo nextest run -p merman-ascii` keeps existing families stable.

### U4. Publish the updated state support boundary

- **Goal:** Make state support discoverable without overclaiming full `beautiful-mermaid` parity.
- **Requirements:** R1, R2, R4, R7, R10
- **Dependencies:** U2, U3
- **Files:**
  - `crates/merman-ascii/README.md`
  - `crates/merman-ascii/ASCII_GAP_REGISTRY.md`
  - `crates/merman-ascii/STATE_SUPPORT.md`
  - `crates/merman-ascii/FLOWCHART_SUPPORT.md`
- **Approach:** Add a focused `STATE_SUPPORT.md` once the adapter lands. Update README and gap
  registry to list state as a supported subset, not as an unsupported family. Leave unsupported
  state features as precise follow-up gaps.
- **Patterns to follow:** `crates/merman-ascii/SEQUENCE_SUPPORT.md`;
  `crates/merman-ascii/FLOWCHART_SUPPORT.md`.
- **Test scenarios:**
  - README shipped matrix includes `stateDiagram` with public entry points.
  - `STATE_SUPPORT.md` lists supported state syntax and explicit unsupported state metadata.
  - `A-FAMILY-010` is narrowed or replaced so state is no longer described as the first open family.
  - Validation gates name `cargo nextest run -p merman-ascii state` and the full package test.
- **Verification:** A user reading docs can predict which state diagrams render and which still
  fail with explicit unsupported-feature diagnostics.

---

## Acceptance Examples

- AE1. Given `stateDiagram-v2\nA --> B: go`, `render_model` returns a deterministic ASCII diagram
  instead of `UnsupportedDiagram`.
- AE2. Given start/end pseudo states, the state adapter renders visible terminal approximations and
  preserves transition direction.
- AE3. Given a composite state that the typed model exposes as a group, the ASCII output contains a
  group box rather than flattening the child state into the root graph.
- AE4. Given state notes, rendering preserves the note text and relationship as a terminal note
  node; given state links, rendering keeps the graph visible and omits URLs from terminal output;
  given unsupported metadata, rendering returns
  `AsciiError::UnsupportedFeature { diagram_type: "state", ... }`.
- AE5. Given the README and support docs after U4, Flowchart, Sequence, State, Class, ER, and
  XYChart all have an explicit shipped subset or explicit deferred boundary.
- AE6. Given the `mermaid-ascii` v1 fixture contract, its copied graph and sequence gates remain
  unchanged by state work.

---

## System-Wide Impact

The public ASCII crate gains one additional supported typed family. CLI and higher-level `merman`
ASCII entry points that already call `render_model` should benefit automatically when they parse a
state diagram through `merman-core`.

This plan intentionally changes user-visible behavior for state diagrams from unsupported-family
failure to supported-subset rendering. It should not change existing Flowchart, Sequence, Class, ER,
or XYChart output except documentation text and any test naming needed to reflect the updated
capability matrix.

---

## Risks & Mitigation

| Risk | Impact | Mitigation |
| --- | --- | --- |
| State-to-graph mapping flattens state semantics | Composite or note behavior may look supported while losing meaning | Reject unsupported state metadata explicitly and add state support docs before broad claims |
| Existing graph renderer limitations leak into state | Some state diagrams may fail even though graph-like | Keep U2 as a supported subset and route deeper graph defects to the existing architecture plan |
| Documentation cleanup closes a real gap by accident | Future work may lose a needed tracking item | Tie each closed or narrowed gap to existing tests and leave remaining unsupported features explicit |
| `beautiful-mermaid` comparison becomes a parity promise | Users may expect full family/shape parity | State docs must say source-backed subset, not byte-for-byte or feature-complete parity |
| New state tests are too snapshot-heavy | Snapshot churn can hide adapter errors | Prefer semantic assertions plus small deterministic output examples |

---

## Sources / Research

- `docs/plans/2026-06-23-002-refactor-ascii-architecture-deepening-plan.md`: Existing deep ASCII
  architecture plan.
- `crates/merman-ascii/src/lib.rs`: Current ASCII family dispatch and unsupported-state boundary.
- `crates/merman-ascii/README.md`: Current shipped family matrix.
- `crates/merman-ascii/SEQUENCE_SUPPORT.md`: Current stale and current sequence support claims.
- `crates/merman-ascii/ASCII_GAP_REGISTRY.md`: Current ASCII gap registry and state-family gap.
- `crates/merman-ascii/V1_MERMAID_ASCII_COVERAGE.md`: Copied `mermaid-ascii` fixture contract.
- `crates/merman-core/src/diagrams/state/render_model.rs`: Typed state model available to ASCII.
- `crates/merman-core/src/diagrams/state/db.rs`: State shape, group, note, and edge model creation.
- `crates/merman-core/src/tests/state.rs`: Parser-backed state semantics and edge cases.
- `crates/merman-ascii/src/graph/adapter.rs`: Flowchart-to-graph adapter pattern.
- `crates/merman-ascii/src/graph/model.rs`: Shared graph model usable by state.
- `repo-ref/beautiful-mermaid/README.md`: Reference broad family coverage.
- `repo-ref/beautiful-mermaid/src/index.ts`: Reference family dispatch.
- `repo-ref/beautiful-mermaid/src/ascii/index.ts`: Reference ASCII renderer family coverage.
- `repo-ref/beautiful-mermaid/src/ascii/shapes/index.ts`: Reference shape registry.
- `repo-ref/mermaid-ascii/README.md`: Reference narrow graph/sequence scope.
- `repo-ref/mermaid-ascii/cmd/parse.go`: Reference graph parser and CLI scope.
- `repo-ref/mermaid-ascii/pkg/sequence/parser.go`: Reference sequence parser subset.
