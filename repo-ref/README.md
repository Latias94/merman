# repo-ref

This directory holds **optional, local** checkouts of upstream repositories used for parity work.

These checkouts are **not committed** and are **not** git submodules. Only the pinned revisions are
tracked in `repo-ref/REPOS.lock.json`.

Typical layout:

- `repo-ref/mermaid` (Mermaid upstream)
- `repo-ref/dagre` (Dagre upstream)
- `repo-ref/graphlib` (Graphlib upstream)
- `repo-ref/dompurify` (DOMPurify upstream)
- `repo-ref/sanitize-url` (sanitize-url upstream)

## How to populate

Clone each repository at the pinned commit shown in `repo-ref/REPOS.lock.json`.
