# ADR 0095: General Theme Authoring and Retained Bindings

- Status: accepted for implementation
- Date: 2026-10-19
- Supersedes: ADR 0082's compact-only authoring vocabulary, prohibition of authoring references,
  and direction toward preset-specific creation parameters

## Context

The current high-level definition cannot represent complete preset appearances while retaining
shared values. Exporting a complete snapshot preserves appearance but loses the relationships
needed for ordinary restyling. Extra Playground controls cannot restore information absent from
the document. The maintainer authorized a general authoring model and coordinated pre-release
serialization changes on 2026-10-19.

## Decision

Use a self-contained unreleased version 1 design document with optional typed named values, explicit
references, semantic default styles, ordered scoped rules, ordinal palettes, canvas, and effects.
Direct values remain valid everywhere a shared value can be consumed. Presets create ordinary
design documents; the editor never branches on a preset name. Presentation metadata is optional
and does not affect rendering. One document owns both ordinary and advanced authoring.

Keep a complete-specification exchange variant for callers that intentionally distribute concrete
snapshots. Do not infer references from literal equality or look up preset inheritance on import.
One coordinated migration updates first-party bindings and exchange owners; a permanent legacy
interpreter is not required for this unreleased contract.

The design document is the sole editable owner; a resolved specification is an explicit output,
not a synchronized second editable copy. Named resource IDs are exact keys, including any dots,
and references retain those identities through exchange. Consumer field descriptors own accepted
types, units, finite choices, and numeric bounds. Optional labels and display groups cannot weaken
those domains or create a new drawing operation. Inspection describes declared bindings, not
winning rendered properties or qualified output support.

Rust owns bounded admission, typed reference resolution, consumer domains, inspection, and
materialization. Materialization lowers to concrete compiler inputs. Keep the existing compiler,
prepared theme reuse, family-owned painting, actual execution evidence, and resource admission.
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

The new shadow convenience uses pixel offsets and Gaussian standard deviation, with explicit
independent or cascade composition. Independent shadows read one original alpha and merge once;
cascade preserves the existing sequential-filter behavior. Shadow targets distinguish body and
label. Do not expose spread/inset/backdrop controls without an actual implementation.

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

## Acceptance

Acceptance requires the following behavior at the owning public interfaces:

- Simple themes, grids, hard/soft shadows, and cascade glow use the same supported types and
  headless materialization operation without preset-name branches. Custom named values use generic
  controls constrained by their consumer descriptors.
- Preset copies are self-contained designs that preserve supported appearance or have an explicitly
  reviewed semantic change. JSON, theme files, and share exchange retain resource IDs and bindings.
- Shared edits update only explicit consumers; unrelated equal literals stay independent. Detachment
  survives exchange and later edits. ID rename rewrites references atomically and preserves
  appearance; relabeling does not change render identity.
- Independent shadows read the same original alpha and remain distinct from cascade/reset behavior
  in SVG and enabled native outputs. Body and label targets stay separate, paint bounds contain
  shadows without moving layout, and unsupported spread/inset/backdrop inputs have explicit results.
- Missing references, cycles, incompatible types/units, non-finite values, and expansion-limit
  violations report bounded authoring-path errors. Shared aliases cannot allocate an amplified
  concrete specification before admission rejects it.
- Invalid edits retain the last valid preview. Undo/redo and Apply/Cancel preserve independent
  source/config changes. Prepared rendering adds no authoring traversal; performance claims require
  representative measured workloads rather than acceptance of this decision.

The [theme guide](../rendering/custom-diagram-themes.md) owns current public vocabulary and authoring
workflows. These acceptance requirements do not themselves qualify any preset, family/look/output
combination, installed binding, or release artifact.
