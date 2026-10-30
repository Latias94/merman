# merman-model

Shared canonical Mermaid data and controlled execution contracts for Merman consumers.

Default features are empty. System clock, time-zone, randomness and timing adapters are explicit;
the default runtime remains deterministic. Selecting a shared type does not select a source parser
or output backend.

The development extraction is tracked by the
[model/source boundary plan](../../docs/plans/2026-10-30-model-and-source-boundaries.md).
Select the available `diagram-*` features for canonical family data and `presentation-data` for
the migrated full data and borrowed presentation views. These flags do not compile source parsers.
The [implementation record](../../docs/reviews/2026-10-30-model-source-boundaries-implementation.md)
tracks the remaining family migration; the complete package/API cleanup is still in progress.
