# ADR 0095: General Theme Authoring and Retained Bindings

- Status: accepted
- Date: 2026-10-19
- Updated: 2026-11-07 for optional paired appearances in the current alpha contract
- Supersedes: ADR 0082's compact-only authoring vocabulary, prohibition of authoring references,
  and direction toward preset-specific creation parameters

## Context

The former high-level definition could not represent complete preset appearances while retaining
shared values. Exporting a complete snapshot preserves appearance but loses the relationships
needed for ordinary restyling. Extra Playground controls cannot restore information absent from
the document. The maintainer authorized a general authoring model and coordinated pre-release
serialization changes on 2026-10-19.

## Decision

Use one self-contained `ThemeRecipeV1` design document with optional typed named values, explicit
references, semantic default styles, ordered scoped rules, ordinal palettes, canvas, and effects.
Direct values remain valid everywhere a shared value can be consumed. Presets create ordinary
design documents; the editor never branches on a preset name. Presentation metadata is optional
and does not affect rendering. One document owns both ordinary and advanced authoring.

Keep a complete-specification exchange variant for callers that intentionally distribute concrete
snapshots. Do not infer references from literal equality or look up preset inheritance on import.
The coordinated alpha refactor updates the existing version-1 types and their consumers directly;
it does not add a second recipe version or compatibility interpreter.

The design document is the sole editable owner; a resolved specification is an explicit output,
not a synchronized second editable copy. Named resource IDs are exact keys, including any dots,
and references retain those identities through exchange. Consumer field descriptors own accepted
types, units, finite choices, and numeric bounds. Optional labels and display groups cannot weaken
those domains or create a new drawing operation. Inspection describes declared bindings, not
winning rendered properties or qualified output support.

Rust owns bounded admission, typed reference resolution, consumer domains, inspection, and
materialization. Materialization lowers to concrete compiler inputs. Keep the existing compiler,
prepared theme reuse, family-owned painting, actual execution evidence, and resource admission.
Public raw-design and recipe materialization both admit the compiled result; only the private
lowerer stops at concrete inputs. Raw-design calls retain their own encoded-byte boundary and
payload-relative diagnostics, so adding appearances does not change which validation applies.
References are data, not CSS expressions, arbitrary JSON paths, callbacks, or custom interpreters.
Whole-value aliases share immutable resolved storage. The encoded-byte policy bounds the authored
input, unique resolved-value cache, and incrementally retained output collections independently;
final complete-spec admission still counts the exact payload. This prevents small shared documents
from allocating an amplified concrete specification before rejection. The recursion ceiling bounds
active resolution calls, while memoized aliases reuse resolved values; it is not a separate
logical alias-chain length contract. Token and reference-use ceilings bound the document graph.

Default styles lower before ordered exceptions. Existing semantic targets, family/variant/
ordinal selectors and per-facet clear semantics remain explicit. Ordinal palettes lower after
ordered rules. Omission leaves a field unspecified; explicit clear removes the earlier authored
field, and supplied lists replace rather than implicitly append. Whole-value references can be
detached into direct values; no generic recursive merge language is added. Materialize on an
authored change and reuse the prepared/compiled theme rather than traversing references for each
node or export. Mermaid compatibility variables remain literal inputs to their existing evaluator.

The shadow convenience uses pixel offsets and Gaussian standard deviation, with explicit
independent or cascade composition. Independent shadows read one original alpha and merge once;
cascade preserves the existing sequential-filter behavior. Shadow targets distinguish body and
label. Do not expose spread/inset/backdrop controls without an actual implementation.

### One design with optional appearances

A theme is a design identity, with optional Light and Dark appearances. Store the shared authoring
values in `ThemeRecipeV1.design`, the optional local layers in `design.appearances`, and the declared
default in `design.default_appearance`. If appearances are present, the collection is nonempty
and the required default names one of its entries. A one-entry collection is a classified single
appearance. With no collection or default, the design is unclassified and concrete; do not infer
its mode from its name, a paint value, or the editor's chrome.

Common reading designs should provide a deliberately designed pair. Artistic themes may provide
only one appearance; a dark application may display a light paper canvas. A pair is not automatic
inversion, and declaring it does not certify contrast or every family/output combination.

Composition is a bounded typed operation with explicit ownership:

| Local field | Composition |
| --- | --- |
| `tokens` | Replace supplied entries by exact token ID; omitted entries inherit. |
| `styles` | Apply local semantic styles after shared defaults and shared ordered rules. |
| `rules` | Apply after the local semantic styles in authored order. |
| `palettes` | Replace supplied targets; lower palettes after the combined ordered rules. |
| `canvas` | Replace the complete section when supplied. |
| `typography` | Replace the complete section when supplied. |
| `mermaid` | Merge only its declared scalar settings and variable-map entries. |
| `assets`, `requirements` | Shared-only ownership; not legal appearance-local fields. |

Omission inherits; supplied arrays and sections do not recursively merge. Existing clearable style
facets retain Clear, while transparent paint remains a value. A common consumer can reference a
token supplied by every appearance, so validation resolves the composed appearances rather than
requiring an independently compilable shared section.

Within a typography section, family styles inherit omitted properties from the shared default.
An explicit base-typography Clear removes the local property override; a role-style Clear restores
the resolved family base. Within each Mermaid compatibility layer, `dark_mode` and
`variables.darkMode` are one semantic setting: same-layer conflicts fail, and either spelling in a
local appearance overrides the shared setting.

Whole-document admission validates every composition under the same caller policy before
inspection, selected materialization, compilation, or grouped export succeeds. Invalid inactive
appearances reject the document. Graph and expansion bounds apply before amplification can allocate
an oversized result. Diagnostics retain actual shared/local paths and supplying token locations.

### Selection, host policy, and exchange

`ThemeAppearance` is a concrete Light/Dark choice outside the document. An omitted choice uses the
declared default; an explicit unavailable appearance fails. Unclassified designs and complete-spec
snapshots accept only omission. Follow app/editor belongs to the host and resolves before calling
the compiler. A host may fall back to the declared default only when its preferred appearance
is absent, never when a declared branch fails admission.

Rendering receives a whole preset or recipe selection. Options JSON omission inherits, `null`
clears, and a supplied `theme` replaces the prior selection atomically. An appearance-only object
is not a patch. Source/configuration precedence remains intact: `selected_appearance` records the
compiler's choice and does not claim that source overrides produce that visual mode.
Mermaid's literal IDs and compatibility `darkMode` are independent inputs.

Customize and ordinary exchange retain the complete authored document, including inactive
appearances, binding identities, order, and local exceptions. A selected specification is an
explicit snapshot, not a replacement for the editable pair. Snapshot exchange uses the existing
`complete_spec` kind and remains fixed and unclassified when reimported.
Plain-ID preset export returns the same version-1 recipe format: canonical paired IDs retain the
pair, and exact concrete IDs retain their declared single appearance.

Hosts show one design plus an appearance choice. They resolve Follow from their actual application
or editor appearance and display the effective selection. Auto changes do not edit the recipe,
source, frontmatter, or authoring undo history. Explicit choices remain pinned across host changes.
Render identity, cancellation, stale-result rejection, and image export include the concrete choice.
Image export and generated adoption code freeze that choice; workspace sharing retains preference.
Chrome, viewer backing, theme canvas, family backgrounds, and export matte/page paint remain separate.

The first authoring interface is a complete highlighted JSON draft with validation and explicit
Apply. Failed or stale validation preserves the active workspace; Apply cannot silently remove
an explicit appearance preference or replace later source/configuration edits. A future graphical
editor may expose shared/default and local/independent controls, but it must preserve the same
document ownership rather than maintaining a second editable materialization.

## Alternatives and consequences

Retaining the old snapshot editor alone would preserve exact local edits but leave the creation
problem unresolved. Preset-specific structs would duplicate interfaces and exclude arbitrary
community authoring data. A full CSS/DTCG execution engine would introduce selectors, expressions,
and conditions outside the renderer's domain. The selected design reuses typed diagram semantics
and borrows explicit reusable values without adopting those execution models.

The coordinated change includes contract types, materialization, preset sources, bindings, theme
file/share exchange, and the workbench. Independent shadows require real SVG/native rendering
work. No field is declared supported only because it can be serialized. The implementation must
preserve existing supported preset output or document an intentional reviewed change.

Independent light/dark theme IDs would duplicate ordinary catalog entries and let shared edits
drift. Requiring every design to provide both would constrain single-appearance art. Optional local
layers preserve one identity without forcing an unsuitable second appearance. A generic recursive
JSON merge was rejected because token entries, style facets, ordered rules, palettes, and complete
sections have different ownership.

## Acceptance

Acceptance requires the following behavior at the owning public interfaces:

- Simple themes, grids, hard/soft shadows, and cascade glow use the same supported types and
  headless materialization operation without preset-name branches. Custom named values use generic
  controls constrained by their consumer descriptors.
- Preset copies are self-contained designs that preserve supported appearance or have an explicitly
  reviewed semantic change. JSON, theme files, and share exchange retain resource IDs, bindings,
  both authored appearances, and the declared default.
- Shared edits update only explicit consumers; unrelated equal literals stay independent. Detachment
  survives exchange and later edits. ID rename rewrites references atomically and preserves
  appearance; relabeling does not change render identity.
- Independent shadows read the same original alpha and remain distinct from cascade/reset behavior
  in SVG and enabled native outputs. Body and label targets stay separate, paint bounds contain
  shadows without moving layout, and unsupported spread/inset/backdrop inputs have explicit results.
- Missing references, cycles, incompatible types/units, non-finite values, and expansion-limit
  violations report bounded authoring-path errors. Shared aliases cannot allocate an amplified
  concrete specification before admission rejects it.
- Invalid imports do not replace the current valid workspace. Valid appearance selections apply
  immediately; undo preserves independent source/config edits. Import and authoring proposals may
  retain an explicit confirmation boundary. Prepared rendering adds no authoring traversal;
  performance claims require representative measured workloads rather than acceptance of this decision.
- Explicit missing appearances fail, and an invalid inactive branch cannot be hidden by selecting
  the other branch. The default, each explicit choice, and light → dark → light reuse agree across
  Rust, CLI, and binding calls under the same policy.
- Host Follow and explicit pins have distinct lifecycle behavior. Frozen export cannot adopt a
  later host mode or stale render, while sharing carries the requested policy to the receiving host.

The [theme guide](../rendering/custom-diagram-themes.md) owns current public vocabulary and authoring
workflows. These acceptance requirements do not themselves qualify any preset, family/look/output
combination, installed binding, or release artifact.
