# HPD-050 - Sequence Compat JSON Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

After the ASCII Flowchart group-bounds hardening, the next small production panic audit checked
typed render-model compatibility bridges. Sequence already uses `SequenceDiagramRenderModel` as the
semantic source for render-model parsing, but `to_compat_json(...)` still round-tripped through
`serde_json::to_value(self)` and then removed expected fields from the serialized object.

That made the public JSON compatibility path depend on `expect`, `unreachable!`, and panic-on-missing
field assumptions even though all required data is already available as typed fields.

## Changes

- Replaced the serialize-then-remove-field bridge with direct `serde_json::Map` construction.
- Preserved the existing compatibility JSON shape:
  - root `type` is still supplied from the diagram metadata;
  - `accTitle`, `accDescr`, `actorOrder`, `createdActors`, `destroyedActors`, `actorKeys`, and
    `centralConnection` keep their previous JSON field names;
  - absent message `placement` is still omitted;
  - zero `centralConnection` is still omitted;
  - autonumber `start` / `step` keep integer JSON numbers for whole finite values and float JSON
    numbers for decimal values.
- Kept the existing serde derives for ordinary typed model serialization/deserialization; the
  change is limited to the compat JSON projection.

## Verification

- `cargo +1.95 fmt --check -p merman-core` - passed.
- `cargo +1.95 nextest run -p merman-core parse_sequence_render_model_uses_typed_variant_without_changing_json_parse` -
  passed, `1` test run.
- `cargo +1.95 nextest run -p merman-core sequence` - passed, `34` tests run.
- `git diff --check` - passed.

## Boundary

No Sequence parser behavior, SVG output, SVG baseline, root viewport formula, or known Sequence
measurement residual changed. This slice only removes avoidable production panic assumptions from
the Sequence typed-model-to-compat-JSON bridge.
