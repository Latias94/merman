# merman-render-flow

The `diagram-flowchart`, `diagram-swimlane`, and `diagram-agentflow` selectors enable the shared
Flow renderer. It owns config and theme binding, shape and label geometry, source styles,
native typography admission, and complete Mermaid SVG emission. Flowchart can also emit the NativeCompact profile with
`native-compact` and either `layout-dagre` or `layout-elk`. The ELK path accepts two-point sections
with `elk.lineHops: false` in the classic ungrouped pilot with plain single-line rectangle labels
and unlabeled edges. Edge geometry queries use the same prepared routes and label positions as SVG.

The Dagre and ELK adapters prepare, solve, and project layout in this crate. Flow also prepares,
solves, and projects the measured Swimlane graph. `merman-render` selects and admits the backend,
supplies the host's measurement, icon, and math services, and applies final SVG output policy.
The Shoal solver and bridge retain their separate ownership and supply canonical routes for Flow
to paint.

This crate exposes a workspace-internal service provider interface (SPI). It is published in
lockstep with Merman and expects an exact version match with the Merman crates that use it.

Use `merman` for source-to-output rendering.
