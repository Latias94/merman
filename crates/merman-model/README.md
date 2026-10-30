# merman-model

Shared canonical Mermaid data and controlled execution contracts for Merman consumers.

Default features are empty. System clock, time-zone, randomness and timing adapters are explicit;
the default runtime remains deterministic. Selecting a shared type does not select a source parser
or output backend.

The development extraction is tracked by the
[model/source boundary plan](../../docs/plans/2026-10-30-model-and-source-boundaries.md).
Family data and presentation selectors migrate in the subsequent implementation slices.
