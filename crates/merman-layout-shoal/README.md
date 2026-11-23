# Shoal

`merman-layout-shoal` lays out measured directed graphs using the Rust standard library. The flat preview accepts acyclic multigraphs, including disconnected components, parallel edges, middle labels and marker clearances. It returns box rectangles, edge routes, label anchors and the size of the drawing in any of four directions.

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
- The first and last `RoutePoint` are contacts on the source and target box faces named by `EdgeGeometry.ports`. `axis` is the incoming leg's axis in drawing coordinates. At the first point it is the departing axis. A curved leg can change both coordinates while keeping flow-axis tangents. `Orthogonal` produces axis-aligned legs.
- The second and penultimate points bound the straight terminal shafts. Supply the required shaft room in `start_marker.length` and `end_marker.length`. A renderer can move a contact along the face normal to its painted contour, then apply its marker setback. Zero-size boxes and zero spacing may yield coincident points.
- Automatic ports spread over `port_span`, centered on the face. An omitted span uses the face length less twice `spacing.corner_clearance`, floored at zero. An explicit zero pins ports to the center. `anchor_y` places left/right anchors relative to the top edge. A flow-facing `FixedOffset` overrides the automatic anchor and measures from the face center toward the right or bottom.
- The union of node boxes, middle label boxes and route points starts at `(0, 0)`, including for `Up` and `Left`. `bounds.width` and `bounds.height` cover that union. Empty graphs preserve their direction and have zero bounds.

## Preview support

Each edge advances at least `max(1, min_rank_span)` logical ranks. Only occupied ranks use storage. `spacing.rank` controls the minimum gap per step, `spacing.node` separates nodes on a rank, `spacing.edge` separates tracks and `spacing.component` separates disconnected components. The preview defaults to `PackingPolicy::Strip` and reports the component count in `stats.components`.

The public model includes the policies and measurements for later solver stages. Groups, directed cycles, self-loops, terminal labels, ports on other faces, network simplex ranking, two-dimensional packing, width fitting and grid layout currently return `LayoutError::Unsupported`. The preview uses longest paths and up to two ordering sweeps. Edge weights, previous-frame order hints, advanced port pitch and compound policies are reserved for the full solver. The crossing counters are unmeasured and currently read zero.

## Work control and buffer reuse

Use `layout_with_control(&graph, &options, &mut scratch, &mut control)` to share buffers across a batch or apply an operation budget. `WorkControl::check` polls cancellation. `charge(units)` admits work before bulk operations and during graph walks. The standard library handles sorting after admission of its comparison budget. Two sufficient budgets produce the same geometry. `NoopWorkControl` runs without a caller limit.

`LayoutScratch::default()` retains up to 2 MiB of traversal buffers. `LayoutScratch::with_max_retained_bytes(bytes)` sets the total retention cap. `retained_bytes()` reports the retained vector capacities in bytes. A solve can temporarily exceed the cap. On return, including on cancellation and input errors, the buffers are cleared and the cap is reapplied. A zero cap releases all scratch storage after every call. Each `LayoutOutput` owns its geometry independently.

Invalid measurements and inputs whose scale collapses required geometry return `LayoutError::InvalidInput`. A failed vector reservation returns `AllocationLimit`. A checked arithmetic overflow or a work-control error returns `LayoutError::Work(WorkError)`, preserving the distinction between overflow and interruption.

Licensed under MIT or Apache-2.0.
