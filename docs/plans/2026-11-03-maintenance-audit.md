# Evidence-led maintenance audit and refactor roadmap

Created: 2026-11-03

Status: research complete; implementation proposed, not started.

Source snapshot: `0649021aa9bca54add6911b3551d4228943c0987` on local `main`.
The working tree was clean during source investigation. This document is the only
repository addition from the audit. No tests, builds, commits, publication, or
runtime measurements were performed.

## Decision

Keep the current model/source/output architecture. The investigation found no
evidence warranting another workspace-wide rewrite or merger of parser, model,
renderer, and adapter packages. It did find bounded opportunities to remove
duplicate interpretation and consolidate responsibility inside existing owners.

Address the native export cancellation gap separately from behavior-preserving
cleanup. Prefer independently reviewable changes with explicit retirement lists.
Breaking Rust source changes are an available design option when justified; none
of the first proposed slices requires an ABI or protocol break.

This is an audit and sequencing proposal, not an implementation-complete contract
for every candidate. Conditional items need their stated evidence before admission
to an implementation batch. A candidate is not authorization to delete its code.

## Method and evidence boundaries

The requested source was read in full:
[Maintaining software with the GPT-6 family](https://developers.openai.com/blog/maintaining-software-with-the-gpt-6-family),
published October 14, 2026. The relevant approach is to establish usage and deletion
evidence, retain intentional safeguards, prioritize useful changes, and use review
feedback to improve subsequent investigation. Its reported outcomes do not
establish performance expectations for Merman.

Five independent Codex agents investigated core/models, rendering/export, SDKs,
editor/browser code, and engineering tools. Two further agents challenged the
SDK and tooling proposals. The coordinator checked current dependency directions,
documentation entry points, recent implementation history, and selected findings.
Agents inherited the session model; no model comparison was performed.

The investigation consulted `CONTEXT.md`, relevant ADRs, the October 29
[simplification audit](../reviews/2026-10-29-simplification-audit.md), the October 30
[model/source implementation record](../reviews/2026-10-30-model-source-boundaries-implementation.md),
and the November 3 [release simplification](2026-11-03-release-simplify.md).
Completed work from those efforts is excluded from the findings.

- This is bounded source analysis, not exhaustive dead-code or security certification.
- No candidate has a measured runtime, memory, artifact-size, or CI-minute saving.
- Public Rust exports, dynamic callbacks, Cargo features, generated projections,
  and installed consumers are not dead merely because a textual search is empty.
- Cargo capabilities are product contracts, not abandoned runtime experiments.
- GitHub API access failed; open PR overlap and recent hosted run costs remain unknown.
- The pinned Mermaid commit is `1fb7869866dd619168d840291c9c54b4f129758b`
  for `12.2.0`. The local reference checkout is `dcb694ddb58dc5ad3502e7e903cac05fd812eac3`,
  and the installed reference package under `tools/mermaid-cli` reports `11.16.1`.
  Neither was changed. Local parity execution needs the existing alignment procedure first.

## Candidate inventory

Priority describes suggested ordering, not a measured severity score. P1 denotes
an operation-contract gap; P2 denotes a useful bounded refactor; P3 denotes work
to defer until measurement or related development justifies it.

| ID | Priority | Confirmed observation | Disposition |
| --- | --- | --- | --- |
| RENDER-1 | P1 | Native filter proof traversals have no operation control | Fix cancellation observation; preserve both proofs |
| EDITOR-1 | P2 | Playground appearance helpers interpret source config independently | Converge on bounded parser/presentation evidence |
| EDITOR-2 | P2 | A preview decision repeatedly extracts the same Markdown document | Give source snapshots one owner |
| SDK-1 | P2 | WASM editor normalizes JSON, reparses it, and repeats resource projection | Share a typed binding-options kernel |
| TOOL-1 | P2 | Release surface checks duplicate policies and depend on workflow text | Migrate effective assertions, then retire the checker |
| RENDER-2 | P2 | Class repeats the shared ordinary marker-offset arithmetic | Consolidate while retaining buffer reuse |
| CORE-1 | P2, conditional | Some built-in models are sanitized again by the parse pipeline | Characterize security semantics before removing repetition |
| DOC-1 | P2, small | Current entry points name removed APIs, files, and feature recipes | Correct maintained pointers; retain historical records |
| CORE-2 | P3 | Compatibility warning JSON is projected before source remapping, then replaced | Revisit with compatibility projection work |
| SDK-2 | P3 | Node construction uses two different bounded object snapshot implementations | Defer; safety budgets and error behavior differ |
| TOOL-2 | P3 | A Typst job invokes two build/optimization production paths | Measure phases before consolidating production |

## Proposed changes and acceptance evidence

### RENDER-1: carry operation control through native filter proofs

Evidence: `crates/merman-export/src/native_filter_receipt.rs:54` accepts SVG,
the resolved usvg tree, and a viewport, but no `OperationControl`.
`parse_raw_drop_shadows` at line 66 scans XML; `resolved_tree_matches` at line 789
and `visit_group` at line 1099 traverse the resolved representation without
cooperative checkpoints. Raster and PDF preparation call it at
`crates/merman-export/src/lib.rs:1931` and `:2030`.

Outer checkpoints still observe cancellation. This does not prove successful
output after cancellation, unbounded input, or a stack-overflow vulnerability.
The narrower issue is delayed observation during Merman-owned traversal.

Keep ownership in `native_filter_receipt`. Pass the existing control into both
proofs and propagate cancellation through an error-bearing result. `None` must
continue to mean that the filter cannot be certified, not cancellation. Migrate
both callers and delete the old uncontrolled entry point.

Retain original-XML and resolved-tree checks, duplicate-reference rejection,
shadow composition, color-space, glyph/clip/mask evidence, seals, and sticky
terminal/error behavior. This is an intentional cancellation behavior correction,
not a claim of complete behavioral equivalence or general export speedup.

Verification: extend tests in `native_filter_receipt.rs` and export `lib.rs`
for deterministic cancellation during raw scanning and resolved-tree traversal,
plus prior resource-terminal versus cancellation ordering. Use a bounded test seam,
not sleeps or host-speed assumptions. Keep existing filter and conversion tests.

### EDITOR-1: retire the Playground's second source-config interpreter

Evidence: `playground/src/lib/mermaid-appearance.ts:43` layers frontmatter,
workspace config, and directives. At line 69, a regex extracts directive arguments;
line 79 replaces quotes and calls `JSON.parse`. `mermaid-canvas-tone.ts:24`
replays theme precedence and the `"null"` sentinel. Consumers include
`components/theme-panel/ThemePanel.tsx:62` and `components/Preview.tsx:201`.

The helper explicitly describes declarations, not computed styling. Nevertheless,
canvas tone also consumes it. The valid JSON5-shaped directive
`%%{init: {theme: 'dark'}}%%` illustrates a static syntax mismatch: replacing quotes
does not quote the key, so the local JSON parser rejects it. Browser-visible impact
has not been reproduced in this audit.

Core must remain the source-config interpreter. Derive a bounded authored-appearance
summary while existing analysis evidence is available, preserving origin and
availability without retaining the full config tree. Obtain effective canvas tone
from the matching presentation operation's evidence. Do not evaluate visual themes
in semantic-only analysis merely to drive this UI.

Migrate both consumers, then remove source YAML/directive interpretation and engine
precedence replay from these helpers. Preserve the frontmatter locator used for
lossless insertion, workspace-owned JSON editing, source/site authority distinctions,
and unknown states during incomplete input. Any new worker DTO must use the existing
explicit versioning and request-generation rules; do not silently extend a closed schema.

Verification: extend `playground/src/lib/mermaid-config.test.ts`, relevant tests in
`crates/merman-analysis/src/rules/tests.rs`, and existing theme browser scenarios.
Cover JSON5 keys/comments, malformed then valid directives, broken frontmatter,
scoped/global precedence, secure filtering, incomplete input, and late results.
Prove displayed declarations and canvas behavior against parser/presentation
evidence rather than two mutually agreeing TS helpers.

### EDITOR-2: extract a document source snapshot once per decision

Evidence: `tools/vscode-extension/src/preview-session.ts:172` resolves an editor,
resolves its input, and separately lists all inputs. On the ordinary follow path,
editor suitability already calls `resolvePreviewInput` at line 212. Input
resolution at line 291 also performs extraction. `preview-source.ts:286` scans
document lines and joins fence contents, so this is repeated materialization.
CodeLens and pre-debounce event handling have additional consumers.

Preserve extension-owned extraction: preview/export must work without LSP. Give
one preview decision an editor, immutable inputs, and selected input together.
Use one document-source owner for consumers that need a shared current snapshot;
if retaining snapshots, key by document object, version, and language identity and
retain only the current version. Keep cursor selection, lock tracking, panel state,
and render request state in their existing sessions. No general cache framework.

Delete redundant scans and selection passes. Preserve locked fence hash/range
identity, version-jump invalidation, vanished-fence behavior, immediate export
invalidation, and stale render rejection.

Verification: extend `src/test/preview-session.test.ts`,
`preview-source.test.ts`, and `preview-manager.test.ts` under the extension.
Count actual extraction or `lineAt` work for one unchanged version, then cover
multiple panels, cursor movement, document replacement at the same URI, close/reopen,
language changes, changed/deleted locked fences, and disabled LSP.

### SDK-1: share typed analysis option preparation

Evidence: `crates/merman-wasm/src/editor_language.rs:758` calls
`apply_resource_ceiling_json`, decodes its returned JSON, interprets normalized
resource limits at line 835, and prepares analysis fields at line 954.
`crates/merman-bindings-core/src/common.rs:2818` already serializes and validates
the intermediate JSON. Related analysis selection and resource projection exist
at lines 2160 and 2420.

Independent review accepted the ownership issue but identified non-equivalence:
host-ceiling admission and effective analysis-limit projection are distinct jobs;
the existing paths also filter non-analysis resource fields differently.

Bindings-core should own shared envelope decoding, explicit host-ceiling admission,
and typed analysis projection. WASM retains selection of the editor's interactive
ceiling and `DocumentAnalysisContext`; analysis retains config semantics. Add a
narrow typed result over the same kernel. Keep the JSON output used by Typst, sharing
that kernel rather than retaining two option interpreters.

Retire the WASM-local normalized resource readers and field/profile stripping
only after their consumers migrate. Preserve Options schema 3, wrapper conflicts,
top-level parse isolation, strict ceilings, zero/absent limits, requested fixes,
theme admission, and error status/ordering.

Verification: extend the existing tests in `editor_language.rs` and `common.rs`,
plus Typst resource tests. Establish old/new path differential cases for direct
and wrapped options, default/constrained/stricter limits, rejected widening,
non-analysis fields, and interacting invalid fields. Equivalence must be proved,
not assumed from matching type signatures.

### TOOL-1: retire release surface substring proofs after moving policy

Evidence: `scripts/release_surface_contract.py:36` declares another release-surface
catalog, and line 252 checks workflow markers by substring. Its test at
`scripts/test_release_surface_contract.py:86` requires an upload step rename to
fail. `scripts/test_release_workflow_security.py:995` checks the checker's source
text. Preflight and crate release actively invoke the script; it is not dead code.

Independent review confirmed unique policies that cannot be discarded:
JNI must explicitly remain `publish = false` at line 212, four public source
packages must remain in the publish graph at line 260, and profile/package
relationships must remain correct. `tools/publish.py` currently filters declarations;
that alone does not enforce the fixed JNI policy.

Move those policies into existing publish-graph tests, artifact recipe tests,
Android build checks, and SDK archive qualification. Keep required-owner/preflight
evidence and credential isolation. Then delete the surface script, dedicated tests,
two workflow steps, obsolete classifier entries, and source-string assertions
including `test_release_workflow_security.py:928` and `:995`. Update operator links.
Do not introduce another surface manifest, YAML interpreter, or release state machine.

Verification: use `scripts/test_publish.py`, `test_artifact_profile_recipe.py`,
`test_build_android.py`, `test_sdk_archive.py`, `test_release_preflight_evidence.py`,
`test_release_workflow_security.py`, and `test_ci_plan.py`. Mutation cases must
show that a benign step rename passes, while JNI publication, a missing source
package, a missing/wrong profile package, a failed required owner, and skipped
qualification still fail. Run the pinned actionlint and high-severity zizmor checks
for changed workflows. No CI-minute reduction is claimed.

### RENDER-2: give ordinary marker-offset arithmetic one implementation

Evidence: `crates/merman-render/src/svg/parity/class/edge.rs:68` duplicates
the ordinary offset arithmetic in `svg/parity/edge_path.rs:61`. The Class marker
set uses offsets already present in the shared implementation.

Put reusable-buffer arithmetic in existing `edge_path`, let its returning-`Vec`
convenience function delegate, and migrate Class with its relationship-to-marker
mapping. Delete the duplicate Class arithmetic and table. Preserve Class buffer
reuse; a direct switch to a newly allocated vector on every edge is not acceptable.

Keep floating-point operation order, short stubs, zero/one/coincident points,
both-end markers, SVG attributes, and current work accounting. Rounded/ELK,
line-hop, and State-specific policies remain distinct.

Verification: fixed independent numeric examples first, then existing
`class_svg_test`, `flowchart_svg_test`, `state_svg_test`,
`elk_cross_family_semantics_test`, and `edge_geometry_completeness` targets.
Run the current DOM/geometry/root comparison owners for affected families in an
aligned reference environment; do not broaden tolerances or update baselines to
make a supposedly equivalent refactor pass.

### CORE-1: characterize repeated sanitization before selecting one owner

Evidence: Railroad calls `sanitize_common_db_fields` during construction at
`crates/merman-core/src/diagrams/railroad.rs:375`, then the typed parse pipeline
dispatches sanitization again at `presentation/parse_pipeline.rs:690` through
`presentation/diagram.rs`. Railroad's sanitizer at line 129 traverses rules and
the AST, not just common titles. Cynefin and Wardley also repeat common-field work.

The repeated work is confirmed, but safe removal is not. HTML/entity sanitization
must not be assumed idempotent. Start with Railroad, compare the pinned upstream
construction and malformed-input behavior, then select family construction as
the single built-in source-safety owner if the evidence supports it. Keep custom
JSON pipeline admission and shared markup algorithms; do not move source security
into `merman-model` or delete the common stage for every family.

Verification: extend Railroad tests and
`crates/merman-core/tests/parse_projection_contract.rs` with strict/loose security,
malformed HTML, entities, nested AST text, titles, and accessibility fields.
Use independent expected results, not agreement between two paths with the same
new sanitizer. If correct upstream behavior changes current output, classify and
review that as a semantic fix rather than an equivalent optimization.

### DOC-1: repair maintained context pointers

`CONTEXT.md:39` names removed `crates/merman-core/src/baseline.rs`; the generated
owner is `crates/merman-model/src/baseline.rs`. Its parser description at line 55
still names `Engine`; current exports are `SemanticParser` and `MermaidEngine`
at `crates/merman-core/src/lib.rs:137` and `:144`. The capability overview omits
the newly independent model/markup owners.

`CONTEXT.md:140` points contributors to the old fearless-refactor gate document,
whose feature examples still use removed `render`/`raster` features. The current
facade declares `svg`, individual encoders, and selectable families.
`docs/release/MERMAID_UPGRADE_PLAYBOOK.md:115` also claims the removed core baseline
module re-exports the generated constants.

Correct these maintained entry points and direct commands to their existing owners
in `docs/development/CI.md`, current manifests, and `xtask verify`. Explicitly date
or retire obsolete gate recipes. Preserve historical reports and original
measurements; do not rewrite them as current execution evidence. Review wording
and links manually rather than adding Markdown-string gates.

## Deferred candidates

| ID | Evidence | Why deferred; entry condition |
| --- | --- | --- |
| CORE-2 | `diagrams/flowchart.rs:245`, `merman-model/src/families/flowchart/compat.rs:12`, `presentation/parse_pipeline.rs:836` and `:929` construct then replace warning JSON | Cleanup affects compatibility callbacks and source-map ordering for a small projection. Revisit when touching that path; retain complete independent projectors and custom JSON adaptation. |
| SDK-2 | `platforms/node/src/engine.mjs:150` and `:1245`, `platforms/node/src/errors.mjs:204` and `:232` snapshot construction options twice | Independent review downgraded it. Structure budgets, escaped-byte budgets, shared expansion, accessors/`toJSON`, and error priorities differ. Cost is construction-only. Characterize defaults/boundaries/input immutability and measure construction before choosing a common codec design. |
| TOOL-2 | `.github/workflows/ci.yml:1043` and `:1049`; `wasm_size_matrix.rs:732`, `typst_package.rs:435`, `typst_artifact.rs:539` | The same recipe enters Cargo and optimization twice, but this does not establish two full compilations. Measure optimization/strip costs first. Any consolidation must preserve raw, stripped, gzip, and Brotli metrics, source/tool/input/final digests, and real Typst consumers. |

ASCII group-depth work was already identified in the October 29 audit. Its
observable work accounting remains unresolved; it is not a new finding here.

## Architecture to retain

- `merman-model` owns data/shared execution contracts; core owns source ingestion;
  markup owns shared safety algorithms; render/export own their representations.
  The inspected production dependency directions support this architecture.
- `ParsedDiagramRender` carries private source-construction provenance;
  `PresentationDiagram` is independently constructible checked data. Combining
  them would erase a real trust distinction.
- `PreparedSource` shares frozen data; each output owns fresh operation state and
  re-admission against its current limits. Repeated admission is intentional.
- Tree-sitter tolerant syntax and strict semantic parsing serve different callers.
  Compare and Benchmark need separate realms, queues, clocks, and lifecycle.
- SVG pixel geometry and ASCII cell geometry are different contracts.
- Original XML proof, resolved usvg proof, feature closure, generated freshness,
  and installed C/SDK/package consumers establish different guarantees.
- Native C error numbers, JNI/UniFFI projections, pointer ownership, and linear
  memory require explicit adapters. CLI's document-before-encoding stage retains
  a real scheduling/admission responsibility.

## Recommended batches

1. **Establish a current implementation baseline.** Recheck HEAD, local changes,
   overlapping work, exact relevant features, and reference/tool availability.
   Repair DOC-1 as a small independent documentation change. Do not repeat the
   October 29 simplification or the merged November 3 release work.
2. **Repair the control gap.** Implement RENDER-1 with focused cancellation and
   proof tests. It can proceed independently of behavior-preserving cleanup.
3. **Calibrate cleanup with one small equivalence slice.** Prefer RENDER-2 or
   EDITOR-2. Prove fixed numeric behavior or one extraction per document decision,
   delete the old implementation, and review the actual retirement diff.
4. **Converge duplicated interpretation.** Implement SDK-1 and TOOL-1 as separate
   changes after their characterization/policy tests. EDITOR-1 needs its narrow
   authored-versus-effective evidence design settled before changing a worker DTO.
5. **Admit conditional work separately.** CORE-1 needs source/security evidence.
   CORE-2, SDK-2, and TOOL-2 remain deferred unless their stated entry conditions
   establish worthwhile benefit. A comprehensive inventory is not a mandate to
   implement every item.

RENDER-1, RENDER-2, extension snapshots, and release policy have disjoint production
owners and can be prepared independently. SDK-1 and EDITOR-1 overlap analysis/binding
interfaces and should share one coordinator or run sequentially. Keep Cargo commands
serial, reuse `target/`, and bound build/test concurrency. Parallel code reading
does not imply parallel full builds.

## Verification and completion

For each implementation slice, start with meaningful existing owner tests and
only the missing behavioral cases described above. Use `cargo fmt` for Rust and
focused `cargo nextest` recipes selected from the actual current manifests/CI.
Do not copy the historical fearless-refactor feature examples. Widen to feature,
parity, browser, installed-consumer, or release owners only when the change reaches
their contract. A full workspace run does not replace a missing targeted oracle.

A slice is complete when its consumers use the final owner, the replaced code and
temporary adapters are removed, applicable contracts pass, and documentation describes
the resulting interface. Report behavior changes separately from equivalent cleanup.
Keep failed or unavailable environment qualification explicit. Make a performance
claim only with a comparable before/after workload and recorded tool/source identity.

## Reusable investigation prompts

The prompts below capture the audited method with corrected scope and evidence
requirements. They are reusable templates, not a verbatim tool transcript. Each
actual agent also received its relevant paths, ADRs, and recent completion records.

### Discovery prompt

```text
Audit [SCOPE] in Merman at [IMMUTABLE COMMIT] for substantial unused code,
duplicate behavior, and ownership problems worth refactoring. Work read-only:
do not edit files, build, test, commit, publish, or spawn further agents.

Read the applicable repository instructions, current architecture decisions,
recent completed refactors, and actual production consumers before deciding.
Find the strongest few candidates; zero findings is acceptable.

For each candidate, provide exact file/symbol locations, entry-to-consumer
call paths, indirect/dynamic/generated/feature-selected uses, prior decisions,
the strongest reason to retain it, and what remains unverified. An empty
text search is not proof that a public or dynamically reached API is unused.

Propose the final owner, consumer migration, and exact old responsibility to
delete. Breaking development Rust source changes are available when justified;
do not preserve permanent transition layers without a real consumer contract.
Retain pinned Mermaid semantics, intentional security checks, operation controls,
resource/error behavior, and published ABI/wire contracts. Separate deliberate
behavior fixes from equivalent refactors.

Name existing verification owners and the missing behavioral oracle. Distinguish
confirmed facts from hypotheses and measured costs from unknowns. Do not rank by
line count, create a new framework merely to remove duplication, or re-report
completed work. Return priority, confidence, effort, and two apparent duplications
that should remain. Do not execute the proposed validation.
```

### Disjoint scope additions

| Lane | Scope and question |
| --- | --- |
| Core | `merman-core`, `merman-model`, `merman-markup`, Rust facade: is semantic/config/source responsibility duplicated after ADR-0099? |
| Render | `merman-render`, `merman-ascii`, `merman-export`: are identical algorithms repeated, and are control/representation boundaries complete? |
| SDK | bindings-core, FFI, UniFFI, WASM/WASI/JNI, CLI, Node wrappers: do adapters repeat interpretation rather than shape/ownership conversion? |
| Editor | analysis/editor/LSP, Playground, web runtime, VS Code: are source facts or lifecycle state independently interpreted or repeatedly materialized? |
| Tools | xtask, scripts, workflows, descriptors: does each check prove a distinct behavior, or merely mirror source spelling and already-owned facts? |

### Independent challenge prompt

```text
Independently challenge [CANDIDATE] at [COMMIT]. Read only its implementation,
actual callers, relevant contract tests, and existing alternative owner.
Do not implement or search for unrelated findings.

Decide accept, downgrade, or reject. Check whether apparently duplicate paths
have different trust, budget, error, lifecycle, representation, or consumer
contracts. Identify the smallest final ownership design and any policy that
would disappear if the proposed deletion happened today. State the exact
characterization or mutation evidence required before deletion. No measured
benefit means no performance promise. Agreement is not the objective.
```

### Implementation handoff prompt

```text
Implement only [ACCEPTED CANDIDATE] from the reviewed maintenance audit.
First verify the current source snapshot and inspect concurrent local changes.
Use the existing owner and establish the missing behavioral evidence before
removing the old responsibility. Migrate all in-scope consumers, then delete
superseded implementations and temporary transition code.

Preserve the candidate's explicit contracts. Do not create a generic framework,
change unrelated algorithms, broaden parity normalization, or alter publication.
Run focused owner checks with serial Cargo work and bounded concurrency; widen
only for affected contracts. Report the final ownership, exact retirement,
tests actually run, intentional behavior changes, and remaining limitations.
```
