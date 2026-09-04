# ADR-0001: Upstream Baseline (mermaid@11.17.2)

## Status

Accepted

## Context

`merman` is a 1:1 re-implementation of Mermaid. To keep behavioral compatibility measurable, the
project must pin an upstream baseline (tag + commit) that all alignment tests and docs refer to.

## Decision

- Baseline tag: `mermaid@11.17.2`
- Baseline commit (reference checkout): `dcb694ddb58dc5ad3502e7e903cac05fd812eac3`
- Reference source location: `repo-ref/mermaid` (optional local checkout at the baseline commit)
- Pinned revisions are tracked in `tools/upstreams/REPOS.lock.json` (not git submodules).
- The baseline support claim is limited to the implemented diagram matrix in
  `docs/alignment/STATUS.md`; new upstream diagram families can be deferred or out of scope there.

## Consequences

- All alignment specs must reference the baseline tag/commit unless they are explicitly preserving
  historical fixture evidence.
- Any upstream update must update this ADR or add a successor ADR to record the new baseline and the
  intended upgrade path.
