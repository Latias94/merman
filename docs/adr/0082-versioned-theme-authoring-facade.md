# ADR 0082: Versioned Theme Authoring Facade

- Status: accepted
- Date: 2026-08-14
- Accepted: 2026-08-21

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

This ADR defines the accepted alpha authoring design used by the C7a pre-freeze authoring
witnesses. It does not declare a stable public interface, satisfy C6a, or unblock C7a. C7a remains
blocked until the plan's compiler, family-writer, terminal-evidence, rollout, and author-task gates
close. This accepted ADR is the normative candidate source of truth for the version 1 authoring
envelope and expansion tables. Acceptance records design approval only; it does not certify every
implementation projection, freeze an expansion row, or create a compatibility promise. Only
`C7a-contract` begins the compatibility and expansion-version freeze after the required consumer,
terminal-evidence, and rollout witnesses pass.

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

The current built-in presets are definition-backed internally, but the catalog appends Mermaid
`theme = base` and `dark_mode` compatibility values after materialization. Until those values are
represented losslessly by `ThemeDefinitionV1`, preset export returns `complete_spec`, not a compact
definition. That export is a catalog convenience operation; it is not a third authoring language or
an additional low-level renderer input.

### Candidate user workflows and non-claims

The pre-freeze candidate must support these ordinary tasks through one Rust-owned contract:

1. construct or decode a self-contained definition;
2. persist readable JSON and recover identical canonical definition bytes;
3. materialize the definition into an inspectable complete spec;
4. compile and render through the existing closed renderer;
5. query a bounded static support claim without confusing it with runtime application; and
6. export a preset as either a lossless definition or an explicitly labeled complete spec.

This candidate does not promise complete theming for every family, automatic contrast correction,
browser/JPEG/PDF qualification, stable preset cells, a registry, or a frozen cross-language ABI.
Those claims require their own family, target, rollout, and release gates.

### 2. Separate authoring, compilation, and execution authorities

The only valid authority chain is:

```text
ThemeDefinitionV1
    -> ThemeMaterializer
    -> MaterializedThemeWireV1 with a complete DiagramThemeSpec

DiagramThemeSpec
    -> DiagramThemeCompiler
    -> required capabilities + ThemeRecipeFingerprint + compile diagnostics

RenderedDocument / export report
    -> actual application + residuals + portability + target admission
```

`ThemeMaterializer` is a pure deterministic lowering module. It consumes an admitted
`ThemeDefinitionV1`, owns authoring defaults, token expansion, validation of its own input, and
authoring diagnostics, and composes complete-spec wire entries. It does not decode authored rule
entries into renderer-owned typed rules; `DiagramThemeCompiler` remains the sole typed rule decoder.
It does not inspect renderer
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
wire rather than introducing an authoring-only container object. `ThemeMaterializer` preserves and
composes these admitted wire entries; the complete-spec compiler later performs the only decode into
the in-memory typed `ThemeRuleSet`. The wire permits the existing global, family, variant, and
ordinal selector forms. Version 1 rejects a concrete effect ID because the authoring envelope does
not carry effect graphs; authors who need effects use a complete `DiagramThemeSpec`.

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
wire used by `theme.spec`, materialization results, and golden vectors.

Canonical `ThemeDefinitionV1` bytes use RFC 8785 JSON Canonicalization Scheme over the Rust-owned
wire: UTF-8, JCS property ordering, typed array order, no insignificant whitespace, and JCS number
serialization including negative-zero normalization. Non-finite values are rejected before
canonicalization. Unspecified optional facets are omitted; explicit clear remains JSON `null`;
typed colors, lengths, paints, selectors, and IDs serialize through their Rust-owned canonical wire
forms. Human-readable JSON and canonical bytes represent the same definition; whitespace is not a
second format. `DiagramThemeSpecWireV1` has its own canonical bytes for materialized-spec identity.
Both serializers are distinct from the compiler's existing binary recipe-fingerprint encoder.

The typed authoring facade is also a projection of the same wire value. It provides discoverable
named setters for the small candidate token set and typed constructors for family, target, variant,
ordinal, facet, and `Unspecified | Clear | Value` states. The common typed path does not require raw
wire identifiers. These helpers only construct `ThemeDefinitionV1`; they do not own defaults,
expansion, precedence, capability discovery, or execution policy. JSON-to-typed-to-JSON and
typed-to-JSON-to-typed round trips must produce identical canonical definition bytes.

Contract-owned light and dark golden vectors cover Rust construction, JSON decode/re-encode,
bindings-core materialization, canonical complete-spec replay, and generated SDK projections. Each
binding proves schema round-trip, tri-state preservation, bounded admission, and one end-to-end
transport smoke against those same vectors; it does not copy or recompute the Rust-owned expansion
matrix.

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
| 6 | `border` | `ThemeColorValue` | `#94a3b8` |
| 7 | `line` | `ThemeColorValue` | `#64748b` |
| 8 | `accent` | `ThemeColorValue` | `#2563eb` |
| 9 | `series` | Non-empty list of `ThemeColorValue`, maximum 256 | `[#2563eb, #16a34a, #d97706, #9333ea]` |
| 10 | `typography` | `ThemeAuthoringTypographyV1` | See the table below |

`ThemeAuthoringTypographyV1` is intentionally smaller than `TypographySpec`:

| Order | Field | Type | Version 1 default |
| ---: | --- | --- | --- |
| 1 | `font_stack` | Non-empty list of font-family strings, maximum 32 | `["Inter", "ui-sans-serif", "system-ui", "sans-serif"]` |
| 2 | `font_size_px` | Finite positive number | `16` |
| 3 | `font_weight` | Integer from 1 through 1000 | `400` |

`line_height` remains available in the advanced complete-spec typography wire, but is deliberately
absent from the compact token vocabulary. No family currently has a direct portable base
line-height consumer, so accepting it here would materialize an author request that every family
must classify as unsupported or residual. A later expansion version may add it only after a direct
measurement, layout, writer, and terminal-evidence path exists.

Family-specific actor, note, activation, cluster, message, task, and similar fields are not part of
`ThemeTokensV1`. Radius, content padding, stroke width, elevation/shadow, and spacing also remain
rule-only or alpha until they have unambiguous cross-family expansion, resource bounds, and terminal
consumer evidence. Advanced authors use the existing `ThemeRuleSet` or a complete
`DiagramThemeSpec`.

`subtle_text`, `error`, `warning`, and `success` are also absent from expansion version 1. They had
no direct generated consumer after row-coverage pruning, so retaining them would accept author input
that materializes to no visual change. A later expansion version may add a semantic role only after
at least one direct family writer and terminal witness exist.

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
- `typography.default` populated from the three authoring typography fields;
- the generated rules and palettes below;
- authored styles composed according to the rules below; and
- default or empty values for every other `DiagramThemeSpec` section.

No color algorithm, contrast adjustment, target probing, family probing, or host-dependent branch
runs during expansion. In the table, `fill = X` means a solid paint and `stroke = X` means a solid
`stroke.paint`. Rows are emitted in exactly this order:

| Rule index | Target | Variant | Emitted facets | Direct consumer | Terminal witness |
| ---: | --- | --- | --- | --- | --- |
| 0 | `text` | - | `fill = text` | State | State terminal SVG/C6a evidence |
| 1 | `title` | - | `fill = text` | State | `state_svg_title_theme_fill_reaches_the_terminal_svg_text_paint` |
| 2 | `node` | - | `stroke = border` | Flowchart | Scalar route-cutover SVG/PNG evidence |
| 3 | `edge` | - | `stroke = line` | Flowchart | Scalar route-cutover SVG/PNG evidence |
| 4 | `cluster` | - | `fill = surface_muted`, `stroke = border` | Flowchart | Scalar route-cutover SVG/PNG evidence |
| 5 | `actor` | - | `fill = surface`, `stroke = border` | Sequence | Scalar route-cutover SVG/PNG evidence |
| 6 | `lifeline` | - | `stroke = line` | Sequence | Scalar route-cutover SVG/PNG evidence |
| 7 | `message` | - | `stroke = line` | Sequence | Scalar route-cutover SVG/PNG evidence |
| 8 | `state` | - | `fill = surface`, `stroke = border` | State | State terminal SVG/C6a evidence |
| 9 | `state-label` | - | `fill = text` | State | State terminal SVG/C6a evidence |
| 10 | `transition` | - | `stroke = line` | State | State terminal SVG evidence |
| 11 | `transition-marker` | - | `fill = line`, `stroke = line` | State | State structural-role terminal SVG evidence |
| 12 | `transition-label` | - | `fill = text` | State | State structural-role terminal SVG evidence |
| 13 | `transition-label-background` | - | `fill = canvas` | State | State structural-role terminal SVG evidence |
| 14 | `composite` | - | `fill = canvas`, `stroke = border` | State | State structural-role terminal SVG evidence |
| 15 | `composite-header` | - | `fill = surface_alt`, `stroke = border` | State | State structural-role terminal SVG evidence |
| 16 | `composite-label` | - | `fill = text` | State | State structural-role terminal SVG evidence |
| 17 | `special-state` | `special` | `fill = accent`, `stroke = accent` | State | State split-surface terminal SVG evidence |
| 18 | `special-state-inner` | `end` | `fill = canvas`, `stroke = canvas` | State | State split-surface terminal SVG evidence |
| 19 | `note` | - | `fill = surface_alt`, `stroke = border` | Sequence | Scalar route-cutover SVG/PNG evidence |
| 20 | `note-label` | - | `fill = text` | State | State terminal SVG evidence |
| 21 | `activation` | - | `fill = surface_alt`, `stroke = border` | Sequence | Scalar route-cutover SVG/PNG evidence |
| 22 | `entity` | - | `fill = surface`, `stroke = border` | ER | `er_tokens_only_definition_reaches_the_model_owned_entity_surface` |

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

1. Generated rules 0 through 22 are emitted first.
2. Authored rules are appended in their input order. Rule collisions are not deduplicated; the
   existing selector specificity and source-order winner semantics remain authoritative.
3. An authored ordinal palette for one of the two generated targets replaces that target's colors
   in the existing generated palette slot. It does not append a duplicate or change palette order.
4. A non-colliding authored ordinal palette appends after the two generated slots in authored input
   order.
5. Two authored ordinal palettes for the same target are a fatal duplicate-target error, whether or
   not that target also has a generated palette.

The complete spec retains the existing 512-rule ceiling. The executable table derives
`MAX_AUTHORED_RULES = 512 - GENERATED_RULE_COUNT`; for the current 23-row candidate this is 489.
Inputs with 490 authored rules fail with the dedicated rule-budget diagnostic before constructing a
partial spec. If the candidate row set changes before `C7a-contract`, the derived boundary and golden
vectors change with it; after `C7a-contract`, changing the boundary requires a new expansion version.
Concrete effect references in authored rules also fail with a dedicated diagnostic because version
1 does not carry effect graphs.

Development snapshots before this contraction generated `requirement.fill/stroke` and treated the
`requirement` target as applicable to ER. The model-owned ER target is now `entity`; persisted
family-scoped ER rules must replace `family = "er", target = "requirement"` with
`family = "er", target = "entity"`. The Requirement family keeps the `requirement` target. Because
version 1 remains an unfrozen alpha candidate, this correction updates expansion version 1 in place
and does not add a compatibility alias or a second lowering path.

This ADR owns the normative candidate row order and semantics. The executable contract table in
`merman-render` is the sole runtime expansion authority, and CI must compare it exactly with the
normative table and shared canonical vectors. Bindings do not copy these rows or implement the
collision algorithm. Before `C7a-candidate`, every generated rule facet, palette target,
`canvas.base`, and compact typography property must appear in a row-coverage manifest that names at
least one direct typed consumer and terminal witness. A row or token output without coverage is
removed from expansion version 1 rather than retained as a convenience value that only creates a
residual. ADR acceptance approves this design and ownership boundary only; it neither certifies all
coverage nor freezes compatibility. Only `C7a-contract` does that.

The pre-freeze family-shape review uses the same authoring wire and compiler rather than adding new
sum types:

| Family | Representative authored shape | Terminal proof before candidate |
| --- | --- | --- |
| Class | family-qualified `edge.stroke` | Real relation path and the referenced composition marker both carry the authored stroke. |
| Gantt | family-qualified `task.fill` plus `task.radius` | The same task occurrence carries terminal fill and layout-owned `rx`/`ry`. |
| Pie | `pie-slice` ordinal palette | Every slice and matching legend swatch carry the authored ordinal colors. |
| ER | family-qualified rules plus generated `entity` row | Existing entity shell and relation receipts prove the current rule shape; no new authoring variant is required. |
| Architecture | family-qualified cluster/edge rules | Existing target/facet/variant forms are sufficient; the review adds no expansion row or new wire case. |
| C4 | family-qualified cluster rules with existing variants and clear semantics | Existing rule and `Specified::Clear` forms are sufficient; the review adds no expansion row or new wire case. |

### 7. Make the definition the share identity and keep preset governance separate

The ordinary user-facing hierarchy is:

```text
ThemeDefinitionV1 = authored, saved, and shared value
MaterializedThemeWireV1 = inspectable deterministic expansion
DiagramThemeSpec = advanced complete recipe and low-level render input
DiagramTheme = compiled executable theme
```

`MaterializedThemeWireV1` carries its own envelope `schema_version`, the authoring and expansion
versions, the complete-spec schema version, and the editable complete spec. Canonical
`ThemeDefinitionV1` bytes identify the shared authored value; canonical complete-spec bytes identify
the expanded value. Successful materialization does not carry diagnostics, provenance, capability
results, portability claims, or render receipts.

Its serialized form is a closed object with five required, non-null fields:

```text
schema_version = 1
authoring_schema_version = 1
expansion_version = 1
spec_schema_version = 1
spec = DiagramThemeSpecWireV1
```

The envelope `schema_version` versions this result shape independently from the authoring,
expansion, and complete-spec contracts that produced its payload.

Version 1 does not expose a materialization digest. No first-party cache or replay consumer needs
one, and freezing another identity beside canonical definition/spec bytes would add contract cost
without user value. If a future real consumer needs a hash of only canonical spec bytes, that type
must be proposed separately and named `CanonicalThemeSpecDigest`.

Once `C7a-contract` freezes a legal version tuple, every later release that claims the same theme
contract major must continue to decode that tuple and reproduce its canonical expansion. A newer
default or row set uses a new `expansion_version`; a wire-shape change uses a new
`authoring_schema_version`. Removing a frozen tuple requires a new theme-contract major, an explicit
migration operation, a documented prior deprecation cycle, and retained old/new golden vectors.
Catalog visibility may change independently, but stored definitions are never silently
reinterpreted.

Preset revision, maturity, and qualification metadata belong to the preset catalog and release
governance, not to the version 1 authoring envelope. Version 1 therefore does not add
`MaterializedPreset`, `PresetRef`, or `PresetMaterializationDigest` as stable authoring concepts. A
preset may still be selected by the existing render convenience. A copy/export action returns its
self-contained `ThemeDefinitionV1` when that representation is lossless, and otherwise returns a
clearly labeled complete `DiagramThemeSpec`. The system does not infer a compact definition from an
arbitrary complete recipe.

### 8. Keep diagnostics and expansion traces at different maturity levels

The C7a error contract will use the closed `ThemeMaterializationDiagnosticV1` envelope. It carries
`code`, `severity`, an RFC 6901 JSON Pointer `path`, bounded structured `details`, and a non-identity
display message. Fatal failures will return `ThemeMaterializationErrorV1 { schema_version: 1,
diagnostics }` and no `MaterializedThemeWireV1`. Version 1 is deliberately fail-fast and requires
exactly one diagnostic; aggregating multiple independently discovered failures requires a later
error-schema version rather than a second validation pass. Successful results omit diagnostics
entirely; version 1 does not freeze an always-empty warning array. Until that envelope lands, the
renderer-owned Rust error is an alpha implementation surface rather than a frozen cross-binding
failure contract.

The version 1 fatal code registry is:

| Code | Meaning |
| --- | --- |
| `theme-authoring.unsupported-version-tuple` | The complete authoring/expansion version tuple is not in the legal registry. |
| `theme-authoring.invalid-definition-json` | The JSON is malformed, has duplicate members, or does not match the closed authoring wire shape. |
| `theme-authoring.invalid-token-value` | A token or typography value is malformed or outside its numeric domain. |
| `theme-authoring.empty-series` | The required series palette is empty. |
| `theme-authoring.rule-budget-exceeded` | The derived `MAX_AUTHORED_RULES` limit was exceeded. |
| `theme-authoring.duplicate-palette-target` | Two authored ordinal palettes target the same semantic target. |
| `theme-authoring.effect-reference-not-supported` | An authored rule references an effect graph that the version 1 envelope cannot carry. |
| `theme-authoring.resource-limit-exceeded` | Encoded bytes or a bounded string or collection exceeds the host admission ceiling. |

Diagnostic `details` objects are closed per code. Version diagnostics carry the actual tuple and
the legal tuple registry. Invalid-JSON diagnostics carry a bounded stable `reason_id` such as
`malformed-json`, `duplicate-object-key`, or `contract-shape`; they never derive identity by parsing
Serde display text. Value diagnostics carry only a bounded expected-domain ID, never raw unbounded
input. Budget diagnostics carry stable `limit_id`, `actual`, and `max` values. Duplicate-palette
diagnostics carry the target ID and both authored indices. These shapes are part of the same
diagnostic schema version and have cross-binding golden vectors.

Expansion trace is not part of either stable result envelope. An internal or explicitly alpha
`inspect_theme_authoring` operation may return a versioned `ThemeAuthoringTrace` that points to
generated rule indices, palette indices, authored entries, and defaults. It may join that trace with
compiler-owned resolution to explain a theme-internal winner. It must not re-match selectors,
reorder rules, expose family writer ledgers, or claim runtime application or portability.

### 9. Keep discovery and runtime evidence separate

The candidate authoring and discovery operations are:

```text
materialize_theme(definition) -> MaterializedThemeWireV1
describe_theme_support(query_v1) -> ThemeCapabilityDescriptorV1
```

First-party render facades may additionally accept an admitted `ThemeDefinitionV1` and compose
materialization, compilation, and rendering in one call. That orchestration is a convenience over
the same authorities, not a new renderer input or materialization implementation.

`describe_theme_support` returns a C5-owned static upper bound such as `Unconditional`,
`Conditional`, `NotApplicable`, `Unsupported`, or `Unverified`. The V1 query identifies family and
output plus one tagged subject:

```text
rule { target, facet }
ordinal-palette { target }
base-typography { property }
unknown { kind, bounded opaque fields }
```

This subject union is necessary because palettes and family-wide typography are not rule facets.
The unpublished four-axis query has been removed. The subject-based query is the first public
v1 contract; development revisions do not create additional public compatibility versions.

`Unconditional` requires an exhaustive value-domain argument and shared runtime admission predicate
owned by the family or target module; the representative C6a ledger validates end-to-end
integration and detects drift but cannot upgrade a descriptor merely because one fixture passed.
`NotApplicable` means the subject does not exist for the queried family. Missing, unknown, or newer
additive subjects normalize to `Unverified`, never to `Unsupported`.

The candidate discovery seam is a versioned query and result envelope backed by an independently
versioned support-claim projection, not a public copy of the private mechanism matrix. Candidate
fields identify family, output target, tagged subject, coarse support state, and bounded reason IDs.
Selector subclasses, value classes, label modes, and host-policy predicates remain alpha until at
least one first-party authoring consumer proves that the additional dimension is required. The wire
preserves bounded unknown subject fields so an old consumer can display and forward a newer query
without claiming it can execute the new subject.

`export_theme_preset` is a separate alpha catalog convenience. It returns the closed
`PresetExportV1` union (`definition` or `complete_spec`) and follows the catalog's active resource
policy. It is not part of token expansion, support discovery, or the low-level render input contract.

Only a concrete render or export report can report actual `Applied`, residual, portability, and
admission results.

### 10. Assign implementation ownership narrowly

- A dependency-neutral theme-contract module below both `merman-render` and
  `merman-bindings-core` owns every persisted `*WireV1` type, the legal version-tuple registry, and
  canonical wire serialization. It may be a small dedicated crate or an equally dependency-neutral
  lower shared module; it must not import renderer or binding types.
- `merman-render` owns both stages behind distinct seams: `ThemeMaterializer` composes admitted wire
  entries and the executable token contract table into a complete spec wire; then
  `DiagramThemeCompiler` alone decodes `ThemeRuleSetWireV1[]` into the in-memory typed
  `ThemeRuleSet`. It also owns materialization semantics and complete typed results. It consumes
  canonical bytes from the shared contract module and never imports `merman-bindings-core`.
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

The Rust executable table and alpha materializer drive the C7a pre-freeze authoring witnesses. Those
witnesses reuse production `RenderedDocument` receipts and shared observers; they prove authoring,
materialization, canonical light/dark replay, root canvas, compact typography, representative
family shapes, and state isolation without establishing another rendering proof system. Alpha
transport scaffolds may exist before candidate eligibility, but they must remain explicitly
unfrozen and outside stable release claims. Stable binding operations and authoring UI rollout begin
only after the C7a candidate is backed by C5, C6a, the expansion/root/typography coverage manifest,
and the pre-freeze family consumers. This ADR alone does not make C7a eligible.

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
