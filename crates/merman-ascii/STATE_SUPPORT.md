# State ASCII Support

Status: supported subset

This document describes the current `merman-ascii` state support boundary. The renderer consumes
`merman-core` `StateDiagramRenderModel` values; it does not parse Mermaid text itself.

## Supported

| Capability | Status | Notes |
| --- | --- | --- |
| Diagram family | Supported subset | `stateDiagram` and `stateDiagram-v2` inputs that parse into `StateDiagramRenderModel`. |
| Directions | Supported subset | `LR`, `TD`, Mermaid's `TB` alias, `BT`, and `RL` root directions. `BT` and `RL` are rendered as terminal-native output transforms of the TD/LR layouts. |
| States | Supported subset | Simple state nodes render as terminal graph nodes. State aliases and descriptions render as visible labels. |
| Start/end pseudo states | Supported approximation | `[*]` start states render as visible start markers and end states render as distinct terminal final-state markers. |
| Fork/join/choice pseudo states | Supported approximation | `<<fork>>` and `<<join>>` states render as synchronization bars. `<<choice>>` states render as visible diamond-like branch nodes. Implementation ids are omitted from terminal output. |
| Transitions | Supported subset | Directed transitions and non-empty labels render through the shared graph route planner. |
| Composite states | Supported subset | Composite states render as group boxes when their children can be mapped cleanly to graph members. Transitions may attach to composite group boundaries. |
| Divider/concurrency regions | Supported approximation | Mermaid divider groups render as vertically stacked terminal sections with horizontal separator lines. Section children keep their own graph layout and divider implementation ids are omitted. |
| State notes | Supported approximation | Inline and block notes render as terminal note nodes connected with open note edges. Multiline note text is preserved. Mermaid's exact note side placement is approximated by the shared graph layout. |
| Click/href links | Accepted metadata | State link URLs and tooltips are SVG/interaction metadata. They do not block ASCII rendering and are not emitted in terminal output. |
| Character sets | Supported | ASCII and Unicode box-drawing output via `AsciiRenderOptions::ascii()` and `unicode()`. |
| ANSI/HTML color roles | Supported subset | Opt-in `AsciiColorMode` can emit renderer-owned foreground roles for state nodes, groups, transitions, and labels. Mermaid `classDef`, `class`, and `style` foreground colors map `color` to text/title and `stroke`/`border` to node/group borders. |

## Explicitly Unsupported

These features return `AsciiError::UnsupportedFeature` instead of silently dropping semantics:

| Feature | Error feature |
| --- | --- |
| Future or unrecognized state node shapes not mapped by the adapter | `state node shapes` |
| State edge arrow types outside Mermaid's normal state arrowheads | `state arrow types` |
| Directions outside Mermaid's supported direction set | `unsupported state directions` |
| Graph routes the shared route planner cannot represent | `unroutable graph edges` |

## Known Limitations

- State rendering is a terminal graph approximation, not SVG layout parity.
- Start/end, fork/join, and choice pseudo states are terminal approximations of Mermaid's SVG
  shapes. They preserve topology and avoid leaking implementation ids, but they are not pixel
  parity with browser rendering.
- Composite groups currently require child-member mapping. Boundary-entry routing is a terminal graph
  approximation and does not attempt SVG cluster-edge parity.
- Divider/concurrency regions are a terminal graph approximation. Sections stack vertically and
  separators span the parent composite state, but spacing is not SVG layout parity.
- State note side placement is terminal-graph approximate. The note text and note relationship are
  preserved, but Mermaid's exact SVG note offsets are not.
- State links are accepted as interaction metadata and intentionally omitted from terminal output.
- State style fill/background, font weight, font style, and other non-foreground semantics are not
  emitted as terminal foreground colors.
- Any future Mermaid state shape variants should be added one semantic shape at a time with
  parser-backed tests.

## Test Coverage

The support boundary is covered by:

- `cargo nextest run -p merman-ascii state`
- `cargo nextest run -p merman-ascii`

The parser-backed state tests live in `tests/state_model.rs` and exercise the public `render_model`
path from Mermaid text through `merman-core` into `merman-ascii`.
