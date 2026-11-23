# Shoal

`merman-layout-shoal` is an experimental, standard-library-only Rust layout engine for flat directed acyclic multigraphs. Supply measured node and edge-label boxes to get node centers, polylines, label anchors and drawing bounds. The kernel supports four directions, disconnected components and parallel edge occurrences. A self-loop or directed cycle returns `LayoutError::Unsupported`.

Run the [measured DAG example](examples/measured_dag.rs) from the workspace.

```console
cargo run -p merman-layout-shoal --example measured_dag
```

## Renderer contract

- `NodeId(i)` refers to `graph.nodes[i]`. Both output arrays follow their respective input arrays. Repeated edges retain their own route, clearances and label.
- Supply finite, nonnegative dimensions, spacing and marker clearances in the same units. Dimensions use the final drawing axes for all four directions. The caller measures text and includes any padding in the box.
- `node_spacing` separates boxes across a rank and separates weakly connected components. `rank_spacing` is the minimum boundary-to-boundary gap per logical step. Each edge advances at least `max(1, min_rank_span)` steps. A gap can grow to fit labels and the two marker approaches.
- `points[0]` and the last point are the node centers. The second and penultimate points lie beyond the facing sides of their respective node boxes, with space for `start_clearance` and `end_clearance`. Straight edges retain both exterior points. Intersect those endpoint rays with the actual shapes, then apply the marker setbacks during painting.
- `label_center` lies on the route through its measured box. `bounds` includes the node and label rectangles and all route points. Empty input yields empty arrays and zero bounds. Coordinates increase along the rank axis for `Down` and `Right` and decrease for `Up` and `Left`. Use the returned extents as the drawing origin and size.

## Layout and work limits

Shoal ranks each component by longest path and stores only occupied ranks. Two barycenter sweeps order nodes using their neighbors. Each occupied gap reserves a common band for labels. Adjacent edges cross the band on distinct tracks, then fan into ports ordered across each node. Edges that skip occupied ranks use side lanes outside the component's nodes and adjacent labels. Nonoverlapping rank intervals can share a lane. Long routes have at most six points regardless of span. Dense layers can still have edge crossings, and simultaneous long edges can widen a component.

For `V` nodes and `E` edge occurrences, working storage is `O(V + E)` and worst-case work is `O((V + E) log(V + E + 1))`, including ordering and routing. The numeric rank span only enters rank arithmetic and physical spacing.

Call `layout_with_control` to charge work to a caller-owned budget or check cancellation. Its callback receives incremental units during admission, graph walks, ordering and routing. Returning `Err(e)` ends the call with `LayoutError::Interrupted(e)`. A rank sum, allocation or coordinate calculation that exceeds its representation returns `LayoutError::Overflow`. `layout` uses the same kernel with an unrestricted work callback.

Licensed under MIT or Apache-2.0.
