# Class Diagram Debug SVG

This debug exporter is for development only. It visualizes the **headless layout output**
from `merman-render` for Mermaid `classDiagram`. The [selected reference bundle](../../tools/upstreams/MERMAID_REFERENCE_BUNDLE.json)
identifies the current Mermaid baseline.

It is **not** intended to be Mermaid-parity SVG output (Stage B). It exists to make it easy to
spot obvious layout issues (overlaps, broken routes, missing cluster bounds, terminal label placement).

## Run

From a shell with stdin redirection:

```sh
cargo run --locked -p merman-render --features diagram-class --example class_debug_svg < fixtures/class/basic.mmd > out.svg
```

With cardinalities (terminal labels):

```sh
cargo run --locked -p merman-render --features diagram-class --example class_debug_svg < fixtures/class/upstream_relation_types_and_cardinalities_spec.mmd > out.svg
```

## What It Shows

- Blue rectangles: node bounding boxes (classes + notes).
- Gray rectangles: namespace clusters.
- Black polylines: edge routes.
- Yellow boxes: edge label bounding boxes (`edge.title`).
- Cyan boxes: edge terminal label bounding boxes (e.g. cardinalities), labeled as `SL/SR/EL/ER`.

