# Parser equivalence follows preprocessing policy

## Failure and contract

The parser fuzz failure in run `40479759415` is an oracle error. The production parser intentionally
uses different preprocessing policies. The smallest regression is:

```mermaid
gitGraph
%%{
merge x
```

Strict preprocessing removes the unfinished directive through EOF and accepts the empty
`gitGraph`. Lenient preprocessing uses `RecoverLine`, retains `merge x`, and suppresses the family
failure into an `error` model. Both calls succeed, but their identities and models differ.
`gitGraph\n%%{\ncommit\n` also produces two successful `gitGraph` models with different contents.
Comparing identities before deciding whether to compare models cannot repair the general oracle.

The shared oracle now compares semantic JSON with the complete typed-model projection separately
for each mode. Each mode also compares its semantic and typed known-type entrypoints directly,
including error outcomes. A family identity retained by strict success or failure remains the
known-type input when lenient suppression changes the output identity to `error`. The existing
strict detected/known-type comparison remains for accepted diagrams. The fuzzer also retains its
independent catalog-selected family input.

Editor snapshots use `RecoverLine` without family-error suppression. Their parsed metadata and
models match the lenient semantic path. Their retained failures match the lenient error model's
metadata, with only the diagram identity replaced by `error`, and its original error message.
Preprocessing detection failures correspond to no lenient diagram; other preprocessing failures
retain their error categories. Parser panics, missing editor facts, and invalid source spans still
fail. The production `recovered_incomplete_directive` receipt only gates the additional strict
snapshot comparison; it does not bypass recovery-path assertions. No directive scanner or
source-substring exclusion is introduced.

## Regressions and negative controls

`parse_projection_contract` embeds the minimum input and the original CI input without depending
on an untracked seed. It compares each result with its explicit policy-specific source, and runs
both the detected and known-type shared assertions. Existing corpus enumeration remains intact.
The successful-model regression includes ordinary input and a complete directive as controls
where strict and lenient results agree.

The negative controls require these exact assertion categories:

- `recovery_does_not_hide_a_strict_typed_path_acceptance_change`: the minimum input's strict
  semantic result is paired with an invalid strict typed input; `different parser acceptance`
  must panic.
- `recovery_does_not_hide_a_lenient_known_type_model_change`: recovered lenient semantic and
  known-type typed models have different commit IDs but identical metadata; `complete semantic
  projection` must panic.
- `recovery_receipt_does_not_hide_a_snapshot_model_change`: a real snapshot with a recovery
  receipt has a different commit ID; `snapshot recovery projection` must panic.

These supplement the existing Flowchart acceptance and edge-endpoint controls. The controls are
Rust tests awaiting execution, not a claim of a completed mutation-test run.

## Feature and branch boundaries

On the main worktree, snapshot APIs require both `compat-json` and `editor-facts`.
`fuzz/Cargo.toml` already requests `merman/compat-json`; its new default local `editor-facts`
feature forwards `merman/editor-facts`. The local feature is necessary because the shared file's
`cfg(feature = "editor-facts")` is evaluated in the including fuzz package, even if a transitive
dependency already enables that core feature. Disabling fuzz default features omits snapshot
checks while retaining both semantic/render pairs and known-type checks.

The 09 worktree starts from `917d75836` and has unconditional compatibility JSON and snapshot
APIs. Its new shared support file uses local aliases for `Engine`/`ParseMetadata` and the older
effective-config representation; snapshot assertions are unconditional. Its new focused
`parse_projection_contract` suite includes policy regressions, negative controls, corpus replay,
and catalog-selected invalid known-type sources. Its older libFuzzer entrypoint is unchanged and
does not include this shared file. The main worktree already includes it from `fuzz/src/lib.rs`.
No production parser, unrelated fuzz source, or corpus seed is changed by this repair.

## Validation handoff

Cargo compilation and Rust test execution are intentionally deferred to the coordinating agent.
Run these serially in the indicated worktree:

```sh
# Main: with and without editor snapshot assertions.
cargo nextest run --locked -p merman-core --no-default-features --features all-diagrams,compat-json,editor-facts --test parse_projection_contract
cargo nextest run --locked -p merman-core --no-default-features --features all-diagrams,compat-json --test parse_projection_contract
# Main: default fuzz recipe includes snapshot assertions and the existing CI/minimal seeds.
cargo nextest run --locked --manifest-path fuzz/Cargo.toml --lib incomplete_directive_recovery_preserves_each_parse_path_contract
# 09: editor snapshots and compatibility JSON are unconditional.
cargo nextest run --locked -p merman-core --no-default-features --features all-diagrams --test parse_projection_contract
```

Non-compiling checks: direct `rustfmt --check` for the four changed Rust files, Git whitespace
checks, TOML feature-forwarding inspection, and the existing Python fuzz configuration contracts.
These checks do not establish that the Rust suites pass.
