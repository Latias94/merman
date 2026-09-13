---
type: Verification Evidence
title: Cross-crate catalog execution and C5 closure reconciliation
timestamp: 2026-09-13
related_plan: docs/plans/2026-08-09-001-portable-theme-architecture-convergence-addendum.md
git_branch: refactor/presentation-theme-model
tags: theme,catalog,c5,verification
---

# Scope and implementation

The C5 audit at `3db938c45` identified missing executed catalog coverage, rather than another
production family registry. Core already had an independent 41-variant / 33-logical-family
characterization matrix. Its cases now live in a private shared test module consumed by core's
existing parser/editor tests and the renderer's integration test. No public API, feature,
dependency, production catalog, or expected family mapping was introduced.

The renderer test compares the entire case set with the core public catalog. For every parser
variant it forces the real typed parse, checks the selected logical family, prepares a renderer
artifact, checks the artifact family, and invokes its SVG writer. Without a required layout
feature it accepts only the exact expected MissingCapability variant. With both layout features,
every variant must reach the writer and all 33 logical families must be prepared. The test is
unconditional, so a missing feature cannot silently compile away the witness.

The wire test covers every core logical family through JSON serialization, complete-spec decode,
and compilation. Rule-only and typography-only recipes are tested independently: each must equal
its typed recipe fingerprint, and fingerprints must distinguish all 33 scopes. Keeping the two
mechanisms separate prevents one surviving scope from masking the other's accidental erasure.
Parser aliases that are not logical IDs and an unknown future ID must fail with the exact
UnknownId field for both rule and typography scopes.

These tests prove catalog identity and protocol compilation, not complete theme support or
Portable admission for every family. SVG generation uses an empty theme; family-specific
portability remains the responsibility of the separate terminal and acceptance suites.

# C5 requirement reconciliation

The unified current-source owner suite passed. The table maps every C5 completion condition
to implementation and executed evidence. Final closure awaits clean-checkout confirmation.

| C5 requirement | Authority and executable evidence |
| --- | --- |
| One family ID/alias/detection authority; adapters and bindings agree | Core `define_family_catalog!`, derived registry projections, catalog/registry unit tests, shared 41-case renderer test, and binding `diagram_family_capabilities_expose_the_complete_core_catalog`. Core registry tests execute automatic detection as well as forced aliases. Wire tests cover all logical IDs independently from parser aliases. |
| Family-scoped rules and semantic styling do not enter global Mermaid config | `mermaid_compatibility::compile` consumes only explicit `spec.mermaid()`; `semantic_rules_never_enter_the_global_mermaid_config`, selected-family program tests, and legacy overlay tests check the boundary. |
| Rule ordering, static reuse, bounded ordinal and evidence work | `compiled_program_preserves_static_and_ordinal_source_order_per_property`, `compiled_provenance_shares_static_blocks_across_variants_and_ordinals`, selected ordinal candidate charging/rejection tests, compiler resource-limit tests, and shared Unsupported-domain budget/ownership tests. The previous palette-domain fix removed the remaining discovered terminal-by-rule scan. |
| Explicit Mermaid/site/frontmatter/init precedence | Core detector/config-layer tests plus primary-family site/source ownership tests execute effective configuration layering and preserve author-owned values. |
| Every applicable modeled mechanism has a classification | `FamilyThemeProgram::compile` routes specified base properties, rule facets, palettes and bindings through the matrix. All classifiers return TypedAdapter, LegacyCompatibility, or Unsupported; unsupported domains do not fall through to global styling. Matrix tests and `every_matrix_legacy_route_reaches_the_bridge_with_its_probe_value` check declared legacy projections against actual bridge output. |
| Primary families use direct consumers; default typography bridge removed | Flowchart/Swimlane share `FlowchartBaseTypographyPlan`; Sequence uses `SequenceTypographyPlan`; State has matrix-owned classification. KTD19 records six migrated FontStack/FontSize rows. Bridge retirement tests prove no old config contribution; family tests prove terminal Applied evidence and zero compatibility residuals under RequirePortable. Bridge dispatch returns None for all four primary families. |
| Legacy evidence cannot become typed Applied; deletion requires independent authorization | Shared family admission tests, C6 runtime, exact route-cutover authorization, and KTD23 retirement tests retain negative and SVG/PNG terminal witnesses. Unexpected primary Legacy routes fail closed. Remaining Block/Class routes remain explicitly legacy. |
| No-theme Mermaid output unchanged | This slice changes only test code/data and documentation. The production implementation is identical to `05ef33898`, whose full SVG structure comparison passed with the existing explicit browser-text residual policy. The prior checkpoint records its exact command and scope. |

The complete 24-case external-reference mechanism classification, provider/probe deletion and
remaining long-tail bridge migration belong to C7b. C7a catalog-to-artifact qualification,
public discovery rollout and contract freezing remain separate delivery requirements.

# Verification runs

- Initial dual-layout catalog scope: 45/45 passed before splitting the two fingerprint modes.
- Independent review identified the combined-scope blind spot and approved the corrected
  independent modes. It found no remaining source-review issue in the final tests.
- Final minimal-feature catalog suite: **46/46 passed**, including the exact missing-layout
  branches and both independent scope modes.
- Final unified private Release owner suite with both layout features: **1128/1128 passed**.
  The 19 named catalog, bounded-program, primary typography and route/retirement witnesses in
  the reconciliation table were individually confirmed as PASS in the output.
- Formatting and `git diff --check` passed. Independent test review and a separate C5
  completion-condition audit found no remaining blocker within C5's defined scope.
- Clean-checkout confirmation is pending. No production code changed in this slice; the
  no-theme structure evidence is the exact production-equivalent `05ef33898` checkpoint.

The final owner scope includes core catalog, registry and detection tests, binding catalog
projection, renderer `diagram_theme::` and `family::` tests, primary-family SVG integrations,
wire/catalog integrations, C6 runtime, route authorization and retirement checks. This is a C5
owner gate, not a full workspace, all-platform, installed-package or browser verification run.

The final commands were:

```text
CARGO_BUILD_JOBS=2 python3 scripts/run_theme_acceptance.py nextest run --release --locked -p merman-core -p merman-render -p merman-bindings-core -p merman-theme-acceptance --features merman-render/layout-elk,merman-render/layout-cytoscape --lib --test diagram_theme_test --test theme_resolution_svg_test --test typed_family_api_test --test theme_support_discovery_test --test flowchart_cluster_theme_test --test swimlane_cluster_theme_test --test state_svg_test --test sequence_svg_test --test c6_runtime --test route_cutover_runtime --test legacy_projection_retirement --test block_title_legacy_projection --test class_edge_label_background_legacy_projection --test diagram_theme_wire_test -E '(package(merman-core) & (test(tests::registry) | test(family::catalog_tests) | test(tests::detect))) | (package(merman-bindings-core) & (test(diagram_family_capabilities) | test(theme_catalog))) | (package(merman-render) & (test(family::) | test(diagram_theme::) | test(flowchart::theme_evidence::) | test(state::theme_evidence::) | test(state::style_plan::) | test(sequence::theme_evidence::) | test(sequence::typography::) | binary(diagram_theme_wire_test) | binary(diagram_theme_test) | binary(theme_resolution_svg_test) | binary(typed_family_api_test) | binary(theme_support_discovery_test) | binary(flowchart_cluster_theme_test) | binary(swimlane_cluster_theme_test) | binary(state_svg_test) | binary(sequence_svg_test))) | package(merman-theme-acceptance)' --test-threads 2 --no-fail-fast
CARGO_BUILD_JOBS=2 cargo nextest run --release --locked -p merman-core -p merman-render --no-default-features --lib --test typed_family_api_test --test diagram_theme_wire_test -E '(package(merman-core) & (test(tests::registry) | test(family::catalog_tests))) | binary(typed_family_api_test) | binary(diagram_theme_wire_test)' --test-threads 2 --no-fail-fast
cargo fmt --all --check
git diff --check
```

Raw logs are `/tmp/theme-c5-catalog-first.log`, `/tmp/theme-c5-catalog-minimal.log`, and
`/tmp/theme-c5-closure-owner.log`. `/tmp/theme-c5-closure-command.json` retains the exact argument
vector for clean-source replay. These are local test records, not artifact qualification receipts.
The production-equivalent default-output record is
[the palette-domain checkpoint](2026-09-13-unsupported-palette-domain-scan.md).
