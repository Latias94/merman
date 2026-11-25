# merman-render-flow

Shape names, asset dimension checks, and geometry shared by the Flowchart, Swimlane, and
AgentFlow renderers. This crate owns Mermaid's Bang and Cloud outlines and BPMN body geometry,
including connection points. `merman-render` supplies measured labels and owns model admission,
image handling, styling, and SVG emission. With `layout-dagre`, this crate also assembles and
extracts compound graphs, selects cluster anchors, and accounts for graph mutations. The
renderer supplies source direction and projects solved geometry into the diagram layout.

Use `merman` for source-to-output rendering.
