# State ASCII Support

Status: supported subset

This document describes the current `merman-ascii` state support boundary. The renderer consumes
`merman-model` State terminal views; it does not parse Mermaid text itself.

## Supported

| Capability | Status | Notes |
| --- | --- | --- |
| Diagram family | Supported subset | `stateDiagram` and `stateDiagram-v2` inputs that parse into `StateDiagramRenderModel`. |
| Directions | Supported subset | `LR`, `TD`, Mermaid's `TB` alias, `BT`, and `RL` root directions. `BT` and `RL` are rendered as terminal-native output transforms of the TD/LR layouts. |
| Layout and viewport | Supported subset | Canonical-only; Compact and Auto are rejected during admission. A valid over-width diagram follows Allow, Error, or Plain structured Fallback. Resource, cancellation and required routing failures do not trigger fallback. |
| Default spacing | Supported | Canonical prefers no blank vertical node padding and a one-row rank gap. Endpoint, label and container demand can enlarge these dimensions. Explicit scalar/axis padding and graph gaps are preserved. Fixed pseudo-state symbols keep their own sizes. |
| States | Supported subset | Simple state nodes render as terminal graph nodes. State aliases and descriptions render as visible labels. |
| Start/end pseudo states | Supported approximation | `[*]` start states render as visible start markers and end states render as distinct terminal final-state markers. |
| Fork/join/choice pseudo states | Supported approximation | `<<fork>>` and `<<join>>` states render as synchronization bars. `<<choice>>` states render as visible diamond-like branch nodes. Implementation ids are omitted from terminal output. |
| Transitions | Supported subset | Directed transitions and non-empty labels render through the shared graph route planner. |
| Composite states | Supported subset | Composite states render as group boxes when their children can be mapped cleanly to graph members. Transitions may attach to composite group boundaries. |
| Divider/concurrency regions | Supported approximation | Mermaid divider groups render as vertically stacked terminal sections with horizontal separator lines. Empty regions retain their order, including leading, trailing, consecutive, and nested sections. Section children keep their own graph layout and divider implementation ids are omitted. Transitions can cross intermediate separators transversely while retaining their actual state endpoints and labels. |
| State notes | Supported approximation | Inline and block notes render as terminal note nodes connected with open note edges. Multiline note text is preserved. Mermaid's exact note side placement is approximated by the shared graph layout. |
| Click/href links | Accepted metadata | State link URLs and tooltips do not block ASCII rendering. The primary diagram omits interaction metadata; structured width fallback retains authored details. |
| Character sets | Supported | ASCII and Unicode box-drawing output via `AsciiRenderOptions::ascii()` and `unicode()`. |
| ANSI/HTML color roles | Supported subset | Opt-in `AsciiColorMode` can emit renderer-owned foreground/background roles for state nodes, groups, transitions, and labels. Mermaid `classDef`, `class`, and `style` colors map `color` to text/title, `stroke`/`border` to node/group borders, and `fill`/`background` to node/group backgrounds. |

## Explicitly Unsupported

These features return `AsciiError::UnsupportedFeature` instead of silently dropping semantics:

| Feature | Error feature |
| --- | --- |
| Future or unrecognized state node shapes not mapped by the adapter | `state node shapes` |
| State edge arrow types outside Mermaid's normal state arrowheads | `state arrow types` |
| Directions outside Mermaid's supported direction set | `unsupported state directions` |

Required shared-planner search exhaustion returns `AsciiError::RoutingSearchExhausted`, including
the authored edge index. Fixed-presentation marker capacity failures use
`AsciiError::PortCapacityExceeded`. These are separate from unsupported state semantics and do not
trigger width fallback; see the [graph presentation boundary](../../../crates/merman-ascii/README.md#graph-presentation-boundary).

The shared planner measures cyclic compound-scope return-lane demand before searching routes. Label
widths, overlapping returns, local directions, and container margins participate in the same
request used by measured recovery. A bounded initial route trial keeps a complete incumbent when
the original spacing already works; otherwise the remaining preparation credit can enlarge the
measured geometry. When a direct route is blocked by an existing edge, a bounded detour trial
first searches without displacing the existing routes. It uses half the smaller of the remaining
allocation work and private preparation work, leaving credit for conflict repair. A failed private
trial keeps its work charged; public resource and operation errors still propagate. This preserves
the single-region labeled return at 20 columns and 32 rows without changing the Canonical profile.
Endpoint/container label classification is reused only within one immutable geometry generation.
These steps charge the existing operation ledger and preserve cancellation, deadlines, public
resource ceilings, and bounded search diagnostics.
Root returns and self loops retain their existing exterior-envelope rules, with additional
space requested only after a measured routing miss.

## Known Limitations

- State rendering is a terminal graph approximation, not SVG layout parity.
- Conservative return spacing can widen a scene whose original geometry did not complete within
  the bounded initial trial. It improves capacity rather than guaranteeing the smallest diagram.
- Start/end, fork/join, and choice pseudo states are terminal approximations of Mermaid's SVG
  shapes. They preserve topology and avoid leaking implementation ids, but they are not pixel
  parity with browser rendering.
- Composite groups currently require child-member mapping. Boundary-entry routing is a terminal graph
  approximation and does not attempt SVG cluster-edge parity.
- Divider/concurrency regions are a terminal graph approximation. Sections stack vertically and
  separators span the parent composite state, but spacing is not SVG layout parity. Separators
  are open lines, so transitions can pass through intermediate regions without treating their
  full-width separators as closed containers. Parent and unrelated composite frames retain their
  containment constraints. A primary marker may share a separator cell only when it sits directly
  beside its actual endpoint and crosses the separator transversely; this preserves target ports
  for small nodes at zero padding. Vertical markers keep a reading column beside adjacent labels.
  Divider ids are generated structure rather than authored transition targets; direct typed-model
  edges to Divider groups do not participate in terminal-cell recovery.
  Mixed typed-model scopes place empty regions clear of ordinary peers rather than hiding their
  separators beneath node frames. A separator can also mark the first region below the composite
  title band; separator count alone is not a count of concurrent regions.
- State note side placement is terminal-graph approximate. The note text and note relationship are
  preserved, but Mermaid's exact SVG note offsets are not.
- State links are intentionally omitted from primary diagrams and retained as authored details in
  structured width fallback.
- State style fill/background is emitted only in opt-in ANSI/HTML color modes and is invisible in
  plain text output. Font weight, font style, and other SVG-only presentation metadata are not
  rendered.
- Any future Mermaid state shape variants should be added one semantic shape at a time with
  parser-backed tests.

## Test Coverage

The support boundary is covered by:

- `cargo nextest run -p merman-ascii --features diagram-state --test state_model -j 2`
- `cargo nextest run -p merman-ascii --all-features -j 2`

The parser-backed state tests live in `crates/merman-ascii/tests/state_model.rs` and exercise the public `render_model`
path from Mermaid text through `merman-core` into `merman-ascii`. Region coverage includes empty
sections, intermediate-region skip edges, feedback, independent merge heads, and nested concurrency
with distinct start/end pseudo states.
Canonical density coverage checks short forward/reverse lifecycles, explicit legacy spacing and
the CJK compound lifecycle across both character sets and width conventions, retaining all nine
transition heads and the ordered internal states.
`crates/merman-ascii/tests/graph_family_viewports.rs` additionally checks Canonical-only State viewport selection,
complete note/condition fields, logical family identity and independent display-cell measurements
at 60/80/100/120 columns through the public parsed-report entrypoint. The single-region return
also remains a complete primary diagram at 20 columns, with no profile switch, fallback, lost
transition heads, or additional rows. The original three-region conditions retain a 71-by-79
extent ceiling under Trusted Native limits.
