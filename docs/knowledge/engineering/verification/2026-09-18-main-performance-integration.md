# Main Performance Repair Integration

Date: 2026-09-18. Integration parents: theme branch `61d795f41` and fetched
`origin/main` `f6b041131`. The incoming range contains seven commits after
`9edd6d86a`, including PR #139. The earlier source-theme admission and Base evaluator
repairs remain integrated through `620a35ac9`.

## Scope and merge decisions

- Flowchart image-label layout adopts the bounded prefix checks and forward-only
  tag scan from main. Main's Unicode, malformed-tag, repeated-tag, and public SVG
  regressions are retained.
- Flowchart HTML emission keeps the branch's existing module-level image helpers.
  They already compare bounded ASCII prefixes without repeatedly allocating a
  lowercase suffix. The incoming tests exercise that implementation; the removed
  nested helper is not restored.
- Formula measurement keeps `math::prepared` as the shared layout boundary. It
  already measures display-list ink bounds without generating discarded SVGs.
  The merge adopts the clearer metrics naming and incoming measurement-versus-SVG
  regressions, while retaining blank-line height and style-aware Sequence metrics.
- LSP semantic-token work uses main's shared worker budgets, compact line indexes,
  and session-bounded index reuse. Export font-resolver test initialization also
  adopts the incoming cleanup.
- No dependency, public theme schema, release version, or artifact-size budget is
  changed by this merge. Main's performance reports describe their original
  revisions and hosts; they are not new measurements of this theme branch.

## Verification

- Renderer Release nextest with `math,layout-cytoscape`: **2,924 passed, two skipped**.
  This covers library tests and the Flowchart SVG, label-measurement, node-effect,
  marker, theme-resolution, and support-discovery integration targets.
- Workspace formatting and staged whitespace checks passed.
- LSP nextest with `--all-targets --features stdio --test-threads 2`: **331 passed,
  none skipped**, including real stdio-process tests. The initial build stopped
  with `No space left on device` before tests ran. Three inactive September 15
  renderer incremental-cache directories were removed to recover build space;
  no sources, test executables, benchmark artifacts, or local edits were removed.
  The unchanged test command passed on retry.

The checked working tree also contains the ongoing
Flowchart radius applicability changes, which are intentionally excluded from the
merge commit. The contents of all 14 previously modified tracked files were
checked against their pre-merge hashes and preserved. Existing untracked work is
also retained.

This merge does not close U6, C7a, the full platform matrix, or the final performance
and artifact-size audit.
