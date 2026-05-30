# ASCII Reference Implementation Expansion

Status: Complete
Last updated: 2026-05-30

## Why This Lane Exists

`merman-ascii` already has a model-driven ASCII/Unicode renderer for flowchart and sequence
diagrams. Future terminal output should keep that boundary while learning from two MIT-licensed
reference implementations:

- `AlexanderGrooff/mermaid-ascii` for grid placement, routing, fixtures, and the original ASCII
  product shape.
- `lukilabs/beautiful-mermaid` for extended terminal output ideas such as class, ER, xychart,
  color roles, multiline labels, and reference tests.

The lane exists to turn those references into a tracked implementation plan instead of ad hoc
copying from `repo-ref/`.

## Relevant Authority

- ADRs:
  - `docs/adr/0014-upstream-parity-policy.md`
  - `docs/adr/0065-ascii-output-boundary.md`
- Existing docs:
  - `crates/merman-ascii/README.md`
  - `crates/merman-ascii/FLOWCHART_SUPPORT.md`
  - `crates/merman-ascii/SEQUENCE_SUPPORT.md`
  - `tools/upstreams/REPOS.lock.json`
- Related workstreams:
  - `docs/workstreams/ascii-renderer-productization/`
  - `docs/workstreams/ascii-renderer-compatibility-expansion/`
  - `docs/workstreams/ascii-graph-final-parity/`
  - `docs/workstreams/ascii-sequence-parity/`

## Problem

The crate can render useful flowchart and sequence text output, but the next valuable ASCII targets
are class, ER, and xychart. `beautiful-mermaid` has practical implementations for those targets,
but it also owns its own Mermaid parser and SVG renderer. Pulling that shape directly into `merman`
would violate the model-driven boundary and create a second Mermaid implementation.

## Target State

- Reference source provenance is tracked in repository docs and license files.
- `merman-ascii` consumes `merman-core` typed models for any new diagram support.
- Class, ER, and xychart ASCII work is split into independently validatable slices.
- Useful reference behavior from `beautiful-mermaid` is ported only after it is mapped to
  `merman-core` models and covered by Rust tests.
- Mermaid upstream remains the spec; reference implementations are implementation aids, not product
  compatibility targets.

## In Scope

- README/provenance updates for `mermaid-ascii` and `beautiful-mermaid`.
- MIT license notice tracking for both reference implementations.
- Task ledger for class, ER, and xychart ASCII renderers.
- Optional follow-up triage for flowchart/state deltas that `beautiful-mermaid` supports and
  `merman-ascii` still rejects, such as BT/RL approximations, thick edges, and ANSI/HTML color
  roles.

## Out Of Scope

- A second Mermaid parser inside `merman-ascii`.
- Replacing the Mermaid-parity SVG renderer with `beautiful-mermaid`'s SVG renderer.
- Byte-for-byte compatibility with `beautiful-mermaid` output.
- Claiming support for a Mermaid feature before the typed model preserves enough semantics to render
  it honestly.

## Starting Assumptions

| Assumption | Confidence | Evidence | Consequence if wrong |
| --- | --- | --- | --- |
| Both reference implementations are MIT-licensed. | High | Local `LICENSE` files in `repo-ref/mermaid-ascii` and `repo-ref/beautiful-mermaid`. | Stop derived work until provenance is corrected. |
| `merman-core` already exposes typed class, ER, and xychart render models. | High | `RenderSemanticModel::{Class, Er, XyChart}` exists. | Add typed model work before ASCII rendering. |
| `beautiful-mermaid` should be mined for algorithms and tests, not copied as an architecture. | High | It has its own parser and SVG renderer, which conflicts with ADR 0014/0065. | Revisit the boundary with a new ADR before implementation. |
| Class/ER/xychart ASCII can be delivered as separate vertical slices. | Medium | Their typed models and reference renderers are mostly independent. | Split further by diagram feature subsets. |

## Architecture Direction

Keep `merman-ascii` deep at the public interface:

```text
Mermaid text -> merman-core typed model -> merman-ascii diagram renderer -> text output
```

New diagram renderers should live behind the existing `render_model` interface and convert typed
models into small internal text-rendering models. The reference implementations may guide layout,
shape drawing, routing, and fixtures, but parsing remains owned by `merman-core`.

For class and ER, start with readable boxes and relationship markers. For xychart, start with
terminal-native axes, bars, and line plots. Each slice should prefer deterministic snapshots over
visual claims.

## Closeout Condition

This lane can close when:

- reference provenance and license notices are complete,
- at least the planned class, ER, and xychart slices have either shipped or been split into narrower
  follow-on workstreams,
- `merman-ascii` support docs reflect the shipped behavior,
- focused `merman-ascii` gates and broad ASCII feature gates pass,
- and any deferred behavior has a clear owner or non-goal.

Closeout status: satisfied on 2026-05-30. The lane shipped tracked MIT provenance for
`mermaid-ascii` and `beautiful-mermaid`, model-driven class, ER, and XYChart ASCII slices, graph
delta triage with typed thick-edge support, and public `merman`/CLI integration for the shipped
terminal-text families. Remaining work is explicitly deferred to follow-on candidates: class/ER
multi-relationship graph layout, true BT/RL graph transforms, subgraph direction overrides,
style/color roles, state graph text rendering, uncommon flowchart shapes, and richer XYChart
terminal layout.
