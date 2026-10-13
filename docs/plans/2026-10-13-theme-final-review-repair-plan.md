# Theme final review repair

Repair the complete confirmed review of `f754a3c3..96eef309`. The maintainer
requires correct fixes for every discovered defect, with refactoring and cleanup
where warranted. This implementation supersedes the earlier report-only scope.

## Scope and success

- Correct all 68 independently confirmed findings, including the five P1 issues.
- Strengthen all 26 concrete test gaps with assertions on the actual contract or
  terminal output, rather than string presence or receipts alone.
- Assess all 36 actionable necessity records. Consolidate proven duplicate
  internal facts; preserve public compatibility where removal is unproven.
- Preserve Mermaid source semantics, cancellation, admission limits, feature
  closure, and operation-level reporting. Do not mask failures by weakening gates.
- Verify the repaired feature configurations and relevant existing suites. Record
  the exact checks and their limits; claim performance gains only with evidence.
- Reset theme-specific version domains introduced on this unmerged branch to V1,
  regenerate their fingerprints from the compiler, and remove obsolete unmerged
  version migration paths. Retain explicit V1 validation, unknown-version rejection,
  independent published ABI contracts, and truthful historical records.

## Implementation

Use fresh workers with exclusive file ownership for the units in the
[runtime ledger](../workstreams/theme-final-review-repair-20261013/WORKSTREAM.json).
The first wave covers math/Sequence, Flowchart/State/Class, Block/ER/TreeView,
core/export/pipeline, and tooling/bindings. The second wave covers the remaining
geometry and paint families. Stable independent second-wave work proceeded while
first-wave platform checks were running; shared file ownership stayed explicit and
Cargo execution remained serialized.

Workers implement source-backed fixes and focused regressions. The primary agent
alone runs Cargo, coordinates shared files, stages changes, and commits integrated
units. Use two build jobs, the existing target directory, and nextest where
applicable. Do not overwrite unrelated edits. No publishing or merging is in scope.

Characterize existing failures before fixing them where feasible; the preserved
review probes already provide red evidence for several public paths. Prefer
strengthening existing tests over adding redundant scaffolding. Migrate internal
callers and delete superseded private helpers within the same unit.

## Integration and completion

Recheck every stable finding/gap/cleanup ID against the final implementation and
its verification. Run formatting, appropriate lint and feature checks, and an
independent review of the complete repair diff. Resolve integration failures and
newly discovered regressions before declaring completion. Each unit must record
modified files, tests, outcomes, and any justified compatibility preservation.

The immutable review input is [issues.json](../workstreams/theme-final-review-repair-20261013/issues.json).
Execution status belongs in the runtime ledger, not this plan.
