# merman-model

Shared canonical Mermaid data and controlled execution contracts for Merman consumers.

Default features are empty. System clock, time-zone, randomness and timing adapters are explicit;
the default runtime remains deterministic. Selecting a shared type does not select a source parser
or output backend.

The development extraction is tracked by the
[model/source boundary plan](../../docs/plans/2026-10-30-model-and-source-boundaries.md).
Select `diagram-*` features for canonical family data, `all-diagrams` for every payload, and
`presentation-data` for full data and borrowed presentation views. These flags do not compile source parsers.
The [implementation record](../../docs/reviews/2026-10-30-model-source-boundaries-implementation.md)
tracks extraction of all 34 payloads, integrated presentation/product migration, and final validation.
This guide targets `merman-model 0.10.0-alpha.1`. The package is new in the `0.10` line; existing `0.9.x` applications should follow the migration below.

`presentation-data` includes immutable configuration, metadata, full family values, output context, and checked `PresentationDiagram` construction. `compat-json` selects pure Mermaid-compatible data projection. These APIs do not authenticate source history or evaluate visual themes; source producers retain that responsibility. Existing model and output admission still applies.

See the [model/source migration](../../docs/migrations/model-source-boundaries.md) for imports, direct output consumers, configuration construction, and requested analysis fixes.
