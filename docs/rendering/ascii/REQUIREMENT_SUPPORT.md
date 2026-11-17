# Requirement Terminal Support

Requirement has a family-owned traceability view over the canonical typed model. One global
graph shows shared endpoints; complete reading records follow it. The projection does not expand
convergences into repeated tree nodes or translate Requirement through Flowchart semantics.

## Traceability and reading records

Requirements have source-ordered R keys, elements have N keys, and relationships have E keys.
The graph shows each declaration once, with its key and name. Names wrap at twelve terminal
cells to keep prose out of the routing grid. Records retain the exact quoted name independently
of the graph label, including Markdown, whitespace, and terminal-safe control escapes.

All seven relationship kinds remain explicit labels. `contains` uses a solid line with a circle
on the source and no target arrow; that circle approximates Mermaid's circled-plus marker.
`copies`, `derives`, `satisfies`, `verifies`, `refines`, and `traces` use dotted lines with target
arrows. These relationships do not create containment frames. Reverse Mermaid syntax is resolved
to its canonical source and target before projection. TB/TD, BT, LR, and RL change geometry only.

Records keep each requirement's type, name, id, text, risk, and verification method, and each
element's name, type, and document reference. Every relationship retains its kind, source, target,
and occurrence order, including parallel edges, duplicates, and self loops. CSS, classes,
accessibility, source title/configuration, and additional serialized fields remain complete
borrowed JSON records. Rendering neither executes links nor claims browser styling.

Duplicate names, names shared by requirement and element collections, unresolved endpoints,
unknown relationship kinds, and unknown directions use a **StructuredText primary**. Separate
declarations are retained, and references disclose every candidate key or an unresolved endpoint.
These measured primaries report `outcome: primary` and no attempted fallback when they fit.
Empty models retain their complete records without inventing a spatial scene.

## Host policies and limits

- Canonical is the admitted layout profile. Strict and Plain-only Adaptive graph presentation
  share the existing routing and capacity limits. Adaptive can recover complete records after an
  admitted planning miss; resource errors and cancellation remain errors.
- Plain width fallback uses complete keyed records. Allow preserves a wide primary; Error
  reports the measured combined extent. No policy drops attributes, relations, or label suffixes.
- ASCII/Unicode structures and Unicode/CJK cell measurement share the same semantics. All five
  primary encodings are available; viewport fallback and Adaptive remain Plain-only.
- Reading records wrap naturally at 80 columns. Authored whitespace survives requested trimming
  and narrower reflow. Canvas padding is removed before records enter the document.
- Spatial preparation, records, encoding, and recovery use the caller's single operation and
  cumulative work ledger. Candidate scopes isolate temporary cell storage; the selected combined
  document is admitted once. No family-specific larger default or fresh work allowance is used.

## Evidence and references

`crates/merman-ascii/tests/requirement_model.rs` owns parser-backed traceability, complete fields, direction, identity,
encoding, viewport, resource, and cancellation behavior. The independent graph route oracle checks
both canonical axes and structural charsets, including all eight edges and both shared targets in
the larger upstream example. `output/requirement.rs` owns the complete serialized record boundary.

The historical R34 prototype duplicated shared nodes and was rejected. The current graph resolves
that failure; its small/typical/dense reading floor is recorded in the
[phase admission report](../ASCII_PHASE_GATE_REPORT.md#requirement-spatial-admission).
Dense routing remains best effort, and records add vertical reading cost. This is Partial support,
not an arbitrary-graph readability guarantee.

The pinned Mermaid Requirement DB and marker source provide semantic evidence. Neither the
copied `mermaid-ascii` oracle nor the inspected moving reference implements Requirement, so there
is no accepted Requirement byte or quality oracle. Trident completion remains a local diagnostic,
separate from the typed-field and independent relationship checks above.
