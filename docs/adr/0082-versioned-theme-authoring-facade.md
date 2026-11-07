# ADR 0082: Versioned Theme Authoring Facade

- Status: accepted
- Date: 2026-08-14
- Accepted: 2026-08-21
- Updated: 2026-11-07 for the current alpha contract
- Related decision: [ADR 0095](0095-general-theme-authoring-and-retained-bindings.md)
  owns the general design vocabulary, retained bindings, and optional appearance layers.

## Context

Merman needs one portable theme document for Rust, command-line use, bindings, and editors.
Implementing token expansion, preset patching, or appearance resolution independently in each
host would create competing authorities for defaults, rule order, resource admission, and errors.

The original compact `ThemeDefinitionV1` proposal is superseded by ADR 0095. Its old expansion
tables, preset-specific creation parameters, and staged freeze plans are not current interfaces.
The maintained alpha contract uses one `ThemeRecipeV1`, including optional appearances inside
its `design`. This coordinated alpha refactor adds neither a second recipe version nor a
compatibility interpreter.

## Decision

### Keep authoring, compilation, and execution separate

The authority chain is:

```text
original JSON or typed ThemeRecipeV1
    -> bounded admission of the complete document
    -> shared design plus one concrete appearance
    -> reference resolution and complete-spec materialization
    -> DiagramThemeCompiler
    -> immutable DiagramTheme
    -> family-owned rendering and output admission
```

`merman-theme-contract` owns portable types and canonical serialization.
`merman-render` owns composition, typed reference resolution, materialization, compilation, and
family painting. `merman-bindings-core` owns raw transport admission and JSON request envelopes.
Rust facades, CLI, and language bindings compose these owners rather than implementing another
materializer. Prepared rendering consumes a concrete compiled theme and performs no host-mode
lookup or authoring traversal per node.

Successful compilation establishes bounded input validity and declared requirements. Static
support queries, curated design scope, and actual output evidence answer different questions.
Only a concrete render or export can establish what was applied, left residual, or admitted for
that target. Private qualification tools do not become a parallel production evidence system.

### Exchange one complete recipe

The closed exchange union is:

```text
ThemeRecipeV1 =
    { schema_version: 1, kind: "design", design: ThemeDesignV1 }
  | { schema_version: 1, kind: "complete_spec", complete_spec: DiagramThemeSpecWireV1 }
```

The version and kind are required. Unknown versions, unknown fields, mixed variants, and
duplicate JSON keys fail before normalization. Raw byte admission preserves the evidence needed
to reject duplicates; parsing into a generic JSON object and reserializing first is insufficient.

The design variant retains named values, references, ordered rules, and its complete optional
`design.appearances` collection. A complete-spec recipe is an intentional concrete snapshot:
it has no declared appearance support and does not reconstruct bindings or a pair from equal
values. Saving a recipe does not require a preset lookup, a package registry, remote loading,
or a new file format. Font installation, licensing, and host capabilities remain external.

### Keep selection outside the document

A render selection is a complete preset, recipe, or specification choice. Preset and recipe
wrappers may contain a concrete `appearance: "light"` or `"dark"`. A direct recipe selects its
declared default. A specification selection cannot carry an appearance.

`ThemeAppearance` contains only Light and Dark. Follow app/editor is host policy: the host
resolves it before submitting a concrete selection. A missing declared appearance is an explicit
selection error. For Follow, a host may use the document default when its own appearance is not
declared; an admission failure never triggers that fallback.

Options JSON treats `theme` atomically. Omission inherits the engine's compiled theme, `null`
clears it, and a supplied object replaces the whole selection. An appearance-only object cannot
recover a recipe from constructor state. Changing an engine's selected appearance resubmits the
whole selection; a prepared source must be prepared again.

Mermaid's literal theme IDs and `darkMode` retain their configuration ownership.
Neither is a substitute for Merman appearance selection. Source paint, typography, layout, canvas,
viewer backing, and export policy remain distinct owners.

### Reuse the three existing JSON operations

| Operation | Input | Result |
| --- | --- | --- |
| `materialize-theme-json` | Raw `ThemeDesignV1`, or a schema-1 request with `action`, `recipe`, and an optional materialization-only `appearance` | Raw-design materialization, complete-document inspection, or selected materialization |
| `export-theme-preset-json` | Plain UTF-8 preset ID | Complete editable `ThemeRecipeV1` |
| `describe-theme-support-json` | Schema-1 family/output/subject query | Coarse mechanism-support descriptor |

The schema-1 authoring actions are `inspect` and `materialize`.
Inspect admits every declared appearance and reports supported/default appearances plus reference
provenance. It accepts no appearance selector. Materialize performs the same whole-document
admission and returns one concrete specification, effective appearance, and its reference
provenance. The raw-design route remains a creation input, not a second recipe version.

Authoring operations do not inherit render `options.theme`. Their recipe and requested appearance
belong to the operation input. Their results are outputs for inspection or explicit snapshots;
ordinary theme exchange retains the authored document.

Preset export uses a plain ID, with no versioned export wrapper. A canonical paired ID such as
`editor` exports the full pair; a concrete ID such as `editor-dark` exports that declared single
appearance. Both use the same document compiler and admission policy.
No new ABI function, operation ID, transport-specific resolver, or native layout is required.

### Discover designs without granting output support

The schema-1 catalog groups canonical designs. Each preset declares supported/default appearances;
its children include exact `concrete_preset_id` values, swatches, family design scope, application
notes, availability, and qualification declarations. Hosts obtain this relationship from metadata
rather than inferring suffixes or maintaining a second mapping catalog.

Availability admits the complete exported document under the effective compiler policy.
Every appearance of a grouped design shares that gate. A passing branch cannot make a rejected
pair available, and a concrete single-appearance export does not certify the full pair.
Design maturity and dedicated/base-only/unreviewed family scope remain separate from availability.
Qualification cells describe named scenarios, not every shape, font, density, or export.

The same catalog identifies only the deliberately declared Mermaid appearance pairs. It preserves
literal upstream IDs; Default and Dark remain independent designs. Mermaid's `null` sentinel is
not a catalog theme.

### Preserve bounded diagnostics and actual evidence

Encoded bytes, authored graph structure, unique resolved values, materialized output, and compiled
requirements retain their own resource checks. Admission includes inactive appearances before
selection, inspection, materialization, or grouped export succeeds.

Authoring diagnostics identify bounded structured reasons and the actual shared or local JSON
Pointer. Document inspection distinguishes a consumer's `path` from its supplying `token_path`.
These declared bindings are not a winning-rule trace. Render diagnostics and target receipts
remain evidence from their actual operation.

The CLI's `theme inspect` is a thin complete-recipe check over the existing authoring operation.
It accepts a portable recipe file or stdin, while `render --theme-file` accepts the broader
render-selection grammar. Inspecting a theme does not require a new rendering operation.

## Consequences

One authored value can be copied, customized, saved, and shared across hosts. The renderer keeps a
small concrete execution boundary; hosts retain responsibility for preferences, stale results,
frozen exports, and application chrome. The alpha change requires coordinated consumer updates,
but avoids a second version dispatcher or host-side theme engine.

Rejected alternatives include preset-plus-patch selection, arbitrary recursive JSON composition,
runtime OS appearance inside the compiler, CSS selectors or executable token expressions, and
binding-local materialization. Each would add another interpretation or ownership boundary without
helping ordinary theme exchange.

The [theme guide](../rendering/custom-diagram-themes.md) owns user examples, and
[Options JSON](../bindings/OPTIONS_JSON.md#diagram-theme) owns the render-selection grammar.
Those guides and executable contract types take precedence over obsolete historical proposals.
