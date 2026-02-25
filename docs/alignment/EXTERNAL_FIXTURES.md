# External Fixture References

This document tracks fixtures that are sourced from outside Mermaid's own repository.

As of the Mermaid `@11.12.3` alignment baseline, this repository does not check in external
fixtures. Keeping fixtures sourced only from upstream Mermaid makes SVG parity gating clearer and
avoids mixing baselines from other renderers.

Pinned external repositories (commit hashes) live in `tools/upstreams/REPOS.lock.json`.

We may still use other renderers (e.g. `repo-ref/mermaid-rs-renderer`) for performance comparisons
and exploratory debugging, but any fixtures derived from those repos should remain local and
should not be checked in.
