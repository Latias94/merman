---
type: "Subagent Finding"
title: "Semantic admission function boundaries"
description: "Function-level U5 admission seams and full-operation ordering obligations."
timestamp: 2026-10-20T12:27:11Z
record_id: "a1f8d9a0730b46b4b0fe75cd8e10ede9"
producer_id: "codex-frankorz"
run_id: "semantic-capabilities-20261020"
subagent_id: "/root/u5_admission_seam"
related_plan: "docs/plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md"
git_branch: "refactor/semantic-capability-boundaries"
git_commit: "23912f078"
---

# Finding

Read-only U5 design research. Share admitted source/configuration and builtin construction inputs;
keep complete engine initialization and presentation finalization in the full owner. This finding
does not implement U5 or change its dependency on U3/U4 acceptance.

# Evidence

The proposed private borrowed family input needs `diagram_type`, `admitted_config` and `title`.
Preserve the selected diagram alias for errors and family-specific behavior. Admitted configuration
contains source/site/default values, security/CSS admission and cheap appearance selectors; the
semantic API does not promise calculated theme variables. Source configuration/evidence remains
owned by operation metadata. The direct `meta.config` reads in Architecture and Mindmap are
compatibility-only authored-layout presence checks, so those projections retain full metadata.

| Existing owner | Required split or reuse |
| --- | --- |
| `ParsePipeline::finish_preprocessed_controlled` | Preserve second-frontmatter rejection before reading cached visual errors; share source/config admission and title sanitation. |
| `OperationConfigBuilder::new` | Full retains `default_effective_config()` and its cached error. Lean starts with raw site config and reuses source filtering/merging. |
| `OperationConfigBuilder::finalize` | Share cheap appearance resolve/apply and explicit ELK layout selection. Full retains theme materialization, detector replay, paint and binding freeze/seal. |
| `SiteConfigInitialization::new` | Separate raw default/site layering from `build_default_effective_config`; preserve full pristine-install cache/error/restamping behavior. |
| `merge_site_config_override` | Reuse value behavior: legacy typography mirroring, unknown global theme normalization, explicit host secure-policy replacement. |
| `AppearanceDecision::resolve/apply_to` | Reuse scoped/root/null selection and cheap Mindmap selection. Move cheap theme ids/truthiness out of the visual program. |
| `authored_source_overrides/source_filtered_overrides` | Keep distinct public source evidence versus hardened effective policy; preserve CSS admission and recursive secure-key filtering. |
| Controlled preprocessing wrappers | Reuse the single frontmatter/directive/comment/source-map pass, with independently requested editor evidence. |

Full operation ordering has two detector views. During preprocessing, `detect_init_controlled`
uses an empty config when an initialize directive has top-level `config`, to resolve its family
configuration. Main detection later observes initialized full effective configuration with mutation
provenance. Preserve both views. Lean uses catalog builtin detectors and their necessary mutations
(currently explicit Flowchart ELK layout), without legacy registries or custom fallback.

Full order remains: cancellation; preprocessing errors; second-frontmatter rejection; cached visual
Result; filtered source merge; main detector/checkpoint; authored source filtering; appearance
selection; optional materialization; detector replay; authored appearance reapplication; explicit
ELK selection; paint and compatibility freeze/seal; title/common sanitation; family construction.

# Recommendation

Extract cheap theme identifiers/truthiness and raw site initialization first. Make builtin canonical
construction consume the small borrowed input, then split admission from full finalization and move
the distinct public APIs. Keep full callback precedence: custom render, custom semantic JSON,
builtin typed, then missing-typed/unsupported. Use the existing family catalog.

Gate visual initialization/cache errors, compiled compatibility/paint state (including its config
field), calculated theme programs/generated includes, renderer context/fields and legacy callback
definitions. Audit base reads before gating provenance storage; full same-value assignments and
PR #225 mutation restamping remain contractual. Gate editor facts independently while retaining
ordinary spans, structured diagnostics and typed warnings. Shared iterative JSON ownership remains
base infrastructure even when compatibility registries are gated. Authored style encounter order
needed by ASCII remains canonical input, even where it currently lives in render context.

AE3 skips visual program evaluation only: a CSS-admitted invalid visual color can remain source
evidence in lean parsing. Malformed YAML, secure overrides, CSS injection and family identity
sanitation retain their existing admission behavior.

# Disposition

Implementation and verification remain pending. Do not treat source inspection as compile-time
exclusion, behavior equivalence or final size evidence. The approved plan remains authoritative.

# Citations

- `docs/plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md` U5, KTD1-KTD3.
- `docs/knowledge/engineering/subagents/2026-10/semantic-boundary-seams.md`.
- `docs/knowledge/engineering/subagents/2026-10/semantic-config-reads.md`.
- `crates/merman-core/src/parse_pipeline.rs`, `lib.rs`, `preprocess/mod.rs`.
- `crates/merman-core/src/config/{mod,appearance,paint_inputs}.rs`, `diagram/mod.rs`.
