# ADR-0001: Upstream Baseline (mermaid@11.12.2)

## Status

Accepted

## Context

`merman` is a 1:1 re-implementation of Mermaid. To keep behavioral compatibility measurable, the
project must pin an upstream baseline (tag + commit) that all alignment tests and docs refer to.

## Decision

- Baseline tag: `mermaid@11.12.2`
- Baseline commit (reference checkout): `bd85b51e2`
- Reference source location: `repo-ref/mermaid` (optional local checkout at the baseline commit)
- Pinned revisions are tracked in `repo-ref/REPOS.lock.json` (not git submodules).

## Consequences

- All alignment specs must reference the baseline tag/commit.
- Any upstream update requires a new ADR to record the new baseline and the intended upgrade path.
