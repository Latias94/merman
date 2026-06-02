# HPD-080 - Mermaid Source Checkout Audit

Date: 2026-06-02
Task: HPD-080 visible rendering defect triage

## Source Authority

- Lockfile: `tools/upstreams/REPOS.lock.json`
- Mermaid ref: `mermaid@11.15.0`
- Mermaid commit: `41646dfd43ac83f001b03c70605feb036afae46d`

## Finding

While scanning style providers, `repo-ref/mermaid` reported `railroad` and `cynefin` providers.
That contradicted the unsupported-family ledger, so the checkout state was audited before making any
renderer claim.

The local reference checkout had drifted to `develop`:

- `9bae92cd3214f9ec99369ab314ef41ffb283f6b6`

The lockfile commit was checked directly with `git ls-tree`. At the pinned Mermaid 11.15 commit,
`railroad` and `cynefin` are absent from `packages/mermaid/src/diagrams`, while `treeView`,
`ishikawa`, `eventmodeling`, `venn`, and `wardley` remain present unsupported-family candidates.

## Outcome

- Restored `repo-ref/mermaid` to detached HEAD at
  `41646dfd43ac83f001b03c70605feb036afae46d`.
- Re-ran style-provider discovery after the restore.
- Confirmed the existing HPD-080 theme coverage ledger remains consistent with pinned Mermaid 11.15
  source.
- No renderer code changed and no new supported-family renderability defect was found in this scan.

## Boundary

This was a reference-state repair, not a product behavior change. Future source-backed HPD claims
must use the lockfile commit, not whatever branch `repo-ref/mermaid` happens to be on.

## Verification

- `git -C repo-ref/mermaid rev-parse HEAD`
  returned `41646dfd43ac83f001b03c70605feb036afae46d` after the restore.
- `git -C repo-ref/mermaid ls-tree -d --name-only 41646dfd43ac83f001b03c70605feb036afae46d:packages/mermaid/src/diagrams`
  listed no `railroad` or `cynefin` directory.
- `rg --files repo-ref/mermaid/packages/mermaid/src/diagrams | rg "(style|styles|architectureStyles|ishikawaStyles|pieStyles)\.(ts|js)$"`
  matched the expected pinned-provider inventory.
