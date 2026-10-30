# ADR-0098: Semantic and presentation capabilities

## Status

Accepted for the development branch on 2026-10-22. This successor amends the capability and API
clauses of [ADR-0005](0005-configuration-strategy.md),
[ADR-0071](0071-editor-parser-semantic-seam.md),
[ADR-0073](0073-family-owned-diagram-architecture.md), and
[ADR-0096](0096-prepared-source-output-ownership.md). Their family ownership, parser recovery,
source mapping, and output lifecycle decisions remain in force.

[ADR-0099](0099-model-and-source-boundaries.md) selects the subsequent development package/API
ownership for independent model, terminal, graphical and diagnostic consumers. It preserves this
decision's execution boundary; its planned extraction is not yet an implementation claim.

## Context

Semantic inspection, terminal output, and language tooling need canonical meaning without
evaluating visual themes or constructing discarded presentation and JSON projections. Full
Mermaid processing still needs its configuration, theme evaluation, and paired render context.
One family-owned semantic construction supplies these requested projections.

## Decision

`SemanticParser` returns canonical models with `SemanticMetadata`. It retains admitted authored
configuration, effective semantic policy, security normalization, and family validation without
evaluating purely visual theme programs. `MermaidEngine` and `PresentationMetadata` own full
processing, including evaluated configuration and existing theme-error behavior.

Positive Cargo capabilities select the required work: `editor-facts` enables parser observation
independently of presentation; `presentation` enables full processing; `compat-json` implies
presentation for Mermaid-compatible JSON. Core's `terminal-text` leaf supplies normalization and
Unicode width helpers selected explicitly by terminal and diagnostic owners. Diagram selectors
remain independent. Cargo feature union does not change a semantic request into a full operation.

The facade routes semantic and ordinary ASCII targets through `with_semantic_parser`; graphical
and explicit presentation targets use `with_presentation_engine`. `svg` enables presentation,
while the facade compatibility-JSON API requires `compat-json`. Default analysis/editor captures
omit visual-theme diagnostics; hosts request full processing for preview validation.

`SemanticArtifact` contains canonical meaning and operation ownership, with terminal consumption
but no graphical promotion. `PresentationArtifact` retains full context and supports graphical
output. `prepare_source` and `PresentationArtifact::into_prepared_source` share the existing full
`PreparedSource` handoff. Frozen parse facts survive independently controlled outputs, each with
fresh runtime, admission, accounting, and layout. Full custom detectors and adapters remain
explicitly full-only; semantic requests never silently use them.

## Migration and consequences

The source-breaking migration removes `Engine`, `ParseMetadata`, `with_engine`, and
`prepare_semantic`; callers select the corresponding explicit parser, metadata, configuration,
and artifact APIs in the [migration guide](../migrations/render-reuse-host-integration.md).
`complete-svg` and `complete-svg-elk` remain math-free output/layout conveniences under the
2026-10-25 amendment to [ADR-0097](0097-explicit-math-capability.md); add `math` explicitly to
preserve their previous mathematical-label support.

Host transport envelopes, schemas, and capability IDs retain the synchronized baseline: Web
transport API 8, options schema 3, geometry schema 1, and native ABI 3.

This records the implemented boundary for
[plan U8](../plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md#u8-update-installed-products-generated-contracts-and-migration-docs).
It does not certify U1-U9 completion or a measured performance improvement; verification and
performance receipts remain separate evidence.
