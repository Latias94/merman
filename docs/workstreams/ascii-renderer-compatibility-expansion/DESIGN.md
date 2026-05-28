# ASCII Renderer Compatibility Expansion - Design

Status: Complete
Last updated: 2026-05-28

## Problem

`merman-ascii` is now shippable for basic flowcharts and sequence diagrams, but common Mermaid
inputs still fail with explicit unsupported-feature errors. That is the right behavior for the
first productization lane, but downstream library and CLI users will quickly hit high-frequency
flowchart constructs such as edge labels, dotted/open edges, diamond nodes, and subgraphs.

## Target State

ASCII and Unicode output should cover the most common flowchart constructs without silently losing
meaning:

- Edge labels render visibly on the routed connection.
- Open, dotted, and length-modified edges have deterministic text approximations.
- Common non-rectangular node shapes render with stable, documented terminal approximations.
- Subgraphs render as titled group boxes around their supported member nodes.
- Unsupported constructs continue to return structured `AsciiError::UnsupportedFeature` errors.

The target is readable terminal output, not byte-for-byte SVG parity or exact shape geometry.

## Scope

Primary scope:

- `crates/merman-ascii/src/graph/**`
- `crates/merman-ascii/tests/**`
- `crates/merman-ascii/FLOWCHART_SUPPORT.md`
- `crates/merman-ascii/README.md`
- `crates/merman-cli/tests/**` only if CLI smoke coverage needs an update

Supporting docs:

- `docs/workstreams/ascii-renderer-compatibility-expansion/**`
- `CHANGELOG.md` when user-visible support expands

## Non-Goals

- Do not add a second Mermaid parser.
- Do not make ASCII layout depend on SVG coordinates.
- Do not claim full Mermaid flowchart compatibility.
- Do not implement ELK, handDrawn, icon/image, click/callback, or style/class rendering in this
  lane.
- Do not broaden sequence support in the first implementation slice unless flowchart scope is
  already complete and verified.

## Product Policy

Terminal output has less visual vocabulary than SVG. The renderer should therefore use explicit,
documented approximations:

- Rectangular and rounded shapes may share the same box outline when the semantic difference is not
  useful in plain text.
- Decision-like shapes should use a distinct text outline when it improves readability.
- Edge labels should be preserved even when exact placement differs from SVG.
- Edge style differences may be approximated by character choice, but direction and label meaning
  must be preserved.
- Subgraphs should prioritize title, membership, and containment over pixel-level Mermaid layout.

When an approximation would misrepresent the diagram, keep the feature unsupported until a better
representation is designed.

## Architecture Direction

The graph renderer should continue to consume `FlowchartV2Model` through the adapter in
`crates/merman-ascii/src/graph/mod.rs`.

The next slices should extend the internal graph model instead of passing `merman-core` details deep
into the drawing code:

- Add edge metadata for label, stroke style, arrow kind, and requested length.
- Add node shape metadata with a small terminal-shape enum.
- Add optional group metadata for subgraphs.

The drawing layer should stay deterministic and snapshot-tested. Golden fixtures should prefer
small Mermaid inputs that users can understand at a glance.

## Risks

| Risk | Mitigation |
| --- | --- |
| Shape approximations become unstable user-visible behavior. | Document mappings in `FLOWCHART_SUPPORT.md` and snapshot all supported shapes. |
| Subgraphs force a layout rewrite. | Start with containment around already-laid-out nodes; split complex nested routing if needed. |
| Edge labels collide with boxes in dense diagrams. | Support simple adjacent edges first; keep complex routing unsupported or guarded by tests. |
| Scope expands into full flowchart parity. | Keep this lane to high-frequency terminal readability, not complete SVG parity. |

## Exit Criteria

This lane can close when the first Flowchart compatibility slice is implemented, documented,
validated, and either remaining flowchart gaps are listed as explicit follow-ons or the support
matrix clearly marks them unsupported.

Closeout status: satisfied on 2026-05-28. The lane shipped edge labels, common edge variants,
common node-shape approximations, simple titled subgraphs, product examples, CLI smoke coverage, and
fresh closeout evidence.
