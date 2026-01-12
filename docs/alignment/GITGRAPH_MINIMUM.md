# GitGraph Minimum Slice (Phase 1)

This document defines the initial, test-driven minimum slice for GitGraph parsing in `merman`.

Baseline: Mermaid `@11.12.2`.

Upstream references:

- Parser entry: `repo-ref/mermaid/packages/mermaid/src/diagrams/git/gitGraphParser.ts`
- DB behavior: `repo-ref/mermaid/packages/mermaid/src/diagrams/git/gitGraphAst.ts`
- Tests: `repo-ref/mermaid/packages/mermaid/src/diagrams/git/gitGraph.spec.ts`

## Supported (current)

- Headers:
  - `gitGraph`
  - `gitGraph:`
  - `gitGraph <LR|TB|BT>:` (direction)
- Statements (line-oriented):
  - `commit`
    - `commit "message"`
    - `commit msg: "message"`
    - `commit id:"id" tag:"tag" type:<NORMAL|REVERSE|HIGHLIGHT> msg:"message"` (key/value pairs; order is flexible)
  - `branch <name>` (supports quoted names and common git-friendly characters)
    - `branch <name> order:<n>`
  - `checkout <name>` / `switch <name>`
  - `merge <branch>` with optional `id`, `tag`, `type`
  - `cherry-pick id:"<id>"` with optional `tag` and `parent`
- Comments:
  - Lines starting with `%%` are ignored.
- Accessibility:
  - `accTitle: ...`
  - `accDescr: ...`
  - `accDescr { ... }` multi-line blocks (trims line indentation and joins with `\n`)

## DB-level behavior (Phase 1)

- Default branch name/order comes from `gitGraph.mainBranchName` / `gitGraph.mainBranchOrder`.
- `branch` creates a new branch at the current `head` (or empty if no commits) and switches to it.
- `commit`:
  - Auto-generates a read-only id when not provided (prefix includes sequence number).
  - Creates a parent link to the current head when present.
  - Warns when a commit id is reused (`Commit ID <id> already exists`).
- `merge`:
  - Produces a merge commit with 2 parents (current head + other branch head).
  - Matches Mermaid error messages for invalid merges (unknown branch, same branch, missing commits, duplicate custom id).
- `cherry-pick`:
  - Produces a cherry-pick commit on the current branch.
  - Mirrors Mermaid validations for merge-commit cherry-pick parent selection.

## Output shape (Phase 1)

- Headless output snapshot:
  - `type`
  - `commits`: ordered by `seq`
  - `branches`: `[{ name }]` sorted by `order` (Mermaid `getBranchesAsObjArray()`-like)
  - `currentBranch`
  - `direction`
  - `accTitle`, `accDescr`
  - `warnings`: collected warning messages
  - `config`

## Alignment goal

This is an incremental slice. The ultimate goal is full Mermaid `gitGraph` parsing and DB behavior
compatibility at the pinned baseline tag.

