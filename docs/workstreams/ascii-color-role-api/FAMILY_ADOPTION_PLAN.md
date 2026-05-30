# ASCII Color Role API - Family Adoption Plan

Status: Accepted
Date: 2026-05-30

## Decision

Split broader diagram-family color adoption into smaller lanes instead of assigning roles across
sequence, class, ER, and XYChart in one change.

Flowchart is already the first vertical slice. The remaining families have different output
surfaces:

- class and ER share `relation_graph` string boxes and layered `Canvas` routing;
- sequence builds and overlays plain `String` rows across multiple modules;
- XYChart uses local character grids and has series-specific role requirements.

One broad adoption task would mix role storage, trimmed finalization, relation graph refactoring,
series mapping, and sequence lifecycle overlays. That would raise regression risk and make plain
snapshot failures hard to localize.

## Family Matrix

| Family | Current output shape | Role adoption risk | Decision |
| --- | --- | --- | --- |
| Flowchart | `Canvas` with role-aware finalization | Low after ACR-030 | DONE in ACR-040. |
| Class | `RelationGraphBox` role lines plus layered `Canvas` routes | Medium; boxes and routes use different paths | DONE in ACR-052. |
| ER | Same relation graph substrate as class | Medium; cardinalities add relationship text roles | DONE in ACR-052. |
| XYChart | Local role-aware chart line/cell buffers finalized through `Canvas` | Medium; series overlays must keep role ownership stable | DONE in ACR-053. |
| Sequence | Role-aware row buffers with overlays, frames, notes, lifelines, and activations | High; many row builders and lifecycle overlays | DONE in ACR-054. |

## Lane Order

1. ACR-051: Add a shared role-aware text/trim substrate for non-flowchart renderers.
   - Keep default plain output identical.
   - Support trimming trailing unstyled spaces before ANSI/HTML finalization.
   - Decide whether `RelationGraphBox` should become role-aware or be drawn into `Canvas` earlier.

2. ACR-052: DONE. Adopt roles for class and ER together.
   - They share relation graph layout primitives and relationship routing semantics.
   - Roles should cover entity/class text, box borders, relation lines, markers, labels, and
     junctions.

3. ACR-053: DONE. Adopt roles for XYChart.
   - Roles should cover chart titles/text, axes, and `ChartSeries(index)` for bars and line plots.
   - This validates the series-index API from ADR 0067.

4. ACR-054: DONE. Adopt roles for sequence.
   - Roles should cover participant text/borders, lifelines, activations, messages, notes, sequence
     boxes, and control frames.
   - Keep Mermaid `rect`/box fill color interpretation deferred because background/fill remains out
     of scope for the foreground-only API.

5. ACR-060: DONE. Flowchart Mermaid foreground style/class/linkStyle mapping.
   - `classDef`, `class`, inline `style`, and `linkStyle` now map safe foreground semantics.
   - Fill/background interpretation remains a separate product decision.

## Validation Strategy

Each adoption lane should include:

- one forced TrueColor parser-backed test for semantic role coverage;
- one forced HTML parser-backed test for escaping and span grouping;
- the family plain regression filter to prove default output did not change;
- `cargo fmt --all --check`;
- `cargo clippy -p merman-ascii --all-targets -- -D warnings` when code changes are involved.
