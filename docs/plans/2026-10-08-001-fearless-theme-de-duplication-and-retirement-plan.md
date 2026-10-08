---
title: Fearless Refactor of Theme Retirements, Capability Declarations, and Verification Layers
type: refactor
date: 2026-10-08
artifact_contract: ce-unified-plan/v1
product_contract_source: docs/plans/2026-10-06-001-refactor-theme-rendering-ownership-and-performance-plan.md
execution: code
---

# Fearless Refactor of Theme Retirements, Capability Declarations, and Verification Layers

## Goal capsule

**Objective:** Reduce theme architecture and pull-request size by retiring completed legacy migration machinery, consolidating repeated theme-domain declarations, and separating runtime behavior from acceptance-only proof, while preserving every public rendering, theme, error, cancellation, resource, portability, and consumer contract.

**Observed cost:** The current branch adds 450,380 lines against `origin/main`. The largest sources are `merman-render`, theme acceptance manifests, test matrices, and raw performance evidence. The review found no reason to weaken strict validation; the main risks are historical retirement code, duplicated domain declarations, and proof layers that know too much about one another.

**Principle:** Delete only code whose production or acceptance owner is demonstrably gone. Replace repeated declarations with one shared domain model only when each projection keeps its own contract. Keep semantic validation, native receipt validation, provenance, and public capability behavior intact.

**Execution:** Work serially on `refactor/theme-ownership-performance` in focused commits. Each phase must leave the repository buildable and independently revertible. Do not reset, restore, or remove unrelated user files. Do not introduce a process-global cache, family allowlist, validation bypass, or generated-code pipeline without evidence and an explicit owner.

## Current architecture and boundaries

The refactor starts from four distinct responsibilities:

1. **Runtime theme compilation and family routing**
   - `crates/merman-render/src/diagram_theme/compiler.rs`
   - `crates/merman-render/src/diagram_theme/family_program.rs`
   - `crates/merman-render/src/diagram_theme/family_mechanism_matrix.rs`

2. **Public support discovery**
   - `crates/merman-render/src/diagram_theme/support.rs`
   - `crates/merman-render/src/diagram_theme/support_manifest.rs`

3. **Historical route retirement and compatibility proof**
   - `crates/merman-render/src/diagram_theme/legacy_family_theme_bridge.rs`
   - `crates/merman-render/src/diagram_theme/legacy_projection_retirement.rs`
   - `crates/merman-render/src/theme_route_cutover.rs`
   - `crates/merman-theme-acceptance/src/cutover_manifest.rs`

4. **Performance and acceptance evidence**
   - `crates/merman-theme-acceptance/`
   - `docs/performance/evidence/`
   - diagram-family SVG and native export tests.

These responsibilities are related but are not interchangeable. Runtime classification must not depend on acceptance manifests. Public support discovery must not claim that a particular document rendered successfully. Retirement proof must not become a new runtime compatibility layer.

## Non-goals

- Do not rewrite the theme compiler or family renderers solely to reduce file length.
- Do not merge family-specific consumers into a generic renderer.
- Do not remove unknown-input validation, strict native filter receipts, final SVG validation, cancellation checks, resource limits, or provenance.
- Do not change preset appearance, Mermaid look/layout behavior, SVG DOM shape, PNG/PDF admission, or public bindings.
- Do not make raw benchmark JSON the source of a performance decision without the repository's adjacent-revision and A/A calibration workflow.
- Do not introduce a macro or generator before the shared domain model has two concrete consumers and a checked drift contract.

## Phase 0: Baseline and retirement inventory

Create a machine-readable inventory before deleting or moving anything.

### Work

- Record normal production feature closures and the `merman_internal_theme_acceptance` closure.
- For each legacy module, record whether it is compiled in production, tests, private acceptance, release preflight, or nowhere.
- Trace every call to:
  - `legacy_compatibility_route_inventory`;
  - `legacy_replacing_typed_theme_routes`;
  - `authorize_legacy_projection_retirements`;
  - `ThemeRouteCutoverProjection`;
  - `support_manifest` claims.
- Identify whether the empty legacy route inventory is an invariant that can become a final retirement snapshot.
- Record current test counts, artifact hashes, public support descriptors, and representative SVG/PNG/PDF outputs.

### Deliverables

- A short retirement map under `docs/performance/`.
- A list of acceptance and release commands that must remain green.
- A deletion candidate list with an owner and proof requirement for every candidate.

### Stop conditions

Stop if any legacy module is needed by a published runtime path, public binding, release artifact, or current compatibility behavior. Keep the module and mark it as retained compatibility code.

## Phase 1: Retire completed legacy bridge machinery

### Candidate scope

- `crates/merman-render/src/diagram_theme/legacy_family_theme_bridge.rs`
- `crates/merman-render/src/diagram_theme/legacy_projection_retirement.rs`
- legacy-only portions of `crates/merman-render/src/theme_route_cutover.rs`
- historical projection sections in `crates/merman-theme-acceptance/src/cutover_manifest.rs`
- tests that exist only to prove an already-empty route inventory.

### Design

Replace a multi-thousand-line historical bridge with a compact final-retirement contract:

- one typed snapshot of the final empty legacy route inventory;
- one regression test that fails if a legacy route is reintroduced;
- one acceptance check that verifies the snapshot against the current matrix;
- documentation explaining when the snapshot can be removed.

The snapshot must not provide runtime fallback behavior. The current typed family routes remain the only runtime owners.

### Validation

- Production builds with the supported feature recipes enumerated in Phase 0; do not imply testing the full Cargo feature power set.
- Private theme acceptance suite and release-preflight command.
- Tests for unknown route, route reintroduction, projection drift, and error ordering.
- Compare public SVG and native output hashes for representative families.

## Phase 2: Establish one shared theme-domain model

### Candidate scope

- `crates/merman-render/src/diagram_theme/family_mechanism_matrix.rs`
- `crates/merman-render/src/diagram_theme/support_manifest.rs`
- `crates/merman-render/src/theme_route_cutover.rs`
- `crates/merman-theme-acceptance/src/cutover_manifest.rs`
- `crates/merman-theme-contract/src/` where wire identifiers are already public.

### Design

Introduce a small internal domain vocabulary for the concepts already repeated across the systems:

- `DiagramFamilyId`;
- `ThemeTarget`;
- `ThemeFacet`;
- `ThemeDisposition`;
- route/projection identity.

The shared model owns identity and validation only. It does not own public support state, runtime route behavior, or retirement policy. Each consumer derives its own projection:

```text
shared domain identities
    -> runtime mechanism routes
    -> public support claims
    -> acceptance and retirement projections
```

Reuse the existing `ThemeTarget`, `DiagramFamilyId`, and contract facet/property enums rather than inventing a parallel model. Keep independently authored support claims and historical observations when they detect correlated implementation mistakes. Start with the existing shared model and drift tests. Consider generation only after two or more projections use it without losing readable diffs.

### Validation

- Unknown and malformed wire identifiers still return `Unverified` or the existing structured error.
- Support discovery output is byte-for-byte or field-for-field unchanged for the current matrix.
- Runtime route dispositions are unchanged.
- Acceptance manifests reject additions, removals, duplicate routes, and projection-action drift.
- No public crate gains a dependency on the private acceptance crate.

## Phase 3: Separate production matrix from audit implementation

### Work

Split `family_mechanism_matrix.rs` by responsibility:

- production route classification and compilation;
- test-only exhaustive matrix enumeration;
- private acceptance and legacy audit helpers.

Use module boundaries and `cfg` ownership to make the production path obvious. Preserve the current route compiler interface until the new modules have equivalent tests.

### Success criteria

- Normal builds do not compile acceptance-only bridge and retirement code.
- Production modules no longer import acceptance-only types.
- Test and private acceptance modules can still derive the complete matrix from the production classifier.
- No family-specific allowlist is introduced as a shortcut.

## Phase 4: Reduce performance evidence noise

### Work

Keep decision-grade evidence, but separate summary from raw observations:

- retain source revision, feature recipe, tool versions, workload identity, key measurements, thresholds, and artifact hashes in a small checked-in summary;
- archive full Criterion samples and verbose observations durably before removing tracked evidence; ignored `target/` records are temporary working copies, not the sole archive;
- update the performance report to point to the artifact identity and explain reproducibility.

This explicitly refines the preceding plan's evidence-storage contract: raw evidence must remain durably accessible alongside its scorecard. Do not delete a tracked raw record until a stable archive and its digest are verified. Prefer lossless local compaction when no durable artifact owner is available. Do not build a new proof framework just to relocate JSON.

### Validation

- The performance verification script must reject missing source identity, feature closure, tool identity, or artifact hash.
- The summary must reproduce the same accept/reject decision as the full record.
- No size budget is changed solely because raw evidence was moved.

## Phase 5: Normalize verification layers

### Work

Classify existing tests into four contracts:

1. semantic route classification;
2. SVG structure and CSS/reference ownership;
3. native target admission and receipt validation;
4. public consumer and preset exchange behavior.

For each duplicated assertion, keep one owner and replace other copies with a narrow delegation or invariant check. Do not remove a test merely because it touches the same theme property; remove it only when it proves exactly the same contract under the same inputs and output boundary.

Examples:

- route tests verify disposition, not PNG pixels;
- SVG tests verify DOM structure and references, not native font containment;
- PNG/PDF tests verify final target behavior and strict rejection;
- public consumer tests verify serialization and user-visible API behavior.

### Validation

- Every removed assertion is mapped to a retained owner.
- At least one cross-layer integration test remains for each supported effect and portability path.
- Failure diagnostics remain specific enough to identify the owning layer.
- Full family and export matrices remain green.

## Phase 6: Final deletion and architecture documentation

Delete only code proven unused by Phase 0–5. Update:

- `docs/rendering/custom-diagram-themes.md`;
- relevant theme architecture ADRs;
- performance summary and runbook references;
- contributor instructions if new generated or acceptance boundaries exist.

Record retained historical contracts explicitly so future contributors do not recreate the retired bridge.

## Verification matrix

Run serially, reusing the existing target directory:

```text
cargo fmt --all -- --check
cargo nextest run --locked -p merman-render --all-features --lib
cargo nextest run --locked -p merman --features all-diagrams,svg,png,pdf,layout-cytoscape,layout-elk,math
cargo nextest run --locked -p merman-export --features png,pdf --lib
python3 scripts/run_theme_acceptance.py nextest run --locked -p merman-theme-acceptance -p merman --features merman/all-diagrams,merman/layout-elk,merman/math --lib -E 'package(merman-theme-acceptance)'
cargo clippy --locked -p merman-cli --all-targets --all-features -- -D warnings
cargo deny check advisories bans licenses sources
npm --prefix playground/tests run typecheck
npm --prefix playground/tests exec -- playwright test theme-cyberpunk.spec.ts --project=chromium-desktop
```

For each deletion or consolidation, also run the smallest affected test first, then the complete owner suite. Cargo commands must not run concurrently with another Cargo or benchmark lane.

## Stop conditions and rollback boundaries

Stop the phase and keep the previous structure if:

- a public support result changes; any behavior change needs a separate task and is outside this refactor;
- a route or target loses provenance;
- a strict native receipt becomes permissive;
- error, cancellation, or resource-limit precedence changes;
- a private acceptance check becomes a production dependency;
- generated output becomes the only source of truth without a readable source declaration;
- a refactor requires a family-specific bypass or a broad test skip;
- a performance claim cannot be reproduced with equal feature closure and adjacent revisions.

Every phase lands as a focused commit. Do not combine legacy deletion, domain-model changes, evidence relocation, and test restructuring in one commit.

## Definition of done

- No completed legacy bridge remains in normal production or acceptance paths unless its retention has a documented compatibility owner.
- Runtime route classification and public support discovery share validated domain identities without sharing the wrong contract.
- Acceptance projections are generated or derived from a stable source and have drift tests.
- Performance evidence is reproducible without committing large raw sample dumps to the main code review.
- Each semantic, SVG, native, and public-consumer assertion has one clear owner.
- Existing public rendering and theme behavior is unchanged across representative family, layout, output, font, and preset matrices.
- Full CI, security, formatting, and browser gates pass.
- The final diff contains only intentional runtime code, tests, concise evidence summaries, and documentation.
