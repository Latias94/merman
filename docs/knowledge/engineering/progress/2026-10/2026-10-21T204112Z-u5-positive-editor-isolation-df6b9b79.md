---
type: Work Progress
title: Verify the positive editor compilation boundary
description: Close the U5 editor-isolation runtime gates without deleting caches or creating worktrees.
timestamp: 2026-10-21T20:41:12Z
record_id: df6b9b79b0c54ba7bb8db7864a817601
producer_id: codex-root
run_id: semantic-capability-boundaries-u5
related_plan: docs/plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md
git_branch: refactor/semantic-capability-boundaries
git_commit: 134872ff78791b795e9d4e09d0a1d19efd8b5f95
previous_checkpoint: f2f90d34f52c46bcba64ae141acb0263
---

# Boundary And Scope

The implementation described in the preceding editor-isolation checkpoint now
passes the final all-family lean configurations with editor support disabled and
enabled. Editor definitions, journals, source-config provenance and observers
compile only when selected. Mandatory source maps, validation, warning spans,
resource accounting and cancellation remain on ordinary parse paths. Full
compatibility snapshots additionally require both capabilities.

The current unit changes the core owner and its tests. It does not migrate the
actual analysis, editor, LSP, ASCII or facade consumers, retire the remaining
legacy API names, finish package/legal projections or establish final artifact
size. Those U5-U9 requirements remain active. No timing, allocation-total or
final-byte improvement is claimed here.

# Storage Resolution

The preceding checkpoint accurately recorded approximately 1 GiB of free space
on F and rejected cleanup attempts. No blocked deletion was retried and no cache
file was removed. D had 137,424,207,872 available bytes, so the remaining serial
validation used one temporary shared target at
`D:/merman-u5-temp/verification-target`, with incremental compilation disabled,
two Cargo build jobs and two nextest threads. Source and Git remain in the
existing F-drive checkout; no new worktree or C-drive target was created.

After the full core build, F still had 1,038,000,128 free bytes and D had
134,134,243,328 free bytes. The earlier cleanup questions remain unanswered;
validation no longer depends on their approval.

# Final Verification

Logs are retained in `D:/merman-u5-temp/`.

| Selection | Result | Nextest run ID |
| --- | --- | --- |
| `all-diagrams`, no default/editor/presentation capabilities, lib and all test targets | 471 passed, zero skipped | `d45398dd-e1ae-4fa4-ae92-6af76bb963b2` |
| `all-diagrams,editor-facts`, no presentation, lib and all test targets | 511 passed, zero skipped | `846bb75d-0da7-4f47-92de-f2a04c53b0b1` |
| All features, lib and all integration targets | 2,275 passed; one stale golden failure described below; zero skipped | `74b4a4a4-e7d0-4e02-8c14-b3cbba10dc05` |
| Complete snapshot target after the three precise golden repairs | 2 passed, zero skipped | `c690236b-f676-49c8-9cab-53ddd75055e9` |

The earlier final compatibility/no-editor selection passed 1,381 tests
(`83c3270c-0cf3-420a-8ad1-22f6e3c3a573`). The public defaults-off compilation
proofs passed four doctests; defaults-off all test targets typechecked; production
all-feature Clippy passed with `-D warnings`. Their exact commands and logs remain
in the preceding checkpoint. The later Rust edit only repairs the lifecycle test
access described below. Workspace format checking, diff whitespace checking and
the repository's indexed path-length check passed.

The broad full gate first exposed five stale Class field accesses in
`grammar_carrier_lifecycle.rs`. The existing paired Class result exposes borrowed
`classes()` and `namespaces()` iterators. Adapt the test to those methods and sum
the same namespace identifier bytes from the iterator keys. Preserve its source
sizes, counts, small-stack subprocesses, timeout and clone/drop checks. The
complete full run then passed that lifecycle target.

The remaining full-run failure was a previously omitted GitGraph golden update.
Commit `ca140bcb6062c51d88b7a6c513082bdaaa6c45cb` intentionally attached duplicate
commit warnings to original source statements and recorded its RED/GREEN proof
in the GitGraph construction receipt. Three existing golden files still omitted
the span. Their only changes now add the exact UTF-8 byte ranges:

- `upstream_accessibility_and_warnings`: 123..149.
- `upstream_docs_gitgraph_customizing_main_branch_name_015`: 642..660.
- `upstream_pkgtests_gitgraph_spec_054`: 527..553.

Each range was checked against the original fixture bytes and selects its
duplicate `commit` statement, including the frontmatter and Unicode-offset cases.
Messages, rule IDs, model values and fixture source bytes are unchanged. Keep the
complete equality assertion and existing comparator normalization. Both snapshot
tests passed after these changes; no production behavior changed to fit a golden.

# Review And Continuation

The preceding checkpoint records the five completed simplification lenses and
shared/family correctness reviews. Their applied findings include restoring 47
mixed tests rather than excluding their ordinary assertions with editor support.
Follow-up source review covers the final lifecycle/golden repairs separately.

Remote main was fetched again at this boundary. It remains
`71da4cf6fbbf4fcf27fb0465cc9169f6fd6ebd8b`, already included by merge
`7c6914251de521e29f55cbb5129e0269e5dee6ca`. The current branch has zero commits
behind it. Only the remote maintenance branch and `v0.9.2` tag changed; main adds
no new plan dependency. Nothing was pushed or published.

The next bounded unit migrates default analysis to canonical captures and keeps
custom registry analysis explicitly full-only. Read-only inventory found no
production callers of `Analyzer::with_engine` outside analysis, but identified
its custom tests and old full test helpers that must retain their real contracts.
Do not add full dev-dependency features merely to hide those stale helpers.
Continue the approved overall goal rather than marking it complete.

The optional whole-bundle memory validator reports 77 unchanged historical files
without typed frontmatter and exits 1. The existing HEAD version of the theme
retirement inventory confirms that legacy format. Neither new checkpoint appears
in the validator's findings. Preserve those historical documents; this is not a
core test failure or a reason to broaden the refactor into a memory migration.
The complete diagnostic log is `editor-isolation-memory-validation.log`.
