# ADR 0082: Versioned Theme Authoring Facade

- Status: proposed
- Date: 2026-08-14

## Context

Merman is a headless Mermaid engine with typed theme customization. The renderer already has the
correct low-level authority chain:

```text
DiagramThemeSpec
    -> DiagramThemeCompiler
    -> compiled theme
    -> private family adapter
    -> rendered document and terminal evidence
```

That module is deliberately deep: it supports semantic targets, variants, ordinal palettes,
typography, bounded effects, assets, requirements, and complete rule sets behind one compiler
interface. It is also more than most theme authors need to learn.

The alpha `ThemeTokens` type attempted to provide a smaller interface, but it accumulated
family-specific fields for actors, notes, activations, clusters, and other renderer vocabulary. If
that object keeps growing, it becomes a second theme language that duplicates `ThemeRuleSet`.
Bindings also lack one Rust-owned materialization operation, so adding token expansion separately in
Web, Node, Python, or a Playground would create multiple authorities for defaults, ordering,
diagnostics, and replay.

Version 1 is share-first. One self-contained `ThemeDefinitionV1` value is the ordinary authored and
shared theme. Pretty-printed JSON, canonical JSON bytes, Rust or generated-SDK constructors, a local
file, and an optional compressed Playground URL are projections of that same value, not separate
theme formats. A complete `DiagramThemeSpec` remains the advanced share format when the compact
definition cannot express a recipe. Version 1 does not add theme packages, manifests, installation,
lock files, or a remote registry.

This ADR defines the proposed alpha authoring contract used by the C7a pre-freeze authoring
witnesses. It does not declare a stable public interface, satisfy C6a, or unblock C7a. C7a remains
blocked until the plan's compiler, family-writer, terminal-evidence, rollout, and author-task gates
close. While this ADR is `proposed`, it is the sole candidate source of truth for the version 1
authoring envelope and expansion tables. Moving it to `accepted` records design approval only; it
does not certify the implementation, freeze an expansion row, or create a compatibility promise.
Only `C7a-contract` begins the compatibility and expansion-version freeze after the required
consumer, terminal-evidence, and rollout witnesses pass.

## Decision

### 1. Keep the internal renderer closed and compose it at first-party facades

The low-level renderer continues to accept exactly one of:

```text
preset ID
complete DiagramThemeSpec
```

`ThemeDefinitionV1` is not a third low-level renderer input. First-party facades may accept an
admitted definition and compose the existing authorities in order:

```text
bounded decode, when input is bytes
    -> ThemeMaterializer
    -> DiagramThemeCompiler
    -> closed renderer
```

That convenience path must call the Rust-owned operations rather than reimplement expansion or
compilation in a host. The separate materialization operation remains available for inspection,
export, editing, and reuse. Preset-plus-patch, partial-spec merge, output-target branches, runtime
light/dark conditions, host policy, raw CSS, selector strings, and family-capability-dependent
lowering are outside version 1.

Light and dark themes are two independently materialized definitions. A preset is a catalog
selection and starter example, not another authoring format. A copy/export action returns a
self-contained `ThemeDefinitionV1` when the preset has one lossless source definition, and otherwise
returns a complete spec. Version 1 does not require reverse-engineering an arbitrary preset into a
compact definition and does not add runtime preset-plus-patch semantics.

### 2. Separate authoring, compilation, and execution authorities

The only valid authority chain is:

```text
ThemeDefinitionV1
    -> ThemeMaterializer
    -> MaterializedTheme with a complete DiagramThemeSpec

DiagramThemeSpec
    -> DiagramThemeCompiler
    -> required capabilities + ThemeRecipeFingerprint + compile diagnostics

RenderedDocument / export report
    -> actual application + residuals + portability + target admission
```

`ThemeMaterializer` is a pure deterministic lowering module. It owns authoring defaults, token
expansion, validation of its own input, and authoring diagnostics. It does not inspect renderer
support, output targets, family implementations, host policy, or runtime resources. It does not
produce authoritative capabilities, admission, portability, or execution evidence.

`DiagramThemeCompiler` remains the only owner of compiled recipe identity and declared capability
requirements. A concrete render or export report remains the only authority for `Applied`,
`Residual`, `Portable`, and target admission.

### 3. Define the candidate version 1 input envelope

The persisted input has this closed shape:

```text
ThemeDefinitionV1 {
    authoring_schema_version: 1,
    expansion_version: 1,
    tokens: ThemeTokensV1,
    styles?: ThemeRuleSetWireV1[],
}
```

| Field | Required | Null | Version 1 meaning |
| --- | --- | --- | --- |
| `authoring_schema_version` | Yes | Rejected | Must be the integer `1`; it versions the authoring wire shape. |
| `expansion_version` | Yes | Rejected | Must be the integer `1`; it selects the exact defaults and expansion rows in this ADR. |
| `tokens` | Yes | Rejected | A closed `ThemeTokensV1` object. `{}` is valid and selects every version 1 default. |
| `styles` | No | Rejected | `ThemeRuleSetWireV1[]`; omission means an empty authored rule set. The executable expansion table derives the authored-rule limit. |

`ThemeRuleSetWireV1[]` is the closed, flat tagged-array wire projection of the existing typed rule
set. Each entry is either a `rule` entry or an `ordinal-palette` entry, matching the complete-spec
wire rather than introducing an authoring-only container object. Decoding produces the in-memory
typed `ThemeRuleSet` consumed by `ThemeMaterializer`. The wire permits the existing global, family,
variant, and ordinal selector forms. Version 1 rejects a concrete effect ID because the authoring
envelope does not carry effect graphs; authors who need effects use a complete
`DiagramThemeSpec`.

Unknown top-level or nested fields are rejected. Serialized, cached, or cross-process definitions
must carry both version fields. A Rust-only ephemeral builder may select the current versions before
serialization, but persisted data never silently means `latest`. Unknown versions fail closed or
enter an explicit migration operation. Before `C7a-contract`, this candidate version 1 table may be
corrected, reduced, or reordered as an alpha breaking change regardless of the ADR status. Once
`C7a-contract` records expansion version 1, any semantic change to a default, expansion row, row
order, or collision rule requires a new `expansion_version`.

The contract registry owns the legal version tuple. Version 1 accepts exactly
`(authoring_schema_version = 1, expansion_version = 1)` and produces
`DiagramThemeSpecWireV1` with `spec_schema_version = 1`. Bindings project this registry; they do not
independently accept any pair of known integers. `DiagramThemeSpecWireV1` is the canonical closed
wire used by `theme.spec`, materialization results, golden vectors, and digest calculation.

Canonical `ThemeDefinitionV1` bytes use RFC 8785 JSON Canonicalization Scheme over the Rust-owned
wire: UTF-8, JCS property ordering, typed array order, no insignificant whitespace, and JCS number
serialization including negative-zero normalization. Non-finite values are rejected before
canonicalization. Unspecified optional facets are omitted; explicit clear remains JSON `null`;
typed colors, lengths, paints, selectors, and IDs serialize through their Rust-owned canonical wire
forms. Human-readable JSON and canonical bytes represent the same definition; whitespace is not a
second format. `DiagramThemeSpecWireV1` has its own canonical bytes for materialized-spec identity.
Both serializers are distinct from the compiler's existing binary recipe-fingerprint encoder.

The typed authoring facade is also a projection of the same wire value. It provides discoverable
named setters for the small stable token set and typed constructors for family, target, variant,
ordinal, facet, and `Unspecified | Clear | Value` states. The common typed path does not require raw
wire identifiers. These helpers only construct `ThemeDefinitionV1`; they do not own defaults,
expansion, precedence, capability discovery, or execution policy. JSON-to-typed-to-JSON and
typed-to-JSON-to-typed round trips must produce identical canonical definition bytes.

Golden vectors cover Rust construction, JSON decode/re-encode, and generated SDK projections. Each
binding proves schema round-trip, tri-state preservation, bounded admission, and one end-to-end
transport smoke against the shared vectors; it does not repeat the Rust-owned expansion matrix.

External materialization operations apply the host profile's encoded-byte and collection ceilings
before typed decoding. They then validate string, rule, palette, and nested collection bounds while
decoding. The pure `ThemeMaterializer` receives only an admitted typed definition.

Fatal authoring errors return no partial spec.

### 4. Define omission, null, and clear semantics across transports

Version 1 uses one rule throughout the convenience layer: omission selects a documented default;
`null` is rejected except at an existing clearable style facet, where it encodes
`Specified::Clear`. It never clears token defaults or an entire collection or patch group.

| Location | Omitted | `null` | Empty value | Explicit clear |
| --- | --- | --- | --- | --- |
| `ThemeDefinitionV1.tokens` | Fatal: required | Fatal | `{}` selects all defaults | Not available |
| Scalar color token | Use the version 1 default | Fatal | Invalid color is fatal | Not available |
| `tokens.typography` | Use all typography defaults | Fatal | `{}` selects all typography defaults | Not available |
| Typography field | Use its version 1 default | Fatal | Invalid or out-of-range value is fatal | Not available |
| `tokens.series` | Use the version 1 palette | Fatal | Empty list is fatal | Not available |
| `ThemeDefinitionV1.styles` | Empty authored `ThemeRuleSet` | Fatal | Empty tagged array is valid | Not at collection level |
| A facet inside an authored style patch | `Specified::Unspecified` | `Specified::Clear` | Existing facet validation applies | JSON `null` and Rust `Specified::Clear` |

Only the existing typed style-patch seam supports clear. The authoring facade does not invent a
second reset syntax or let `null` erase token defaults.

JSON decoders preserve field presence before converting to typed values. They must distinguish a
missing field from an explicit `null`. Typed non-JSON bindings use an explicit
`Unspecified | Clear | Value` union for clearable style facets. Generated SDK tests construct each
state through the SDK itself; replaying one JSON string through every binding is not sufficient
evidence of tri-state preservation.

### 5. Define the candidate version 1 token vocabulary and defaults

Every field inside `ThemeTokensV1` is optional and defaulted. The `tokens` object itself remains
required so an authoring definition cannot be confused with a complete spec or a preset selector.
Wire identifiers use `snake_case` and project from one Rust-owned executable contract table.

| Order | Field | Type | Version 1 default |
| ---: | --- | --- | --- |
| 1 | `canvas` | `ThemeColorValue` | `#ffffff` |
| 2 | `surface` | `ThemeColorValue` | `#f8fafc` |
| 3 | `surface_alt` | `ThemeColorValue` | `#e2e8f0` |
| 4 | `surface_muted` | `ThemeColorValue` | `#f1f5f9` |
| 5 | `text` | `ThemeColorValue` | `#0f172a` |
| 6 | `subtle_text` | `ThemeColorValue` | `#475569` |
| 7 | `border` | `ThemeColorValue` | `#94a3b8` |
| 8 | `line` | `ThemeColorValue` | `#64748b` |
| 9 | `accent` | `ThemeColorValue` | `#2563eb` |
| 10 | `error` | `ThemeColorValue` | `#dc2626` |
| 11 | `warning` | `ThemeColorValue` | `#d97706` |
| 12 | `success` | `ThemeColorValue` | `#059669` |
| 13 | `series` | Non-empty list of `ThemeColorValue`, maximum 256 | `[#2563eb, #16a34a, #d97706, #9333ea]` |
| 14 | `typography` | `ThemeAuthoringTypographyV1` | See the table below |

`ThemeAuthoringTypographyV1` is intentionally smaller than `TypographySpec`:

| Order | Field | Type | Version 1 default |
| ---: | --- | --- | --- |
| 1 | `font_stack` | Non-empty list of font-family strings, maximum 32 | `["Inter", "ui-sans-serif", "system-ui", "sans-serif"]` |
| 2 | `font_size_px` | Finite positive number | `16` |
| 3 | `font_weight` | Integer from 1 through 1000 | `400` |
| 4 | `line_height` | Existing `LineHeight` wire: `"normal"`, positive multiplier, or `{ "px": positive number }` | `"normal"` |

Family-specific actor, note, activation, cluster, message, task, and similar fields are not part of
`ThemeTokensV1`. Radius, content padding, stroke width, elevation/shadow, and spacing also remain
rule-only or alpha until they have unambiguous cross-family expansion, resource bounds, and terminal
consumer evidence. Advanced authors use the existing `ThemeRuleSet` or a complete
`DiagramThemeSpec`.

The existing alpha-only family fields migrate as follows; no binding keeps them as aliases:

| Removed alpha field | Version 1 default source | Custom replacement |
| --- | --- | --- |
| `edge_label_background` | `canvas` | Rule for `edge-label-background.fill` |
| `cluster_background` | `surface_muted` | Rule for `cluster.fill` |
| `cluster_border` | `border` | Rule for `cluster.stroke` |
| `note_background` | `surface_alt` | Rule for `note.fill` |
| `note_border` | `border` | Rule for `note.stroke` |
| `note_text` | `text` | Rule for `note-label.fill` |
| `actor_background` | `surface` | Rule for `actor.fill` |
| `actor_border` | `border` | Rules for `actor.stroke` and any independently desired `lifeline` paint |
| `actor_text` | `text` | Rules for `actor-label.fill` and `loop-label.fill` |
| `activation_background` | `surface_alt` | Rule for `activation.fill` |
| `activation_border` | `border` | Rule for `activation.stroke` |

Because this is an alpha contraction, removed fields are rejected rather than silently accepted and
ignored. Built-in presets that relied on distinct values move those values into their complete
`ThemeRuleSet` recipes.

### 6. Define candidate expansion version 1

Expansion version 1 creates a complete spec with:

- `canvas.base = solid(tokens.canvas)`;
- `typography.default` populated from the four authoring typography fields;
- the generated rules and palettes below;
- authored styles composed according to the rules below; and
- default or empty values for every other `DiagramThemeSpec` section.

No color algorithm, contrast adjustment, target probing, family probing, or host-dependent branch
runs during expansion. In the table, `fill = X` means a solid paint and `stroke = X` means a solid
`stroke.paint`. Rows are emitted in exactly this order:

| Rule index | Target | Variant | Emitted facets |
| ---: | --- | --- | --- |
| 0 | `text` | - | `fill = text` |
| 1 | `title` | - | `fill = text` |
| 2 | `node` | - | `stroke = border` |
| 3 | `node-label` | - | `fill = text` |
| 4 | `edge` | - | `fill = line`, `stroke = line` |
| 5 | `edge-label` | - | `fill = text` |
| 6 | `edge-label-background` | - | `fill = canvas` |
| 7 | `cluster` | - | `fill = surface_muted`, `stroke = border` |
| 8 | `cluster-label` | - | `fill = subtle_text` |
| 9 | `actor` | - | `fill = surface`, `stroke = border` |
| 10 | `actor-label` | - | `fill = text` |
| 11 | `lifeline` | - | `fill = line`, `stroke = line` |
| 12 | `message` | - | `fill = line`, `stroke = line` |
| 13 | `message-label` | - | `fill = text` |
| 14 | `loop` | - | `fill = surface_alt`, `stroke = border` |
| 15 | `loop-label` | - | `fill = text` |
| 16 | `state` | - | `fill = surface`, `stroke = border` |
| 17 | `state-label` | - | `fill = text` |
| 18 | `transition` | - | `fill = line`, `stroke = line` |
| 19 | `transition-marker` | - | `fill = line`, `stroke = line` |
| 20 | `transition-label` | - | `fill = text` |
| 21 | `transition-label-background` | - | `fill = canvas` |
| 22 | `composite` | - | `fill = canvas`, `stroke = border` |
| 23 | `composite-header` | - | `fill = surface_alt`, `stroke = border` |
| 24 | `composite-label` | - | `fill = text` |
| 25 | `special-state` | `special` | `fill = accent`, `stroke = accent` |
| 26 | `special-state-inner` | `end` | `fill = canvas`, `stroke = canvas` |
| 27 | `marker` | - | `fill = accent`, `stroke = accent` |
| 28 | `note` | - | `fill = surface_alt`, `stroke = border` |
| 29 | `note-label` | - | `fill = text` |
| 30 | `activation` | - | `fill = surface_alt`, `stroke = border` |
| 31 | `task` | - | `fill = surface`, `stroke = border` |
| 32 | `task` | `active` | `fill = surface_muted`, `stroke = line` |
| 33 | `task` | `error` | `fill = surface_alt`, `stroke = error` |
| 34 | `task` | `warning` | `fill = surface_alt`, `stroke = warning` |
| 35 | `task` | `success` | `fill = surface_alt`, `stroke = success` |
| 36 | `requirement` | - | `fill = surface`, `stroke = border` |
| 37 | `relation` | - | `fill = line`, `stroke = line` |
| 38 | `table` | `odd` | `fill = surface` |
| 39 | `table` | `even` | `fill = surface_alt` |

Expansion then creates ordinal palettes in this fixed order, each using the complete `series` list:

| Palette index | Target |
| ---: | --- |
| 0 | `node` |
| 1 | `pie-slice` |

The materializer only emits these palettes; it does not define one global palette-versus-fill
precedence rule. The Flowchart/Swimlane Node adapter must prove that its generated Node palette is a
fallback when no matching fill winner exists. The Pie adapter must separately prove the PieSlice
palette behavior used by its terminal writer. These per-target precedence rules belong to the C5
family-program contract and row-coverage manifest, not to `DiagramThemeCompiler` capability
collection. Changing `series[0]` must change the first applicable Node and PieSlice terminal fill in
their respective authoring witnesses.

`axis`, `legend`, `chart-series`, `timeline-event`, and `journey-task` are deliberately absent from
expansion version 1. Their only current semantic consumers are XY/Quadrant/Radar, Timeline, or
Journey families scheduled for C7b breadth, outside the C7a pre-freeze witness set. They may enter a
later expansion version after those family writers and terminal witnesses exist.

Composition with `ThemeDefinitionV1.styles` is deterministic:

1. Generated rules 0 through 39 are emitted first.
2. Authored rules are appended in their input order. Rule collisions are not deduplicated; the
   existing selector specificity and source-order winner semantics remain authoritative.
3. An authored ordinal palette for one of the two generated targets replaces that target's colors
   in the existing generated palette slot. It does not append a duplicate or change palette order.
4. A non-colliding authored ordinal palette appends after the two generated slots in authored input
   order.
5. Two authored ordinal palettes for the same target are a fatal duplicate-target error, whether or
   not that target also has a generated palette.

The complete spec retains the existing 512-rule ceiling. The executable table derives
`MAX_AUTHORED_RULES = 512 - GENERATED_RULE_COUNT`; for the current 40-row candidate this is 472.
Inputs with 473 authored rules fail with the dedicated rule-budget diagnostic before constructing a
partial spec. If the candidate row set changes before `C7a-contract`, the derived boundary and golden
vectors change with it; after `C7a-contract`, changing the boundary requires a new expansion version.
Concrete effect references in authored rules also fail with a dedicated diagnostic because version
1 does not carry effect graphs.

The executable contract table in `merman-render` must generate or verify this expansion. Bindings do
not copy these rows or implement the collision algorithm. Before `C7a-candidate`, every generated
rule facet and palette target must appear in a row-coverage manifest that names at least one direct
typed consumer and terminal witness. A row without coverage is removed from expansion version 1
rather than retained as a convenience value that only creates a residual. Moving this ADR from
`proposed` to `accepted` approves the design and ownership boundary only; it neither certifies that
coverage nor freezes compatibility. Only `C7a-contract` does that.

### 7. Make the definition the share identity and keep preset governance separate

The ordinary user-facing hierarchy is:

```text
ThemeDefinitionV1 = authored, saved, and shared value
MaterializedTheme = inspectable deterministic expansion
DiagramThemeSpec = advanced complete recipe and low-level render input
DiagramTheme = compiled executable theme
```

`MaterializedTheme` carries the authoring and expansion versions, complete-spec schema version,
editable complete spec, and bounded diagnostics. Canonical `ThemeDefinitionV1` bytes identify the
shared authored value. A `ThemeMaterializationDigest` may additionally bind the version tuple and
canonical complete spec for cache or replay use, but it excludes diagnostics and display messages
and is not the primary share identity. It remains alpha until a first-party cache or replay consumer
demonstrates that it must be a stable public field.

The materialization digest is not the compiler-owned `ThemeRecipeFingerprint`, a preset
qualification receipt, an artifact digest, or runtime evidence. If a future caller needs a hash of
only canonical spec bytes, that type must be named `CanonicalThemeSpecDigest`.

Preset revision, maturity, and qualification metadata belong to the preset catalog and release
governance, not to the version 1 authoring envelope. Version 1 therefore does not add
`MaterializedPreset`, `PresetRef`, or `PresetMaterializationDigest` as stable authoring concepts. A
preset may still be selected by the existing render convenience. A copy/export action returns its
self-contained `ThemeDefinitionV1` when that representation is lossless, and otherwise returns a
clearly labeled complete `DiagramThemeSpec`. The system does not infer a compact definition from an
arbitrary complete recipe.

### 8. Keep diagnostics and expansion traces at different maturity levels

Materialization uses the closed `ThemeMaterializationDiagnosticV1` envelope. It carries
`code`, `severity`, an RFC 6901 JSON Pointer `path`, bounded structured `details`, and a non-identity
display message. Fatal failures return `ThemeMaterializationErrorV1 { schema_version: 1,
diagnostics }` and no `MaterializedTheme`. Diagnostics sort by UTF-8 path bytes, then code, then the
canonical details encoding.

The version 1 fatal code registry is:

| Code | Meaning |
| --- | --- |
| `theme-authoring.unsupported-schema-version` | The authoring schema version is unknown or illegal in the requested tuple. |
| `theme-authoring.unsupported-expansion-version` | The expansion version is unknown or illegal in the requested tuple. |
| `theme-authoring.missing-required-field` | A required field is absent. |
| `theme-authoring.disallowed-null` | A non-clearable field is explicitly null. |
| `theme-authoring.unknown-field` | A closed authoring object contains an unknown field. |
| `theme-authoring.invalid-token-value` | A token or typography value is malformed or outside its numeric domain. |
| `theme-authoring.empty-series` | The required series palette is empty. |
| `theme-authoring.rule-budget-exceeded` | The derived `MAX_AUTHORED_RULES` limit was exceeded. |
| `theme-authoring.duplicate-palette-target` | Two authored ordinal palettes target the same semantic target. |
| `theme-authoring.effect-reference-not-supported` | An authored rule references an effect graph that the version 1 envelope cannot carry. |
| `theme-authoring.resource-limit-exceeded` | Encoded bytes or a bounded string or collection exceeds the host admission ceiling. |

Diagnostic `details` objects are closed per code. Version diagnostics carry `actual` and the legal
tuple registry. Field diagnostics carry the offending field name. Value diagnostics carry only a
bounded expected-domain ID, never raw unbounded input. Budget diagnostics carry stable `limit_id`,
`actual`, and `max` values. Duplicate-palette diagnostics carry the target ID and both authored
indices. These shapes are part of the same diagnostic schema version and have cross-binding golden
vectors.

Expansion trace is not part of either stable result envelope. An internal or explicitly alpha
`inspect_theme_authoring` operation may return a versioned `ThemeAuthoringTrace` that points to
generated rule indices, palette indices, authored entries, and defaults. It may join that trace with
compiler-owned resolution to explain a theme-internal winner. It must not re-match selectors,
reorder rules, expose family writer ledgers, or claim runtime application or portability.

### 9. Keep discovery and runtime evidence separate

The candidate operations are:

```text
materialize_theme(definition) -> MaterializedTheme
describe_theme_support(query) -> ThemeCapabilityDescriptor
```

First-party render facades may additionally accept an admitted `ThemeDefinitionV1` and compose
materialization, compilation, and rendering in one call. That orchestration is a convenience over
the same authorities, not a new renderer input or materialization implementation.

`describe_theme_support` returns a C5-owned static upper bound such as `Unconditional`,
`Conditional`, `NotApplicable`, `Unsupported`, or `Unverified`. `Unconditional` requires an
exhaustive value-domain argument and shared runtime admission predicate owned by the family or
target module; the representative C6a ledger validates end-to-end integration and detects drift but
cannot upgrade a descriptor merely because one fixture passed. `NotApplicable` means the semantic
target or facet does not exist for the queried family. Missing, unknown, or newer additive rows
normalize to `Unverified`, never to `Unsupported`.

The stable discovery seam is a versioned query and result envelope, not an exported copy of the
private mechanism matrix. Its stable fields identify family, output target, semantic target, facet,
coarse support state, and bounded reason IDs. Selector subclasses, value classes, label modes, and
host-policy predicates remain alpha until at least one first-party authoring consumer proves that the
additional dimension is required. The wire preserves unknown string IDs and additive rows so an old
consumer can display a newer catalog without claiming it can execute the new family.

Only a concrete render or export report can report actual `Applied`, residual, portability, and
admission results.

### 10. Assign implementation ownership narrowly

- A dependency-neutral theme-contract module below both `merman-render` and
  `merman-bindings-core` owns every persisted `*WireV1` type, the legal version-tuple registry, and
  canonical wire serialization. It may be a small dedicated crate or an equally dependency-neutral
  lower shared module; it must not import renderer or binding types.
- `merman-render` owns decoding `ThemeRuleSetWireV1[]` into the in-memory typed `ThemeRuleSet`, the
  executable token contract table, `ThemeMaterializer`, expansion, materialization semantics, and
  complete typed results. It consumes canonical bytes from the shared contract module and never
  imports `merman-bindings-core`.
- `merman-bindings-core` owns transport admission and external envelopes only; it projects or
  generates SDK types from the dependency-neutral wire contract and does not own a second
  serializer. Authoring wire ownership therefore introduces no dependency from
  `merman-bindings-core` to `merman-render`; any unrelated pre-existing execution dependency is not
  an authority for wire or canonicalization.
- First-party bindings call the Rust-owned operations through the existing target-independent
  `merman::diagram_theme` facade. They may expose a one-step definition-to-render convenience, but
  they do not import renderer internals, expand tokens, reorder rules, canonicalize wires, or
  resolve palette collisions locally. Terminal/ASCII styling remains a separate contract.
- The former alpha `ThemeTokens::into_theme_spec` and `ThemePreset::spec` paths are deleted;
  `DiagramThemeCompiler::compile_preset` delegates through the same versioned materializer and
  complete-spec decoder. Family-specific token fields and duplicate expansion tables are not
  retained as compatibility implementations. Preset catalog metadata stays outside the authoring
  wire and materializer.
- The renderer and private family adapters remain unaware of the original authoring form.
- Full asset-bearing complete specs remain subject to encoded-byte and effective runtime resource
  limits after materialization and during compilation/session admission.

The Rust executable table and alpha materializer land after the current correctness gates and the
representative native C6a checkpoint, then drive the C7a pre-freeze authoring witnesses. Those
witnesses reuse production `RenderedDocument` receipts and shared observers; they prove authoring,
materialization, and state isolation without establishing another rendering proof system. Public
binding operations and authoring UI roll out only after the C7a candidate is backed by C5, C6a, the
expansion row-coverage manifest, and the pre-freeze family consumers. This ADR alone does not make
C7a eligible.

## Consequences

- Common theme authoring becomes smaller without weakening the complete typed recipe language.
- The authoring module is deep: callers share one small value and may use either one composed render
  convenience or the explicit materialization operation. Canonical wire serialization remains in
  the dependency-neutral contract module; defaults, expansion, ordering, collision handling, and
  semantic validation remain local to `merman-render`.
- The compiler, capability catalog, preset qualification, and runtime evidence retain separate
  authorities.
- Stored authoring inputs are replayable because expansion behavior is explicit and versioned.
- Presets remain optional conveniences rather than product identity or a separate theme engine.
- Existing family-specific alpha token fields require migration into versioned defaults or explicit
  rules.
- ADR acceptance approves the module boundary and candidate contract; it does not by itself create
  a compatibility promise. `C7a-contract` is the sole version-freeze event. Before that event,
  unproven rows are corrected or removed as alpha breaking changes; after it, changing a version 1
  default or expansion row requires expansion version 2 rather than silent drift.

## Rejected Alternatives

1. Accept tokens directly in render options.
   This creates a third render-time input and makes materialization depend on execution interfaces.
2. Support preset plus overrides.
   This requires merge precedence, null/clear behavior, preset revision migration, and patch rules
   that form another theme language.
3. Expose a public primitive-to-semantic-to-family recipe hierarchy in version 1.
   The existing `ThemeRuleSet` already expresses advanced semantic and family overrides. Wrapping it
   again would create a shallow duplicate interface.
4. Add a `mode` field or output-target conditions.
   Callers can materialize separate definitions. Runtime conditions would make expansion depend on
   execution context and undermine replay.
5. Give bindings local token helpers.
   Generated types are acceptable; separate expansion implementations are not.
6. Let materialization report capabilities or portability.
   Those facts depend on compilation, family writers, target export, and session policy.
7. Expose CSS selectors or arbitrary token algorithms.
   They bypass semantic targets, complicate portability proof, and turn the library into a styling
   runtime.
8. Adopt DTCG or CSS variables as the renderer input.
   They may be future import/export adapters, but neither format represents diagram family targets,
   variants, terminal evidence, or admission.
9. Add theme packages, manifests, install commands, lock files, or a remote registry.
   A self-contained definition or complete spec already satisfies copying, files, generated SDKs,
   and Playground links. Distribution infrastructure requires a separate demonstrated product need.
