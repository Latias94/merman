# Upstream checkouts

This repository uses **optional, local** checkouts under `repo-ref/` for parity work.

These checkouts are **not committed** and are **not** git submodules. Pinned revisions are tracked
in `tools/upstreams/REPOS.lock.json`.

Typical layout:

- `repo-ref/mermaid` (Mermaid upstream)
- `repo-ref/dagre` (Dagre upstream)
- `repo-ref/graphlib` (Graphlib upstream)
- `repo-ref/dompurify` (DOMPurify upstream)
- `repo-ref/sanitize-url` (sanitize-url upstream)

## How to populate

Clone each repository at the pinned commit shown in `tools/upstreams/REPOS.lock.json`.

