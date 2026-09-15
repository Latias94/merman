# Theme release-line preparation

Date: 2026-09-15. Source: `753cdc1c2`, following the archive candidate `57ec788c0`.
This is a local version proposal and compatibility experiment, not a selected release or freeze.

## Fuzz lockfile repair

Preparing a new release initially failed before applying any caller changes: the fuzz workspace's
lockfile did not contain the current theme dependency graph. The root lockfile normalized cleanly;
the missing `merman-theme-contract`, canonical serializer and local dependency-edge updates were
confined to `fuzz/Cargo.lock`. Cargo's offline workspace update refreshed that lock without
upgrading existing registry packages. Commit `753cdc1c2` contains the generated result.

`cargo check --locked --manifest-path fuzz/Cargo.toml` and the root facade's locked SVG check passed.
The stable toolchain check is a lock/compile observation, not the nightly fuzz campaign.
The release coordinator correctly rejected the original drift and left its caller unchanged;
its rejection must not be relaxed to combine dependency changes with a version bump.

## Proposed versions tested

Both proposals used the existing transactional `release-version.py set` coordinator in fresh,
linked worktrees of an independent clone. Cargo 1.95.0 and npm 12.0.2 were used; Node 24.21.0 and
npm 12.0.2 came from task-local installations. An initial PATH ordering selected Node's bundled
npm 11.19.0 and was rejected; placing the npm 12 executable first resolved that environment issue.
The ordinary maintainer worktree was not assigned a new release version.

| Proposal | Version projections | Fresh candidate consumer | Published alpha.6 with candidate siblings |
| --- | --- | --- | --- |
| `0.8.0-alpha.7` | Passed | Passed | Failed with three E0004 errors in the published facade's `diagnostic.rs` |
| `0.9.0-alpha.1` | Passed | Passed | Intentionally not combined: outside the 0.8 Cargo compatibility line |

The command was `python3 scripts/verify_prerelease_compatibility.py --version <proposal>
--previous-version 0.8.0-alpha.6`. This existing check uses the ASCII facade feature recipe; it is
not an all-feature compatibility audit. The positive candidate compile and the rejected
same-line old consumer are real Cargo executions with fresh consumer lockfiles.

Published alpha.6 sibling requirements are not exact. Its facade exhaustively matches the old
`merman_core::Error`; the new core adds `Internal`, `ThemeEvaluationLimit`, and `non_exhaustive`.
The registry facade therefore cannot compile against these new siblings. Replacing correctly
classified errors or restoring obsolete public APIs solely to fit that old match is not proposed.
A new 0.9 release line is the recommended alternative under the existing release admission rule.
This does not change the new, unpublished theme schemas, recipe revisions or catalog versions:
they remain 1. The workspace/package release and the theme wire contract are independent axes.

## Reviewable local proposals

Both proposals have 29 version-owned files, 117 additions and 117 deletions. No error-model or
compatibility alias change was applied. The coordinator completed and the resulting version
projections and whitespace checks passed.

- Alpha.7 worktree: `/tmp/merman-alpha7-linked2`; patch
  `/tmp/merman-alpha7-version-proposal.patch`, SHA-256
  `77f5de865c35ff52ecb63a3ea7f8bd6b982d57b18fd3074be90938e8661c0060`.
- 0.9 alpha.1 worktree: `/tmp/merman-alpha9-linked`; patch
  `/tmp/merman-alpha9-version-proposal.patch`, SHA-256
  `3e3df906697dc649d33468c39497b4a588bdb149e0ea129090c8bf5bf895444c`.
- Logs: `/tmp/merman-alpha7-{version-check,compatibility}.log` and
  `/tmp/merman-alpha9-{version-prep,version-check,compatibility}.log`.
- Release-projection, prerelease-compatibility and fuzz-configuration unit contracts: 33 passed.
  Those fixture-based tests are separate from the actual Cargo results above.

After the maintainer selects the release line, regenerate the version projection from the selected
clean source rather than applying an old proposal over intervening changes. Review version-specific
migration promises, installation examples and package-local changelogs, stamp the intended release
date, then run immutable preflight. Windows and remaining host/compiler-floor lanes, final
artifact/catalog rollout and C7a contract freeze still need their owner evidence. Selecting a
version alone does not complete those gates, and no tag or publishing workflow was triggered.
