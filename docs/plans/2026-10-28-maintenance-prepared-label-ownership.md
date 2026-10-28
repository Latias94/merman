# Maintenance prepared label ownership

Date: 2026-10-28. Target: v0.9.x / Mermaid 12.1.0.

## Problem

The maintenance correctness repairs share label interpretation rules, but Class
and Block still prepare the same terminal content separately during layout and
SVG emission. A future change can therefore make measured and emitted content
diverge again. Retain the prepared content through the existing family operation.

## Decisions

- Add private Class and Block family artifacts containing the existing semantic
  model/layout pair and family-owned prepared labels. Public layout JSON,
  `ClassNodeLabelPlan`, options, capability boundaries, and wire operations stay
  unchanged.
- Prepare Class SVG titles and central relationship label rows during layout.
  Retain title metrics and rows through both Dagre and ELK. SVG emission consumes
  these terminals without reconstructing or measuring them. Existing HTML, note,
  member, namespace, and terminal-label paths retain their contracts.
- Prepare Block node and edge XHTML once using the effective maintenance
  sanitizer configuration. Measure the retained fragment and emit that exact
  fragment. Resolve repeated source node IDs using the existing writer's source
  ownership, and associate edges by semantic position rather than an assumed
  unique label or edge ID.
- Use maintenance's existing operation cancellation and resource policy. Do not
  import main's theme architecture, prepared-math transport, or additional public
  resource limits. Retained data remains private to its family artifact.
- Remove superseded preparation paths rather than retaining a writer fallback
  which can hide a missing prepared terminal.

## Verification

- Exercise preparation and real SVG emission through public render operations.
  Reject repeat measurements for retained Class titles, verify retained Block
  fragments and edge associations, and check cancellation and layout projection.
- Run Cargo serially with the shared target directory and bounded build/test
  concurrency. Run the affected nextest suites, layout snapshots, strict Clippy,
  formatting, Class-only/Block-only builds, and the existing SVG comparison gate.
- Expect no layout or output change from transferring ownership. Investigate any
  difference before changing a golden or receipt; do not widen tolerances.
- Commit the adaptation, update PR #247 and its evidence, and report CI against
  the actual pushed revision. These correctness checks make no performance claim.
