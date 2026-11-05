# Mermaid Compatibility and Evidence

This directory contains maintained compatibility guidance and policy. Start with
[current status](STATUS.md), then use the guide for the boundary under investigation. Exact
capabilities, fixture admission, baseline provenance, and comparison facts belong to structured
owners. When prose disagrees with an owner, correct the prose.

## Guides and Policies

| Document | Purpose |
| --- | --- |
| [Status](STATUS.md) | Selected baseline, support boundaries, and verification entry points. |
| [Admission model](ADMISSION_INVENTORY.md) | How capabilities, compare policy, and evidence establish admission. |
| [Family notes](FAMILY_NOTES.md) | Family-specific caveats with implementation and test owners. |
| [Parity policy](PARITY_POLICY.md) | Required semantic/structural convergence and bounded browser residuals. |
| [Semantic edge labels](SEMANTIC_LABEL_PARITY.md) | Identity, geometry, presentation, and exact residual contracts. |
| [Config and frontmatter](CONFIG_FRONTMATTER_SUPPORT.md) | Accepted, merged, consumed, and rendered configuration behavior. |
| [CLI compatibility](CLI_COMPATIBILITY.md) | Versioned `mmdc` command-line compatibility and deliberate divergences. |
| [External sources and licensing](EXTERNAL_FIXTURES.md) | Fixture provenance, comparison lanes, design references, and attribution. |

Completed plans, review rounds, import lists, and test-count reports are not maintained here.
Family syntax and fixture inventories live with their implementation, tests, and manifests;
ordinary verification does not require matching Markdown document pairs.

The [release-facts collector](../../scripts/collect_release_facts.py) has a legacy fallback that
parses the [37-family status table](STATUS.md#primary-svg-matrix). Its section headings and
first-column family IDs remain a compatibility surface for that consumer; structured compare facts
still own admission.

## Machine Authorities

| Concern | Owner |
| --- | --- |
| Selected Mermaid and companion sources | [Source lock](../../tools/upstreams/REPOS.lock.json), [reference bundle](../../tools/upstreams/MERMAID_REFERENCE_BUNDLE.json), and [selection receipt](../../tools/upstreams/MERMAID_SELECTION_DECISION.json) |
| Canonical family and parser/render capabilities | [`family.rs`](../../crates/merman-core/src/family.rs) |
| Primary SVG matrix, commands, default DOM modes, and exact exclusions | [Compare facts](../../crates/xtask/src/cmd/compare/diagrams.rs) and their family adapters |
| Admission consistency | [`admission.rs`](../../crates/xtask/src/cmd/admission.rs) |
| Semantic/layout input evidence | Family directories under [fixtures](../../fixtures/) and their goldens |
| Upstream SVG provenance and per-input exclusions | Per-family `_baseline-manifest.json` files under [upstream SVGs](../../fixtures/upstream-svgs/) |
| Theme override oracle | [`theme_variables_oracle.json`](../../fixtures/_verification/theme_variables_oracle.json) |
| Semantic-label residuals | [Signed label geometry](../../fixtures/_verification/label-geometry-residuals.json) and [comparator](../../crates/xtask/src/cmd/compare/labels.rs) |
| Browser text/layout residuals | [Signed snapshots](../../fixtures/_verification/browser-text-layout-residuals.json) and [comparator](../../crates/xtask/src/cmd/compare/browser_text_layout.rs) |
| Root viewport contracts | [Deterministic examples](../../fixtures/_verification/deterministic-root-contracts.json), [root comparator](../../crates/xtask/src/cmd/compare/root_contract.rs), [browser oracle](../../playground/tests/root-viewport-oracle.ts), and [signed paint residuals](../../fixtures/_verification/root-viewport-residuals.json) |

`cargo run -p xtask -- check-alignment` cross-checks capabilities, compare ownership, semantic/layout
goldens, fixtures, and upstream manifests. It does not parse ordinary alignment prose.

The [12.3 migration](../migrations/mermaid-12-3.md) describes current development behavior and
dependency selection. ADRs retain the scope of their original decisions. The
[fixture version policy](../../fixtures/README.md) distinguishes historical input comments from
selected runtime and SVG provenance; an old authoring version does not select current behavior.

See the [upstream baseline guide](../rendering/UPSTREAM_SVG_BASELINES.md) for regeneration,
[SVG output contract](../rendering/SVG_OUTPUT_PIPELINE.md) for consumer/export boundaries, and
[upgrade playbook](../release/MERMAID_UPGRADE_PLAYBOOK.md) for source-release admission.
Focused geometry inspection is documented by the
[Class](../rendering/CLASS_DEBUG_SVG.md), [ER](../rendering/ER_DEBUG_SVG.md),
[Flowchart](../rendering/FLOWCHART_DEBUG_SVG.md), and [State](../rendering/STATE_DEBUG_SVG.md)
debug SVG guides.
