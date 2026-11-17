# ASCII and Unicode Rendering

Start with the [support matrix](../ASCII_SUPPORT_MATRIX.md) for current family availability,
layout and encoding admission, viewport policy, and result metadata. The
[crate README](../../../crates/merman-ascii/README.md) owns installation, Rust API examples,
terminal themes, and public options.

## Family behavior

| Family | Maintained support note |
| --- | --- |
| Agentflow | [Roles, expanded flows, and complete reading records](AGENTFLOW_SUPPORT.md) |
| Block | [Columns, spans, nested grids, and fixed-position relationships](BLOCK_SUPPORT.md) |
| Flowchart and Swimlane | [Graph semantics, shapes, labels, and routing](FLOWCHART_SUPPORT.md) |
| Pie | [Shared-scale percentage bars and complete values](PIE_SUPPORT.md) |
| Requirement | [Shared-endpoint traceability and keyed fields](REQUIREMENT_SUPPORT.md) |
| Sequence | [Participants, signals, controls, and lifecycle](SEQUENCE_SUPPORT.md) |
| State | [State roles, concurrency, and compound transitions](STATE_SUPPORT.md) |
| Usecase | [Roles, boundaries, relationships, and ordered JSON tables](USECASE_SUPPORT.md) |

Class and ER keep their [joint capability matrix](../ASCII_CLASS_ER_CAPABILITY_MATRIX.md).
Packet, XYChart, and structured-text families are described by the shared support matrix and
the crate's public API documentation.

## Design and evidence

| Task | Maintained owner |
| --- | --- |
| Compare terminal references and understand intentional differences | [Reference comparison](ASCII_REFERENCE_COMPARISON.md) |
| Find current implementation gaps and acceptance conditions | [Gap registry](ASCII_GAP_REGISTRY.md) |
| Check resource, cancellation, and panic boundaries | [Resource audit](../ASCII_RESOURCE_AND_PANIC_AUDIT.md) |
| Check family admission and width evidence | [Admission report](../ASCII_PHASE_GATE_REPORT.md) |
| Reproduce the immutable copied-fixture boundary | [V1 coverage contract](V1_MERMAID_ASCII_COVERAGE.md) |
| Interpret later reference-fixture discoveries | [Moving-reference manifest](ASCII_MOVING_REFERENCE_MANIFEST.md) |
| Understand the measured terminal-cell representation choice | [Cell representation evidence](TERMINAL_CELL_PROTOTYPE_RESULTS.md) |

Detailed support and evidence documents live here. The crate retains its package README, legal
notices, and fixture-local documentation and data. Runtime capability evidence points to these
maintained owners; a document move must update those paths and generated projections together.
