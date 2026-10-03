# ADR-0092: Mermaid 12.1 Upstream Transition

## Status

Proposed for unreleased Merman `0.8.0`; validation and reference-artifact admission are in
progress. This ADR does not approve publication or supersede the admitted alpha.7 evidence until
the release gates pass.

## Date

2026-10-02

## Context

Merman `0.8.0-alpha.7` admitted Mermaid 12.0.0 as its behavioral and structural baseline through
ADR-0090. The development line now evaluates Mermaid 12.1.0 while keeping the prior corpus as the
comparison baseline until the new reference graph is refreshed and admitted. The Mermaid package,
parser package, and reference CLI have independent release roles and must not be described as one
version.

## Decision

- Select Mermaid `12.1.0` at commit
  `21f72f07ea22c0af48a3149c550654e80d8e40cb` for the unreleased compatibility target.
- Select `@mermaid-js/parser@2.0.1` as the parser companion.
- Select `@mermaid-js/mermaid-cli@12.0.0` as the reference comparison host. This is the CLI used
  to execute the selected Mermaid package for reference artifacts; it is not the Merman CLI
  version and does not imply that Mermaid and the CLI share a release number.
- Keep other selected companions unchanged unless a separate source-backed decision admits a
  change. Keep the independent Tree-sitter baseline under its own owner.
- Preserve Mermaid 12.0.0 as comparison and historical alpha.7 evidence until the 12.1 reference
  bundle, generated projections, legal inventory, and release gates are refreshed together.
- Add no diagram family, Cargo feature, FFI version, or editor-facts schema version as part of this
  source transition.

## Consequences

The current README, release playbook, package-surface guide, and upgrade guide may describe the
unreleased 12.1 target. Family alignment records and dated audit/progress documents may retain 12.0
when they identify admitted evidence or historical scope. A document must not call the 12.0 graph
the active target without that historical qualifier.

The reference bundle and generated artifacts remain the authorities for exact package integrity and
publication admission. Once the 12.1 gates pass, a later status update may promote this proposal to
accepted and supersede the alpha.7 active-baseline wording in release-facing indexes.

## Parser diagnostic residuals

Mermaid 12.1 renders the parser's diagnostic text in its error SVG. Merman preserves its native
Rust diagnostic while following the upstream error renderer's wrapping, truncation, escaping,
and viewport rules. Reproducing Jison or Langium/Chevrotain parser-state-specific prose is outside this transition;
invalid inputs must still be rejected by both parsers.

The two registered invalid state-diagram fixtures and the invalid Treemap fixture
`upstream_treemap_classdef_and_css_compiled_styles_db` in
`fixtures/_verification/parser-diagnostic-residuals.json` are reviewed implementation differences,
not DOM parity. The Treemap fixture is rejected by both parsers; its SVG difference comes from
implementation-specific diagnostic text and the resulting error-label geometry, not a Treemap
layout or theme difference. Their receipts bind the Mermaid version and source commit, complete
input bytes, and complete upstream/local SVG bytes, including diagnostic text and resulting geometry. The
receipts are consumed only by verification and never by production rendering.

The catalog also includes seven invalid documentation fixtures: Packet's
`upstream_docs_packet_bits_syntax_v11_7_0_002` and `upstream_docs_packet_syntax_001`, and Radar's
`upstream_docs_radar_axis_007`, `upstream_docs_radar_curve_008`,
`upstream_docs_radar_examples_005`, `upstream_docs_radar_options_009`, and
`upstream_docs_radar_title_006`. Unsubstituted `start`/`start-end` or `...` placeholders, and the
trailing axis-list comma in the Radar example, are rejected by both parsers, as their existing
error goldens require. Review confirmed matching error icons,
titles, and version nodes; only native versus Langium/Chevrotain diagnostic text and its resulting
geometry differ. These ten exact receipts across four fixture families do not extend valid
Packet or Radar syntax.

Only the existing non-strict comparison modes may accept those exact receipts. Strict mode, SVG
parse errors, root-contract failures, changed receipt bytes, and previously unreviewed fixtures
remain blocking. A receipt also fails if its DOM mismatch disappears. Reports count accepted
parser-diagnostic comparisons separately; a successful command means all blocking checks passed,
not that accepted diagnostic differences became matches.
