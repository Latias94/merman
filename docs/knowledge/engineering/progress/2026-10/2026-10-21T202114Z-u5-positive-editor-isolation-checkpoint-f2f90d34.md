---
type: Work Progress
title: Isolate editor capability definitions and preserve mixed test coverage
description: Record the implemented U5 boundary and storage-blocked final runtime gates.
timestamp: 2026-10-21T20:21:14Z
record_id: f2f90d34f52c46bcba64ae141acb0263
producer_id: codex-root
run_id: semantic-capability-boundaries-u5
related_plan: docs/plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md
git_branch: refactor/semantic-capability-boundaries
git_commit: 134872ff78791b795e9d4e09d0a1d19efd8b5f95
---

# Implemented Boundary

The current uncommitted core changes make `editor-facts` a positive compilation
boundary. The editor namespace and exports, generated rename policy, private
demand parameters, requested journals, family collectors and combined callbacks
are conditional. Full legacy snapshots require both compatibility JSON and editor
capabilities; ordinary semantic, presentation and JSON parsing remain available
without editor support. Mandatory diagnostics, source maps and validation remain.

Source-configuration provenance and YAML key/path evidence are also conditional,
including their fields, helpers and parser observation parameters. The same YAML
builder still owns values, aliases, materialization limits and strict failures.
No stub module, unit-typed journal, second grammar or JSON bridge was introduced.

Flowchart no longer retains editor-only style copies and identifier span vectors
when the capability is absent. Agentflow retains its required style target span,
authored effects and entity checkpoints. Accessibility scanning uses the existing
private editor demand: Capture retains statement observations, while Omit retains
the same title, description and masked input without inserting journal records.

Some family-owned grammar records still carry source details, and several
feature-enabled Omit parsers construct empty `EditorSemanticFacts::default()`
values. This checkpoint does not claim that all transient grammar data or empty
stack values have disappeared. It establishes no heap-total, latency or final-byte
improvement. Actual consumer migration and final cleanup remain plan work.

# Review And Corrections

The five simplification lenses completed: reuse 0 findings, quality 3 applied,
efficiency 1 applied, tests 2 applied, comments 0, skipped 0. Quality changes remove
Architecture's constant-None placeholder, restore the managed-JSON test's original
scope and share the identical directive initialization invocation while retaining
its panic/cancellation behavior. Efficiency review identified the Flowchart style
copies and span vectors; adjacent accessibility journal work was corrected too.

Test review caught an Architecture import/helper gate and mixed tests incorrectly
excluded as whole editor functions. All 85 newly gated scanned-family tests were
classified: 47 mixed tests regained their ordinary model, strict error/span,
resource, depth and cancellation assertions; 38 editor-only tests retain their
feature condition. Nine further compatibility tests were adapted locally. No
assertion, fixture, stack size, timeout or budget was removed or relaxed.

Independent shared correctness and family adversarial reviews reported no new
findings after the known fixes. Follow-up reviews checked the Flowchart/Agentflow
changes and the restored mixed tests. Reviewers were read-only; root owns edits,
formatting, Cargo and Git. These reviews do not constitute final U9 acceptance.

A formatter-specific defect was reproduced directly: rustfmt 1.95 drops `cfg`
attributes on unnamed bare-function-pointer parameters, but preserves them on
named parameters. Railroad's test callback type now names its parameters, like
the existing canonical callback type. Run feature checks after formatting; a
successful pre-format compilation did not protect this conditional signature.

# Evidence

Logs are retained in `D:/merman-u5-temp/`. Cargo uses the canonical F-drive
checkout/shared target, two build jobs and two nextest threads.

| Check | Result | Receipt |
| --- | --- | --- |
| All diagrams, compatibility JSON, no editor: lib plus projection, feature, EventModeling, Usecase and Agentflow contracts | 1,381 passed, zero skipped | `83c3270c-0cf3-420a-8ad1-22f6e3c3a573` |
| Defaults-off public namespace/provenance doctests after formatting | 4 passed | `editor-isolation-final-doc.log` |
| Defaults-off all test targets typecheck | Passed | `editor-isolation-default-tests-final.log` |
| All-feature production Clippy with `-D warnings` | Passed | `editor-isolation-full-clippy-final.log` |
| Workspace formatting and diff whitespace | Passed | `editor-isolation-final-fmt.log`, `git diff --check` |
| All diagrams, no editor, intermediate lib suite | 454 passed, zero skipped | `03ef1d65-1368-4be0-b0e4-87c58ba74775` |
| All diagrams plus editor, intermediate lean lib/public snapshot suite | 503 passed, zero skipped | `2870b13d-e834-4160-81e8-c14d8f94145f` |
| New accessibility demand proof, selected test | 1 passed, 499 intentionally filtered | `49780821-d497-4748-8d63-076d2c7c750c` |

The two public compilation proofs were observed RED before their respective
namespace/provenance boundaries, then GREEN. The accessibility demand proof first
failed with E0061 because the requested-demand parameter was absent, then passed.
Intermediate suites are not substituted for final reruns after the mixed-test
restoration and accessibility change. Narrow builds have cfg-specific warnings;
only the stated all-feature production run is a strict lint pass.

# Storage Boundary And Resume

F-drive free space reached approximately 1 GiB after the completed compatibility
suite. A safer cleanup was prepared for exactly 14 current-run files in
`target/debug/deps`: the six just-verified test executable/PDB pairs and the
matching core rlib/rmeta. Their feature fingerprints, names, sizes and modification
times (2026-10-21 20:12-20:14 UTC) were checked. The automatic approval layer
rejected that nonrecursive deletion with `blocked by policy`; no files were
removed. Explicit user confirmation for this exact scope is pending. The earlier
eight old renderer-incremental cache directories remain untouched as well.

Do not retry deletion through another API or infer approval from elapsed time.
No new worktree or target directory was created. The implementation remains
uncommitted pending final runtime verification; the existing goal stays active.

After storage clearance, finish these serial gates before the focused commit:

1. Rerun the final all-diagram lean suites with editor disabled and enabled.
2. Run all-feature core lib, `parse_projection_contract`, `deep_lifecycle`,
   `editor_semantic_facts` and `semantic_editor_snapshot`; include the other changed
   full integration contracts where storage permits.
3. Recheck formatting/strict production lint if further code changes are needed,
   then write a completion receipt and commit only the owned core changes.
4. Continue actual lean analysis/editor/LSP/ASCII consumers, naming/product and
   generated/legal updates, then U9 parity/ABI/package checks and comparable
   stripped executable measurements. Do not mark the overall goal complete.

Remote main is already integrated: merge `7c6914251de521e29f55cbb5129e0269e5dee6ca`
contains `origin/main` through `71da4cf6fbbf4fcf27fb0465cc9169f6fd6ebd8b`.
That upstream delta changes release/changelog documentation only and does not
change the approved refactor plan. Nothing was pushed or published.
