# merman-render-flow

The `diagram-flowchart`, `diagram-swimlane`, and `diagram-agentflow` selectors enable the shared
Flow layout configuration, including Mermaid defaults, label modes, and typography and density
settings. Agentflow also owns the overlay that projects admitted Agentflow settings into the
shared layout inputs. `merman-render` selects and admits the layout backend and supplies the
host-selected theme and measurement services.
The crate also projects visible endpoints and memberships and builds collision-safe layout edge IDs.

Shape names, asset dimension checks, and geometry shared by the Flowchart, Swimlane, and
AgentFlow renderers. This crate owns Mermaid's Bang and Cloud outlines and BPMN body geometry,
including connection points. It parses authored styles, prepares the operation-local class styles
shared by layout and paint, and writes the SVG document, marker definitions, graph and edge-label
groups, SVG edge labels, edge paths, container shells, and the supported shape bodies. It builds
the SVG hierarchy indexes used during emission. `merman-render` supplies measured labels and route
geometry, dispatches the graph body, admits the theme and model, and applies final output policy.
With `layout-dagre`, this crate also
assembles and extracts compound graphs, selects cluster anchors, and accounts for graph mutations.
The renderer supplies source direction and projects solved geometry into the diagram layout.

This crate exposes a workspace-internal service provider interface (SPI). It is published in
lockstep with Merman and expects an exact version match with the Merman crates that use it.

Use `merman` for source-to-output rendering.
