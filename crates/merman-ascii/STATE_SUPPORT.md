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
| Start/end pseudo states | Supported approximation | `[*]` start and end states render as visible `*` nodes so transitions remain inspectable in text output. |
| Transitions | Supported subset | Directed transitions and non-empty labels render through the shared graph route planner. |
| Composite states | Supported subset | Composite states render as group boxes when their children can be mapped cleanly to graph members and transitions do not target the composite group itself. |
| Character sets | Supported | ASCII and Unicode box-drawing output via `AsciiRenderOptions::ascii()` and `unicode()`. |
| ANSI/HTML color roles | Supported subset | Opt-in `AsciiColorMode` can emit renderer-owned foreground roles for state nodes, groups, transitions, and labels. Mermaid state style/class metadata remains deferred. |

## Explicitly Unsupported

These features return `AsciiError::UnsupportedFeature` instead of silently dropping semantics:

| Feature | Error feature |
| --- | --- |
| State notes and note edges | `state notes` |
| Click/href links | `state links` |
| `classDef`, `class`, `style`, compiled CSS, and label styles | `state styles` |
| Divider/concurrency regions | `state dividers` |
| Transitions whose endpoint is a composite group container | `state group transition endpoints` |
| State node shapes outside `rect`, `rectWithTitle`, `stateStart`, `stateEnd`, and `roundedWithTitle` | `state node shapes` |
| State edge arrow types outside Mermaid's normal state arrowheads | `state arrow types` |
| Directions outside Mermaid's supported direction set | `unsupported state directions` |
| Graph routes the shared route planner cannot represent | `unroutable graph edges` |

## Known Limitations

- State rendering is a terminal graph approximation, not SVG layout parity.
- Start and end pseudo states both render as `*`; their direction is communicated by transitions.
- Composite groups currently require child-member mapping. Edges to or from a composite group
  container are rejected until graph routing can attach to group boundaries honestly.
- State notes, links, and styles are model metadata in the typed state model, but are not represented
  in the initial terminal output.
- Broader state-specific shape support should be added one semantic shape at a time with
  parser-backed tests.

## Test Coverage

The support boundary is covered by:

- `cargo nextest run -p merman-ascii state`
- `cargo nextest run -p merman-ascii`

The parser-backed state tests live in `tests/state_model.rs` and exercise the public `render_model`
path from Mermaid text through `merman-core` into `merman-ascii`.
