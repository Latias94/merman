# ADR-0001: Upstream Baseline (mermaid@11.15.0)

## Status

Accepted

## Context

`merman` is a 1:1 re-implementation of Mermaid. To keep behavioral compatibility measurable, the
project must pin an upstream baseline (tag + commit) that all alignment tests and docs refer to.

## Decision

- Baseline tag: `mermaid@11.15.0`
- Baseline commit (reference checkout): `41646dfd43ac83f001b03c70605feb036afae46d`
- Reference source location: `repo-ref/mermaid` (optional local checkout at the baseline commit)
- Pinned revisions are tracked in `tools/upstreams/REPOS.lock.json` (not git submodules).
- The baseline support claim is limited to the implemented diagram matrix in
  `docs/alignment/STATUS.md`; new upstream diagram families can be deferred or out of scope there.

## Consequences

- All alignment specs must reference the baseline tag/commit.
- Any upstream update requires a new ADR to record the new baseline and the intended upgrade path.
