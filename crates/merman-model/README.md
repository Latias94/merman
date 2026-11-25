# merman-model

Shared canonical Mermaid data and controlled execution contracts for Merman consumers.

Default features are empty. System clock, time-zone, randomness and timing adapters are explicit;
the default runtime remains deterministic. Selecting a shared type does not select a source parser
or output backend.

The [model/source boundary ADR](../../docs/adr/0099-model-and-source-boundaries.md)
defines package ownership.
Select `diagram-*` features for canonical family data, `all-diagrams` for every payload, and
`presentation-data` for full data and borrowed presentation views. These flags do not compile source parsers.

This guide covers `merman-model 0.11.0`. The package was introduced in `0.10.0`. Applications upgrading from `0.9.x` should follow the migration below. See the [host guide](../../docs/release/PREBUILT_PRODUCTS.md) for available versions and installation channels.

`presentation-data` includes immutable configuration, metadata, full family values, output context, and checked `PresentationDiagram` construction. `compat-json` selects pure Mermaid-compatible data projection. These APIs do not authenticate source history or evaluate visual themes; source producers retain that responsibility. Existing model and output admission still applies.

Insert links into the Sequence model's `SequenceActor.links` in menu display order. Its
`IndexMap<String, Value>` retains that order and serializes as a JSON object.

See the [model/source migration](../../docs/migrations/model-source-boundaries.md) for imports, direct output consumers, configuration construction, and requested analysis fixes.
