# Agentflow Terminal Support

Agentflow has a family-owned terminal projection over its canonical typed model. The selected
Mermaid source defines the seven node roles, three relationship operators, containment, and
metadata. Shared graph geometry does not turn Agentflow into a Flowchart semantic model.

## Spatial subset

Plain task, tool, input, reference-document, action, decision, and connector nodes have visible
role labels and family-appropriate terminal contours. Sequence relationships end in an arrow; references
are dotted and headless; failures end in a cross. Labels, edge identities, parallel relationships,
self loops, and authored edge lengths retain their ordinary graph ownership.

Expanded flow frames support nesting, flow endpoints, local directions, and global nodes outside
containers. Spatial admission requires unambiguous acyclic membership and preserves root TD/TB,
BT, LR, or RL direction. Every route still passes shared contact, occupancy, label, and paint
validation. Dense routing can return a bounded planning diagnostic.

## Complete reading records

Semantic metadata, collapsed views, source diagnostics, browser styling/interaction facts,
unsupported shape variants, and ambiguous containment select a **StructuredText primary**. This
is an explicit family presentation, not an unsuccessful spatial scene. Reports retain a measured
primary extent, `outcome: primary`, and `fallback_attempted: false` when it fits.

Records assign source-ordered keys to nodes (N), flows (G), connectors (C), edges (E), and
diagnostics (D). Quoted authored identities and labels remain independent of those keys. Each edge
names its original source and target and distinguishes sequence, reference, and failure semantics.
Flow records retain direct members, local direction, and collapsed state. Shared authored IDs list
all matching records; duplicate or unresolved identities are disclosed explicitly.

Collapsed reading records describe **authored contents**, including hidden children and original
internal relationships. They do not claim the browser's collapsed visible topology or silently
redirect original endpoints. Metadata retains unknown keys, nested types, instruction, model,
params, returns, value, and connectorRef. Connector metadata is descriptive; rendering does not
resolve references or execute tools. Diagnostics, authored styles, accessibility, source
configuration, and titles remain complete records as well.

## Host policies and limits

- Canonical is the admitted layout profile. Strict and Adaptive graph presentation are available;
  Adaptive is Plain-only and can select complete records after a routing or port-capacity miss.
- ASCII and Unicode structure share semantics. The CJK width profile uses ASCII structure and
  preserves authored Unicode text with the selected terminal-cell measurement.
- Allow retains wide output; Error reports its measured width; Plain Fallback emits complete
  records within the requested width or an accurate fit diagnostic. No policy truncates facts.
- Reading records preserve authored whitespace even when trailing-canvas-space trimming is
  requested. Replacing the primary document charges retained cells once and keeps work cumulative.
- Plain, ANSI16, ANSI256, TrueColor, and HTML primary encodings share the same content. Styled
  fallback is rejected by capability preflight.
- Parsing, model preparation, rendering, and fallback retain the caller's operation, resource
  ledger, cancellation, and deadline. No Agentflow-specific budget reset or larger default is used.

## Evidence and reference scope

`crates/merman-ascii/tests/agentflow_model.rs` owns parser-backed roles, operators, containment, metadata, viewport,
resource, and cancellation controls. `crates/merman-ascii/src/output/agentflow.rs` checks complete raw-envelope records,
identity overlap, exact resource admission, and cancellation. The independent route oracle in
`crates/merman-ascii/src/graph/routing/tests/generated.rs` checks the three operators' contacts, markers, continuity,
and final paint in both canonical directions and structural character sets.

The pinned Mermaid Agentflow implementation and syntax guide supply semantic evidence.
`mermaid-ascii` has no Agentflow parser or renderer; it supplies no Agentflow quality oracle.
Trident's documentation cases can measure local completion, but its current terminal observer
does not independently validate Agentflow domain facts or browser-visible collapsed topology.
