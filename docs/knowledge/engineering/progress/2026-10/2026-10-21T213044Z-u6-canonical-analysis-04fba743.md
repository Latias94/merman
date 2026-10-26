---
type: Work Progress
title: Route default analysis through canonical semantic captures
description: Remove default presentation and compatibility JSON work while preserving an explicit full custom adapter.
timestamp: 2026-10-21T21:30:44Z
record_id: 04fba743f3424ea193613424de888394
producer_id: codex-root
run_id: semantic-capability-boundaries-u6
related_plan: docs/plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md
git_branch: refactor/semantic-capability-boundaries
git_commit: 76f398e51
---

# Actual Consumer Migration

Default `Analyzer::new` and `with_options` now use `SemanticParser`, including in
an ambient full-feature build. The analysis package's normal core dependency
selects only `editor-facts`. Default capture consumes the canonical family model
and directly moves its typed warning facts; it constructs neither legacy diagram
JSON nor a presentation sidecar. Existing source, warning and recovery candidates
still enter the same downstream diagnostic projection.

The explicit `compat-json` analysis feature exposes
`Analyzer::with_mermaid_engine(MermaidEngine, options)`. This closed custom backend
preserves detector/parser registries and full presentation validation. Options
remain authoritative for exact site configuration and runtime policy. Diagnostic
policy derivation keeps the environment identity; source or snapshot policy
changes establish a new identity. Operation contexts and cancellation retain their
previous ownership and outer error channel.

Remove `with_engine` and the unreachable internal `engine_from_options` helper;
there is no legacy forwarding shim. Private `AnalysisMetadata` retains only the
diagram type, authored config and effective layout actually used by analysis.
`SemanticDiagramModel::into_warning_facts` moves the four warning-bearing families'
vectors without decoding JSON or cloning a complete model. Full custom engines
are boxed so their 616-byte representation does not pad every semantic backend;
the strict lint exposed this before final acceptance.

Custom-only tests remain capability-gated. Mixed rule-corpus tests retain their
ordinary cases. Canonical source-config helpers replace old full metadata calls;
no full dev-dependency feature was added to hide those calls. Stable diagnostic
catalogs and wire versions are unchanged. The README documents the intentional
R7/R8 removal of visual-theme evaluation from default analysis.

# Proof And Configuration Repair

Before production changes, the public default source/host-theme proof failed with
`merman.config.invalid_theme_color`: nextest
`6b0588e2-d6c9-42fe-bcad-b36dd1c7e625`, one selected test failed, 45 outside the
filter. After migration, both diagnostic and rich-facts calls accept valid
Flowchart semantics and retain complete source-mapped node facts. Separate full
adapter cases still report the existing color/evaluation errors.

The first lean suite then passed 310 tests and exposed a real semantic config
omission in `parse_failures_retain_operation_effective_layout`:
`5638cd88-de1c-4e13-9acf-53ae303977c9`. Swimlane capture retained the global `elk`
default rather than its family `swimlane` default. Keep that test's original
expected result and fix the config owner.

Extract the existing pure appearance winner selection into
`config/appearance_selection.rs`; full `AppearanceDecision` reuses it unchanged.
The semantic path uses admitted source, authored site overrides and upstream
family/root defaults, then applies the existing Flowchart-ELK detector effect.
Only theme/look/layout identifiers are selected. Theme evaluation, materialization,
paint provenance and full context definitions remain presentation-only. Selected
values use the existing controlled iterative clone helper.

The expanded existing Swimlane test checks detected and known-type routes,
explicit host layout, source admission and scoped theme/look/layout. With
presentation enabled it compares all three selected keys to actual full metadata.
An initially authored null expectation was corrected only after the real full
comparison passed and existing merge/preprocessing source confirmed that
frontmatter null is ignored before selection. The selected full characterization
was `302dc46c-a16e-41da-8a51-c2f939f4096b`; raw selector null-presence behavior was
not changed. A clone-helper signature typo was corrected before runtime gates and
is not counted as semantic RED evidence.

# Verification

Cargo runs use the existing F-drive checkout and one D-drive verification target,
two build jobs, incremental disabled and two nextest threads. No cache deletion or
new worktree was needed. Logs are in `D:/merman-u5-temp/`.

| Scope | Result | Nextest run ID |
| --- | --- | --- |
| Core and analysis, all families plus editor, no presentation/compat | 822 passed, zero skipped | `c4d188d4-cbc7-44bf-b351-78bb18832b76` |
| Core and analysis, all features, lib and all integration targets | 2,606 passed, zero skipped | `331916ed-701e-43b5-85c3-168d463196d5` |
| Analysis all features after the final custom-backend boxing | 330 passed, zero skipped | `329c713e-a6b3-4813-97c1-ec7599fc51a8` |

The final representation-only boxing changes analysis; the unchanged core gates
remain covered by the full run. Production Clippy for both packages passes with
all features and `-D warnings`. Workspace formatting and diff whitespace pass.
Core defaults-off doctests pass four compilation proofs. Lean all-family analysis
doctests pass three examples and two compilation proofs.

An independent consumer outside the workspace selects only
`merman-analysis` with defaults off and `all-diagrams`, uses resolver 2 and a lock
seeded from workspace SHA256
`de1fbd7c80a19c446943d97b6dedbf1c43046914bf02e7cb31f8d2b7b269aa3e`.
Resolved registry versions were checked against that lock. Metadata confirms 34
diagram selectors and only `editor-facts` among core's presentation/editor/JSON/
test capabilities; neither analysis compat nor test-support is enabled.

The locked offline consumer builds and performs both public diagnostic and rich
captures for Flowchart, Sequence, State and invalid visual color input. All four
pass, retaining parser-backed, complete source-mapped facts. A PowerShell array
concatenation mistake initially passed a directive-only argument; the corrected
four-input invocation is the accepted runtime log. This probe is compilation and
behavior evidence, not a final stripped-byte measurement.

Local probe identities:

- Manifest: `51c29c24d06e4a67d45c16fab6091c46e128976b4d13dea1eb0add23c3af1db3`.
- Source: `82c68cc592143875573dc0895b48519fa83005d88a3ec232653172ee5acb6ce0`.
- Metadata: `b7c363a382acb1cc470db9fd6193543cf265a4daa96100bb99b4ee3fffac9f40`.
- Runtime log: `279162dad1a65a390f1f9959f3c30834944b0e5d772d0071c191a77508157b5a`.
- Debug executable: `a0b839d1e9e4823d40e1def4c9e4672f27bebf8c325bce2432bc16cdf22504e4`.

Independent correctness and test-coverage reviews reported no confirmed findings;
the shared-selector repair also received a follow-up source review. Reviewers did
not run Cargo. Narrow configurations still emit existing cfg-specific warnings;
only the stated all-feature production selection is a strict lint pass.

# Remaining Approved Work

This is a bounded U6 consumer unit, not completion of the whole goal.
DiagnosticsOnly still requests canonical editor journals before dropping rich
symbols; the finer recovery-only observation remains to be implemented and proved.
No latency, allocation-total or final-size improvement is claimed here.

Next remove editor-core/LSP's unconditional full dependency edges and migrate the
single completion test helper to canonical capture, retaining editor/LSP contracts.
ASCII needs its actual authored CSS precedence, Gantt display-end facts, XY display
policy, State/Class/Mindmap terminal views and complete bounded semantic fallback;
do not simply remove its manifest capabilities. Facade target routing and prepared
full artifacts, installed/generated/legal updates, old-name retirement and final
parity/ABI/package/comparable-size gates remain U6-U9 work. No push, PR or release
is authorized or performed.
