# ADR 0095: General Theme Authoring and Retained Bindings

- Status: accepted for implementation
- Date: 2026-10-19
- Supersedes: ADR 0082's compact-only authoring vocabulary, prohibition of authoring references,
  and direction toward preset-specific creation parameters
- Implementation design: [General theme authoring interface](../research/2026-10-19-general-theme-authoring-interface.md)

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
ordinal selectors, per-facet clear semantics, and palette behavior remain explicit. Whole-value
references can be detached into direct values; no generic recursive merge language is added.

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

The research design's author-facing acceptance scenarios govern implementation: shared edits and
detachment survive JSON, custom named values use generic controls, preset copies retain appearance,
independent and cascade shadows differ correctly, source/config editing remains isolated, and
reference processing is bounded and absent from reused prepared-render work.
