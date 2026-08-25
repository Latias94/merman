# Resvg-Safe Fallback Typography Audit

Date: 2026-08-23

This ledger records the issue #89 review and the adjacent fallback-owner audit. It is an
adapter-level audit: Mermaid parity SVG, the pinned Mermaid source, and the `repo-ref/mermaid` and
`repo-ref/zed` checkouts are not modified.

## Contract

`SvgPipeline::resvg_safe()` resolves supported typography against the original SVG and XHTML
source context before removing `foreignObject`. The same computed font size, family, weight,
style, fill, and line height drive measurement, wrapping, placement, and generated SVG text.
Generated fallback markers and source-class hooks remain available to hosts.

The resolver is intentionally bounded. It admits the selector and value forms evidenced by the
pinned Mermaid output and host contracts; it does not attempt browser-complete CSS, rich-text run
layout, font shaping, or post-fallback metric recomputation.

## Owner and fixture coverage

There are 13 fallback owners and 14 fixture directories because Flowchart also owns `swimlane`.
The representative test is `fallback_owner_representatives_render_typed_resvg_safe`; the ignored
exhaustive test is `all_fallback_owner_fixtures_render_typed_resvg_safe_audit` and can be filtered
with `MERMAN_RESVG_SAFE_AUDIT_FAMILY`.

| Owner | Fixture directories | Representative evidence | Exhaustive result |
| --- | --- | --- | --- |
| Architecture | `architecture` | owner manifest + typed resvg-safe render | 185 rendered / 0 skipped |
| Block | `block` | owner manifest + typed resvg-safe render | 119 rendered / 0 skipped |
| Class | `class` | public 16px regression and owner render | 251 rendered / 0 skipped |
| ER | `er` | public entity 16px / relationship 14px regression | 101 rendered / 0 skipped |
| Event Modeling | `eventmodeling` | owner manifest + typed resvg-safe render | 10 rendered / 0 skipped |
| Flowchart | `flowchart`, `swimlane` (recursive) | owner manifest + Zed-like fallback contract | 1190 rendered / 1 parser-only skipped |
| Journey | `journey` | owner manifest + typed resvg-safe render | 26 rendered / 0 skipped |
| Kanban | `kanban` | owner manifest + typed resvg-safe render | 87 rendered / 0 skipped |
| Mindmap | `mindmap` | owner manifest + typed resvg-safe render | 114 rendered / 0 skipped |
| Requirement | `requirement` | owner manifest + typed resvg-safe render | 47 rendered / 0 skipped |
| Sequence | `sequence` | Typst/public transport and owner render | 321 rendered / 1 parser-only skipped |
| State | `state` | owner manifest + typed resvg-safe render | 286 rendered / 0 skipped |
| Venn | `venn` | public presentation-inheritance regression | 12 rendered / 0 skipped |

The two skipped fixtures are explicit parser-boundary facts, not fallback-style exemptions:

- `fixtures/flowchart/upstream_flow_text_ellipse_vertex_parser_only_spec.mmd` is classified by the
  fixture catalog because pinned Mermaid 11.16 cannot render that parser-only ellipse case.
- `fixtures/sequence/stress_end_keyword_016.mmd` uses parenthesized `end` participant syntax that
  the pinned Merman sequence lexer rejects before SVG fallback. This is an audit-local sequence
  parser boundary, deliberately not added to the shared provenance catalog so this typography fix
  does not require a baseline-manifest refresh; changing fallback typography would not correct it.

The Flowchart audit also caught a test-harness defect: the fixture
`stress_flowchart_nbsp_source_provenance_079.mmd` contains the visible label `Infinity`. The audit
now scans visual XML attributes and stylesheet values for non-finite tokens instead of rejecting
ordinary text content, so valid labels are not mistaken for invalid geometry.

## Semantic evidence

### Issue #89 comment adjudication

The August 23, 2026 issue comment correctly records that the pinned Mermaid layout golden measures
ClassDiagram rows at the 16px theme size while the SVG stylesheet paints a 10px rule. That is
intentional Mermaid behavior and is not a request to change Mermaid parity. In the captured
ClassDiagram SVG, `classLabel` and `classGroup` occur in the stylesheet but not in the rendered
element ancestry, so `.classLabel .label` and `g.classGroup text` do not match those XHTML labels
under ordinary CSS selector semantics. The former fallback index extracted class tokens from those
selectors and applied their declarations to any `.label`, which was the Merman adapter defect.
This change is therefore a `resvg-safe` source-context adapter: it prevents the false selector
match while preserving the source metric used for measurement. It does not change Mermaid layout,
parity SVG, or the pinned baseline.

The focused semantic suite covers:

- absent and present contextual ancestry (`.classLabel .label`),
- SVG element selectors not matching XHTML text leaves,
- ER contextual relationship selectors and label backgrounds,
- Venn presentation-attribute inheritance,
- attribute selectors, child/descendant matching, specificity, source order, and `!important`,
- invalid selector-list and unsupported-value fail-closed behavior,
- `rem` root sizing, `em`/percentage parent sizing, unitless line-height inheritance,
- nested XHTML styles and deterministic deepest-common-ancestor fallback for mixed leaves, and
- measurement/emission identity including font family, weight, and style.

Public consumer evidence is present in:

- `crates/merman/tests/resvg_safe_typography.rs`,
- `crates/merman/tests/zed_mermaid_issue_fixtures.rs`,
- `crates/merman/tests/zed_editor_contract.rs`, and
- `crates/merman-typst-plugin/src/lib.rs` plus the Typst issue fixture.

### Issue #92 disposition

Issue #92 proposes making the Typst package inject a global
`.merman-foreignobject-fallback-text { font-size: 16px !important; }` rule by default, with an
empty-string opt-out. This is a downstream readability policy, not the source-context defect in
#89. It was evaluated against the corrected fallback and is intentionally not included in this
change:

- the corrected Merman output already keeps the ClassDiagram metric at 16px before Typst or Zed
  postprocessing;
- a global 16px rule would also overwrite legitimate source-context metrics, such as the ER
  relationship 14px case, Venn presentation-derived sizes, and explicit host/theme typography;
- because the rule is injected after fallback measurement, it changes painted font size without
  recomputing wrapping or placement, so it can create a new metric/geometry mismatch; and
- changing the Typst default would be an opinionated package behavior/API decision, whereas the
  current `scoped-css` option already provides an explicit opt-in without changing parity or
  package defaults.

The recommended handling is to close #92 as superseded for the #89 adapter path, keep the Typst
package unopinionated, and document that metric-affecting CSS must be supplied before
fallback. A future Typst-only readability profile can be considered separately if it is scoped to
an explicit user choice and has per-family/non-16px regression coverage; it must not be a hidden
global override.

## Residual boundary

Metric-affecting host CSS must be supplied before the fallback stage. CSS injected after generated
`<text>` exists can change paint in a consumer, but it cannot cause Merman to remeasure wrapping or
recompute placement. Zed's current global fallback `font-size: 16px !important` rule remains a
downstream limitation and is intentionally not changed in this issue; the Merman output is
validated before that rule is injected, while a post-return host composition is a separate
paint-only contract.
