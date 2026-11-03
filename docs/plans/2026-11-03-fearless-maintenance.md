---
title: Fearless Maintenance and Responsibility Retirement - Plan
type: refactor
date: 2026-11-03
origin: docs/plans/2026-11-03-maintenance-audit.md
baseline: 0649021aa9bca54add6911b3551d4228943c0987
---

# Fearless Maintenance and Responsibility Retirement - Plan

## Goal Capsule

Remove the redundant implementations, competing interpretation, ineffective gates,
and stale current guidance identified by the maintenance audit. Give each retained
behavior one implementation owner, migrate its consumers, and verify the resulting
product rather than preserving a transition architecture.

The maintainer authorized implementation, deletion, breaking changes, environment
preparation, subagents, and focused local Conventional Commits on 2026-11-03.
This plan supersedes the audit's provisional deferrals: all eleven candidates are
in scope. Characterization and measurement determine the correct final design,
not whether to leave an examined issue unresolved.

## Problem Frame

Recent package and pipeline refactors established useful dependency boundaries,
but some consumers still interpret configuration independently, materialize the
same facts repeatedly, or validate another owner's spelling. These costs and the
uncontrolled native proof traversal are concrete local issues; they do not justify
merging the model, source, graphical, terminal, and host ownership boundaries.

## Product Contract

### Requirements

- R1. Every audit candidate has a final owner, migrated consumers, an explicit
  retirement, and applicable verification; no temporary alternate implementation
  remains merely for development-source compatibility.
- R2. Existing Mermaid syntax, source authority, output geometry and safety remain
  correct against the pinned 12.2.0 behavior; corrections to current behavior are
  documented and tested independently from equivalent cleanup.
- R3. Native filter certification cooperatively observes operation cancellation
  in its own input-dependent traversals while preserving both original and
  resolved representation checks and sticky terminal semantics.
- R4. Editor and host consumers obtain configuration meaning from its actual
  owner; declaration provenance, effective presentation, input admission, and
  current-operation limits remain distinct facts.
- R5. Publish policies and build budgets remain enforceable at their actual
  producer/consumer owners after retiring substring proofs and duplicate production.
- R6. Prepared data, source provenance, typed output pairing, independent output
  controls, terminal geometry, and host memory ownership retain their distinct contracts.
- R7. Breaking Rust/TypeScript source interfaces and internal protocols may change
  when useful. Migrate all repository consumers, remove old aliases, and update
  versioned schemas only when their actual contract changes. Do not modify already
  published artifacts or silently reinterpret established external wire versions.
- R8. Environment setup is part of implementation. Materialize pinned references
  and needed tools without overwriting unrelated checkouts or user changes; reuse
  existing build outputs and bound concurrency.

### Key Decisions

- **Complete all eleven audit items instead of leaving a conservative shortlist.**
  (session-settled: user-directed — chosen over provisional deferral: the maintainer
  explicitly requested fearless removal of all identified redundancy and issues.)
  Governs R1, R3, R4, R5.
- **Permit breaking development changes and delete superseded paths.**
  (session-settled: user-directed — chosen over compatibility scaffolding: the
  maintainer authorized breaking changes and removal.) Governs R1, R7.
- **Prepare required environments autonomously.**
  (session-settled: user-directed — chosen over environment-only deferral: the
  maintainer authorized handling the environment.) Governs R8.

## Planning Contract

### Key Technical Decisions

- KTD1. Consolidate inside existing behavior owners; add no generic parser,
  compiler analysis, cache framework, workflow resolver, or per-family interface crate.
- KTD2. Use characterization before removing security, budget, or error-path work.
  Expected results must come from independent fixed examples or pinned upstream
  behavior, not two consumers of the same newly shared implementation.
- KTD3. Keep all Cargo verification under the integration owner and serial.
  Parallel workers receive disjoint files and do not run installs, builds, global
  formatters, code generators, index writes, or commits in the shared checkout.
- KTD4. Capture execution and environment evidence in engineering-memory shards
  and commits. This plan records design rather than accumulating progress checkboxes.

## Implementation Units

### U1. Carry control through native filter certification

Goal: satisfy R3 by extending the existing export proof, then remove its uncontrolled entry.

Files: `crates/merman-export/src/native_filter_receipt.rs`, relevant callers/tests
in `crates/merman-export/src/lib.rs`.

Approach: propagate the caller's control through raw XML and resolved tree
traversals with an error-bearing result; uncertifiable input remains `None`.
Do not remove or combine the distinct representation checks.

Verification: deterministic in-traversal cancellation, existing shadow/clip/mask
proofs, conversion admission, resource terminal versus later cancellation, and
PNG/JPEG/PDF preparation. Use the owning export nextest targets.

### U2. Consolidate ordinary marker offsets

Goal: delete Class's duplicate arithmetic while preserving its output-buffer reuse.

Files: `crates/merman-render/src/svg/parity/class/edge.rs`,
`crates/merman-render/src/svg/parity/edge_path.rs`, affected render tests.

Approach: one ordinary offset implementation accepts an output buffer; existing
returning-value callers delegate. Keep rounded/ELK, line-hop, and State policy local.

Verification: independent fixed endpoints/stubs/degenerate paths and both-end
markers, then Class/Flowchart/State/cross-family geometry tests and affected SVG
structure/parity/root comparisons against the aligned reference environment.

### U3. Own extension document extraction once

Goal: replace repeated per-decision extraction with shared immutable source facts.

Files: `tools/vscode-extension/src/preview-source.ts`, `preview-session.ts`,
`preview.ts`, `preview-manager.ts`, `codelens.ts`, and their existing tests as needed.

Approach: make editor/input/source selection consume one extraction result;
retain only current document-version source snapshots where consumers share them.
Keep panel locks, selections, render requests, and LSP availability independent.

Verification: extraction work on unchanged versions, multiple panels, cursor-only
changes, document/language replacement, close/reopen, locked-fence movement/deletion,
stale output rejection, and LSP-disabled preview/export.

### U4. Retire release surface text proofs

Goal: enforce actual publication policies without the duplicate surface catalog.

Files: `scripts/release_surface_contract.py` and its tests for deletion;
`tools/publish.py`, publish/artifact/Android/SDK tests, release workflow security
tests, `scripts/ci_plan.py`, affected classifier cases, the two workflow callers,
and current operator documentation.

Approach: migrate JNI-private, source publishability, exact profile/package, and
required-owner policies to existing publish/recipe/qualification owners before
removing markers, second-order string assertions, the script, and workflow steps.

Verification: benign step rename succeeds; enabling JNI publication, removing a
required source package/profile, selecting a wrong package, failed owner, skipped
qualification, or credential leakage fails. Run focused Python and pinned workflow
syntax/security checks, then the hygiene suite.

### U5. Share typed binding analysis preparation

Goal: remove WASM's JSON round trip and independent resource interpretation.

Files: `crates/merman-bindings-core/src/common.rs`, public exports if needed,
`crates/merman-wasm/src/editor_language.rs`, relevant Typst callers/tests.

Approach: envelope parsing, explicit ceiling admission, and typed analysis
projection share a kernel. WASM owns its chosen ceiling/document context;
analysis owns config semantics. Keep a JSON projection only for real JSON consumers.

Verification: direct/wrapped options, conflicting wrappers, top-level parse,
analysis-only field filtering, zero/default/constrained/stricter limits, widening
rejection, requested fixes, theme admission, error status/order, and Typst behavior.

### U6. Retire Playground source appearance interpretation

Goal: UI declarations and effective canvas presentation consume authoritative facts.

Files: `playground/src/lib/mermaid-appearance.ts`, `mermaid-canvas-tone.ts`,
theme/preview consumers and tests; narrow existing analysis/binding/render evidence
interfaces and projections as needed after U5.

Approach: derive bounded declaration provenance from source-owned evidence and
effective theme from presentation-owned results. Delete frontend YAML/directive
parsing and precedence replay. Preserve lossless config insertion and incomplete
input states. Select explicit version migration for any changed closed transport DTO.

Verification: JSON5 keys/comments, malformed and subsequent directives, broken
frontmatter, secure filtering, scoped/global/sentinel behavior, operation generations,
and theme/canvas browser interactions. Build and smoke the actual Web/editor consumers.

### U7. Make built-in sanitization single-owned

Goal: remove overlapping family/pipeline sanitization, beginning with Railroad.

Files: Railroad/Cynefin/Wardley construction, the corresponding dispatch in
`crates/merman-core/src/presentation/diagram.rs` and `parse_pipeline.rs`, existing
family and projection tests.

Approach: characterize pinned-source HTML/entity/title/accessibility behavior,
then keep source sanitation at the successful family construction owner. Preserve
custom JSON admission and shared markup algorithms; no model-owned source policy.

Verification: strict/loose security, malformed HTML/entities, nested Railroad AST,
titles/accessibility, JSON/typed/editor projections, source coordinates and family SVG.
Document any necessary correction to previously double-sanitized output.

### U8. Project compatibility warnings after remapping

Goal: retire preprojected built-in JSON plus typed warnings and subsequent replacement.

Files: warning-bearing family callbacks/catalog, compatibility path in
`crates/merman-core/src/presentation/parse_pipeline.rs`, relevant family/model
projectors and projection/editor tests; execute after U7 to avoid pipeline contention.

Approach: keep typed family results through source mapping, project once at the
compatibility boundary, and remove obsolete callback/carrier/writeback paths.
Retain independent complete model projectors and custom-JSON adaptation.

Verification: Flowchart/GitGraph/Block warning existence/order/coordinates across
frontmatter/comments/BOM/CRLF, Agentflow's intentional absence, cancellation and
error ordering, custom callback behavior, and compatibility/editor snapshot parity.

### U9. Give Node bounded JSON snapshots one owner

Goal: remove duplicate constructor-specific clone machinery while retaining admission.

Files: `platforms/node/src/engine.mjs`, `errors.mjs`, transport/API contract tests.

Approach: characterize both current budget models and error priorities; give the
existing codec a reusable safe snapshot/encoding primitive and keep engine defaults
as product policy. Remove the independent constructor clone helpers. No unchecked
public bypass for arbitrary input and no replacement with bare `JSON.stringify`.

Verification: getters/toJSON are not invoked; input is unchanged; cycles, sparse
arrays, invalid Unicode, host values, structure/escaped-byte limits, shared expansion,
and defaults crossing limits retain defined behavior in native and WASM products.
Measure constructor work separately from ordinary rendering.

### U10. Produce Typst WASM once for budgets and packaging

Goal: retire duplicate build/optimization production while retaining distinct checks.

Files: `crates/xtask/src/cmd/typst_artifact.rs`, `typst_package.rs`,
`wasm_size_matrix.rs`, command dispatch/docs and Typst workflow callers.

Approach: the existing Typst producer owns raw/optimized/stripped bytes and
identity. Budget measurement consumes those results; smoke consumes the frozen
package. Migrate or retire the redundant internal production entry rather than
adding a generic provenance/cache system. Coordinate workflow files after U4.

Verification: record optimization-stage costs; retain raw/stripped/gzip/Brotli
definitions and current limits, exact tool/feature/source/input/output digest
checks, stale-artifact rejection, package identity, and real Typst positive and
compile-fail consumers. Compare final artifact bytes under the same recipe/tools.

### U11. Remove stale current guidance

Goal: current context and operator guides describe final APIs, owners, and gates.

Files: `CONTEXT.md`, `docs/release/MERMAID_UPGRADE_PLAYBOOK.md`, obsolete
`docs/workstreams/fearless-refactor/GATES.md` and its current-facing links,
unit-specific migration/operator documents.

Approach: identify model/markup/baseline and parser owners correctly; replace
obsolete render/raster gate recipes with current authoritative commands or retire
the redundant guide. Preserve unique historical decisions and measurements.

Verification: manual accuracy/link checks against real manifests and command
owners, existing path-length and whitespace checks. No Markdown-string CI gates.

## Verification Contract

Characterize safety- or error-sensitive paths before changing their behavior.
Missing focused cases are part of each unit, not a new blanket test framework.
Workers author disjoint changes while the integration owner runs Rust tests
serially with bounded build and test threads, using the shared `target/`.

After focused owners pass, run the affected feature/dependency/generated/legal
checks, the complete Python hygiene suite, Node/extension/Playground tests and
builds, changed browser interactions, and affected SVG DOM/root comparisons.
Exercise true final Web/Node/Typst consumers when their artifacts or protocols change.
Run integrated format, warnings-denied lint, relevant workspace tests, and independent
correctness/security/contract reviews before declaring completion.

## Definition of Done

All eleven audited responsibilities are resolved in the final implementation;
consumers use one intended owner and replaced code is deleted. Required behavior
checks pass with source/tool identities recorded, and review findings are resolved.
Unmeasured claims stay unclaimed. No pending environment check is called complete.
Focused commits preserve progress on the feature branch. The work performs no
package publication, tag mutation, or modification of released artifacts.

## Risks and Implementation-Owned Questions

Malformed HTML can make removing repeated sanitation observable; warning source
mapping and Node budget/error ordering also need characterization. Resolve these
from pinned behavior and current contracts before deletion, with explicit migration
for intended corrections. Do not preserve a duplicate implementation to avoid
settling its responsibility.

Frontend evidence may require a versioned DTO change; choose the narrowest existing
owner that carries provenance and operation generation without eager theme work.
Typst metrics must retain pre-optimization raw bytes even when production is shared.
These are implementation decisions within the authorized scope, not permission gates.
