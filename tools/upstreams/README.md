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
- `repo-ref/zenuml-core` (Mermaid workspace oracle retained for comparative evidence)
- `repo-ref/zenuml-core-3.50.1` (matrix-selected compatible ZenUML Core behavior source)

`MERMAID_REFERENCE_BUNDLE.json` is the machine-readable release graph. It records npm integrity,
source provenance, external diagram/layout packages, the selected compatible behavior source, and
generated projections. `ZENUML_CORE_ADMISSION.json` keeps the oracle-to-candidate decision evidence.
The package-manager lockfiles remain generated materialization evidence; do not edit them by hand.

"Latest" is an observation, not an automatic selection rule. The pinned Mermaid workspace lock is
the behavior oracle. The newest stable package inside an upstream-declared compatible range is an
admission candidate, and it replaces the oracle only after every recorded behavior, security, and
resource gate passes. A newer major outside that range remains a separate admission.

The selected `@zenuml/core@3.50.1` package declares an exact optional CLI peer on
`playwright-core@1.57.0`, while the Playground browser-test harness uses `1.61.1`. npm therefore
reports that dev-only peer as invalid in `npm ls --all`, even though the selected browser entry does
not import it. The admission descriptor classifies this upstream metadata residual. Do not hide it
with `--force`, `legacy-peer-deps`, or a Playwright downgrade; exact locks, runtime-entry evidence,
and browser gates are the owned contracts. The Playground manifest and Pages workflow require
npm 11.17.0 or newer because earlier optional-peer resolvers cannot clean-install this graph; the
workflow uses Node 24 and installs the exact recorded npm toolchain before dependency installation.

## How to populate

Clone each repository at the pinned commit shown in `tools/upstreams/REPOS.lock.json`.

Install the Playground and reference CLI with lifecycle scripts disabled (the package-local
`.npmrc` files enforce this and the official npm registry). Any required install action must first
be added to the audited allowlist in the reference bundle and invoked explicitly.
Playground `dev`, `build`, and `test` invoke their owned runtime preparation directly; do not move
that work into `pre*` or `post*` hooks because `ignore-scripts=true` intentionally disables them.

Use `cargo run -p xtask -- verify-mermaid-reference` for the clean-clone static contract (bundle,
locks, provenance, admission evidence, and generated projections). After populating `repo-ref/`
and running scriptless npm installs, add `--materialized` to verify checkout commits, installed
package versions, and content hashes.

After reviewed render evidence confirms a new reference graph, run
`cargo run -p xtask -- gen-mermaid-reference --refresh-provenance` to update the owned upstream SVG
manifests. The ordinary generator deliberately leaves provenance stale so a dependency-only lock
change cannot silently re-attest existing baselines.
