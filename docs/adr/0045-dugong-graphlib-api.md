# ADR 0045: `dugong-graphlib` API and Semantics

## Status

Accepted

## Context

`dugong` will provide a Dagre-compatible layout engine in Rust. Upstream Dagre relies on a Graphlib
Graph API (compound graphs, multigraphs, default labels, traversal helpers). To keep the Dagre port
close to upstream and to make `dugong` generally useful, we will implement a dedicated graph library
crate: `dugong-graphlib`.

Constraints and goals:

- headless (no DOM, no rendering)
- deterministic behavior for parity (avoid hash iteration dependence)
- ergonomic API for Rust users while staying conceptually close to Graphlib
- minimal crate fragmentation (only split graph container vs layout engine)

## Decision

### Crate split

- `dugong-graphlib`: graph container + algorithms/utilities (Graphlib equivalent).
- `dugong`: layout engine (Dagre equivalent), depends on `dugong-graphlib`.

### Identifiers

- Node IDs are strings in upstream Graphlib and in Mermaid diagrams. `dugong-graphlib` uses `String`
  as the primary node identifier type.
- Edge IDs are represented as a key `(v, w, name?)` to support multigraph behavior.

### Graph configuration

`Graph` supports:

- `multigraph`: if true, multiple edges between the same `(v, w)` are distinguished by `name`.
- `compound`: if true, supports `parent/children` relationships for clusters/subgraphs.

These flags are set at graph construction time and are immutable.

### Labels

`Graph` stores three label domains:

- graph label (global attributes, e.g. `rankdir`, `nodesep`, `ranksep`)
- node label (per-node attributes, e.g. `width`, `height`, later `x`, `y`)
- edge label (per-edge attributes, e.g. `weight`, `minlen`, label size, later `points`)

Defaults:

- `set_default_node_label(f)` sets a generator used when a node is first referenced without an
  explicit label.
- `set_default_edge_label(f)` sets a generator used when an edge is first created without an
  explicit label.

The default label generators must be deterministic for parity.

### API surface (conceptual mapping)

`dugong-graphlib` provides APIs conceptually equivalent to upstream:

- `set_graph(label)` / `graph()` accessors for graph label
- `set_node(id, label)` / `node(id)` / `has_node(id)` / `remove_node(id)`
- `set_edge(v, w, label, name?)` / `edge(v, w, name?)` / `has_edge(...)` / `remove_edge(...)`
- `nodes()` / `edges()` returning deterministic iteration order
- `set_parent(child, parent)` / `parent(child)` / `children(parent)`
- `predecessors(node)` / `successors(node)` / `in_edges(node)` / `out_edges(node)`
- `node_count()` / `edge_count()`

Naming in Rust may be idiomatic, but the semantics must match upstream to enable parity tests.

### Deterministic iteration

Upstream JS Graphlib relies heavily on insertion order (Maps/arrays) and stable sorts (ES2019+).
To reduce parity risk:

- `nodes()` and `edges()` must return items in a deterministic order.
- internal algorithms must not rely on `HashMap` iteration order.

Implementation detail is deferred, but the API must guarantee deterministic ordering.

### Serialization / debug

To support parity work and debugging, `dugong-graphlib` should provide:

- a stable, JSON-like export format (graph label + nodes + edges + parents), similar in spirit to
  `graphlib/json` in the JS ecosystem.

This is not part of the minimum API, but is a planned capability.

## Non-Goals

- SVG/canvas rendering
- text measurement or font shaping
- Mermaid-specific cluster adjustments (handled in `merman` rendering)

## Consequences

- `dugong-graphlib` can be used independently of `dugong` for other graph problems.
- `dugong` can closely follow upstream Dagre algorithm structure without embedding a bespoke graph
  container.
- Deterministic ordering guarantees make parity tests reproducible across platforms and runs.
