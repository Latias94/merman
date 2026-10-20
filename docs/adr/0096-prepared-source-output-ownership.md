# ADR-0096: Prepared source and output ownership

Accepted for the development branch on 2026-10-20. Hosts need to redraw unchanged Mermaid source
when width or font metrics change. Keep one immutable `PreparedSource` with canonical semantics,
effective configuration and genuine parse-time runtime facts; let family preparation borrow that
data while each output owns fresh control, admission, runtime, work accounting and layout.
Completed outputs own their public data. Freezing parse facts avoids reparsing or inventing a
second deferred grammar that would regenerate dates and IDs during redraw. The existing one-shot
operation runner remains the execution owner.

Compatibility JSON is an output projection, not a reusable rendering input or an independently
injectable layout payload. Native typography admission inspects typed declarations. Flowchart,
Swimlane and Agentflow share family-owned route and label decisions between SVG and geometry;
encoded geometry has its own bounded writer and byte budget. A discarded SVG, generic scene graph
or production SVG readback would preserve the wrong work and resource responsibilities.

Web prepared wrappers retain their creating runtime. Browser mounting owns observers, document
font measurement, versioned candidate publication and disposal; it does not introduce a global
model cache or native persistent-handle ABI. Kanban width adaptation is explicit whole-column
placement. Other families retain their source-backed layout behavior. These choices transfer
Pele's parse-once lifecycle to Merman's Mermaid compatibility boundary without adopting its
different rendering contract. New theme behavior is outside this decision.

See the [consumer migration](../migrations/render-reuse-host-integration.md),
[family architecture](0073-family-owned-diagram-architecture.md),
[browser ownership](0074-browser-runtime-and-benchmark-ownership.md) and
[implementation plan](../plans/2026-10-19-1806-refactor-render-reuse-host-integration-plan.md).
