# Shoal

`merman-layout-shoal` is an experimental Rust layout engine for measured directed multigraphs. It accepts nested groups, cycles, self-loops, parallel edges, disconnected components, port hints, and middle or terminal labels. It returns rectangles, edge paths, label anchors and the drawing extent in any of four directions. The crate uses the Rust standard library.

```rust
use merman_layout_shoal::{Direction, EdgeSpec, Graph, NodeId, NodeSpec, Options, Size, layout};

let mut graph = Graph::default();
graph.direction = Direction::Right;
let size = Size { width: 96.0, height: 36.0 };
graph.nodes.push(NodeSpec::new(size, 0));
graph.nodes.push(NodeSpec::new(size, 1));
let mut edge = EdgeSpec::new(NodeId(0).into(), NodeId(1).into());
edge.end_marker.length = 8.0;
graph.edges.push(edge);

let drawing = layout(&graph, &Options::default()).unwrap();
let source = drawing.nodes[0];
assert_eq!(drawing.edges[0].points[0].x, source.x + source.width);
```

Run the example to inspect the box contacts and the label of an edge spanning three stages. [Example source](examples/measured_dag.rs)

```console
cargo run -p merman-layout-shoal --example measured_dag
```

## Geometry contract

- `NodeId`, `GroupId` and `EdgeId` are dense `u32` indices into `Graph`'s respective arrays. Outputs use the same indices. Each parallel edge keeps its own route and measured label.
- Node and label sizes use the final drawing axes. Supply finite, nonnegative outer dimensions including padding. `Rect.x` and `Rect.y` give the top-left corner. `Rect::center()` gives the center for renderers that place shapes by their center.
- The first and last `RoutePoint` are contacts on the source and target box faces named by `EdgeGeometry.ports`. `axis` is the incoming leg's axis in drawing coordinates. At the first point it is the departing axis. A curved leg can change both coordinates while keeping flow-axis tangents. `KnotKind::GapBoundary` limits the extent of rounding into the neighboring rank. `Orthogonal` produces axis-aligned legs.
- Supply marker and label footprints in drawing units. The renderer moves box contacts onto the actual shape along the indicated side normal and applies the marker setback once. The solver reserves intermediate ranks for labels and parallel lenses where needed.
- `LayoutOutput.bounds` is the size of the normalized drawing. The union of its node boxes, label boxes and routing points begins at `(0, 0)`. Edge occurrences are returned in semantic source-to-target order, including edges reversed internally for ranking.

## Layout policies

The default ranks the acyclic condensation of each connected component. `CyclePolicy::HybridAdaptive` anchors small cycles at their first external entry or first declared node. Larger cycles use greedy feedback-arc ordering with node hints and declaration order to resolve ties. `CyclePolicy::SccGreedyFas`, `CyclePolicy::SccDfs`, and `RankPolicy::BoundedNetworkSimplex` provide alternative policies for comparisons on the same input.

Long edges normally travel along intermediate channels inside the diagram. On small graphs, the hybrid policy can reserve exterior lanes for long feedback edges when doing so preserves or improves rank crossings. `BudgetCaps::max_dummies` bounds the number of expanded vertices across the drawing. The solver records each edge downgraded at this cap in `LayoutStats::direct_fallback_edges`. `topological_crossings` counts inversions between consecutive ranks. For a flat graph, `routed_crossings` counts pairs of routing polylines with a proper intersection. For a compound graph it compares the assembled paths in the selected routing style and excludes the fan near a shared endpoint. Both measurements precede final outline clipping and SVG painting.

Set `Options::bus_routing` with `RoutingStyle::Orthogonal` to allow unlabelled, consistently styled fans to share a trunk where there is room. Each edge keeps its own semantic occurrence and terminal marker.

Set `Options::global_orthogonal` with `RoutingStyle::Orthogonal` to search for shorter, lower-conflict routes around measured boxes. Shoal can give coincident legs separate tracks where their approach order and available clearance allow it. Port contacts, marker leads, and compound border stitches are kept. `BudgetCaps::max_ortho_grid_nodes` limits the search, which falls back to the layered drawing when a refinement does not fit. Bus routing takes precedence when requested.

`Options::inline_labels` tries to place short middle labels in an existing rank gap in flat graphs. A crowded gap restores its label sublayer. `Options::compact_ranks` can move a short node up to use space under its immediate parent when their edge is straight and the proposed geometry clears nearby boxes and routes.

`PackingPolicy::AspectTargeted` uses two-dimensional shelves with a target proportion across and along the flow. `BalancedAspect` packs disconnected components against a skyline and uses a target physical width-to-height ratio. `Strip` packs in a single row across the flow. `PackingPolicy::FitWidth` wraps complete components within a physical width when they fit individually.

`WidthPolicy::FitWidth` tries component wrapping and tighter spacing. It can also turn a horizontal flow below a configured breakpoint and fold eligible leaf fans across rows. Set `allow_turn` and `fold_wide_ranks` explicitly. Read the selected direction and width from `LayoutOutput`, including when the result still exceeds the requested width. Grid quantization is reserved for a future policy.

Set `Options::capture_node_ranks` for interactive layouts. Each `LayoutOutput::node_ranks` entry uses its component's or compound container's local rank origin. Match surviving nodes by the caller's semantic IDs and populate `NodeSpec::order_hint` with the previous rank and physical cross-axis center. Set `order_hint_direction` to associate the hints with the prior local direction.

`NodeSpec::set_position_hint` adds a component-relative center. Supply the node's previous measured size and caller-defined incident-edge signature to give unchanged nodes more weight. When the hinted drawing has crossings, Shoal can compare it with a stateless solve within the same work budget. It selects the stateless drawing when a crossing count improves and both counts are at most those of the hinted drawing.

For flat, natural-width editor layouts, `Options::reuse_unchanged_components` can reuse component geometry within the same `LayoutScratch`. Set `capture_node_ranks` and give each node its current incident-edge signature on the first solve. On the next solve, match surviving nodes by their stable IDs and supply the prior frame's ranks and positions alongside current signatures. Shoal checks the component's measurements, topology, options and frame geometry before reusing it.

## Nested groups

Set the `parent` of a node or group to its enclosing `GroupId`. Each group can supply a minimum size, padding, a measured title and a local direction. With the default `GroupPolicy`, a group crossed by an external edge inherits its parent's direction. Set `inherit_crossed_direction` to `false` to keep the declared direction across that boundary. `GroupGeometry` reports the effective direction, parent, rectangle and title center in drawing coordinates.

Use `Endpoint::Group` to connect a group directly to an outside node, another group or its own member. `LayoutOutput.border_stitches` identifies each shared border point by edge, point index, group and side, in path order. A painter can interpolate each section through that point while applying markers only at the semantic endpoints. If the drawing exceeds `max_border_ports`, `LayoutOutput.truncated_routes` identifies the physical endpoints of the affected edge occurrences.

For curved routes between facing sibling boundaries, Shoal can straighten the corridor between the groups and align both contacts on a shared lane. It preserves the order and spacing of adjacent ports and checks the change against nodes, labels, markers and other routes.

Set `Graph::lane_order` to the root group IDs in physical left-to-right or top-to-bottom order. Shoal arranges lane members on a shared rank grid, aligns the lane borders, and places titles on the starting side of the flow. Use `WidthPolicy::Natural` for these graphs and put loose items into a root lane. For an already placed grid, `route_orthogonal_spans` routes between fixed lead ends around the supplied obstacle rectangles.

## Work control and buffer reuse

Use `layout_with_control(&graph, &options, &mut scratch, &mut control)` to share buffers across a batch or apply an operation budget. `WorkControl::check` polls cancellation. `charge(units)` admits work before bulk operations and during graph walks. The standard library handles sorting after admission of its comparison budget. `WorkControl::try_charge_optional` admits refinements such as compound seam alignment. Declining optional work keeps a valid drawing from the preceding pass. `NoopWorkControl` runs without a caller limit.

`LayoutScratch::default()` retains up to 2 MiB of reusable vertex, segment, adjacency and layer storage. `LayoutScratch::with_max_retained_bytes(bytes)` sets the total retention cap. `retained_bytes()` reports the retained vector capacities in bytes. A solve can temporarily exceed the cap. On return, including on cancellation and input errors, the retention cap is reapplied. A zero cap releases all scratch storage after every call. Each `LayoutOutput` owns its geometry independently.

Invalid measurements and inputs whose scale collapses required geometry return `LayoutError::InvalidInput`. A failed vector reservation returns `AllocationLimit`. A checked arithmetic overflow or a work-control error returns `LayoutError::Work(WorkError)`, preserving the distinction between overflow and interruption.

Licensed under MIT or Apache-2.0.
