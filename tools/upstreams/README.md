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
generated projections. `ZENUML_CORE_ADMISSION.json` keeps the oracle-to-candidate decision evidence,
`ZENUML_BROWSER_SECURITY_EVIDENCE.json` records executable desktop/mobile isolation and security
observations, and `ZENUML_CORE_V4_DEFERRED_ADMISSION.json` binds the outside-range `4.2.0` identity
to a separate unfinished major-admission inventory. The package-manager lockfiles remain generated
materialization evidence; do not edit them by hand.

"Latest" is an observation, not an automatic selection rule. The pinned Mermaid workspace lock is
the behavior oracle. The newest stable package inside an upstream-declared compatible range is an
admission candidate, and it replaces the oracle only after every recorded behavior, security, and
resource gate passes. A newer major outside that range remains a separate admission.

The deferred v4 artifact is not a source checkout, implementation claim, or feature decision. It
must retain zero impact on the selected `3.50.1` graph until its grammar, semantic, editor/LSP,
render, browser-security, resource, license, and release-surface evidence is independently closed.
`verify-mermaid-reference` validates that artifact against the exact latest-stable identity in the
reference bundle and fails on a missing or modified artifact.

The selected `@zenuml/core@3.50.1` package declares an exact optional CLI peer on
`playwright-core@1.57.0`, while the Playground browser-test harness uses `1.61.1`. The application
runtime and browser-test harness are therefore separate npm projects with separate locks. The
runtime tree intentionally leaves the optional CLI peer absent; the test tree owns Playwright and
Axe without making either visible to ZenUML Core. Both trees must pass independent `npm ci` and
`npm ls --all` gates. Do not collapse the trees or hide a conflict with `--force`,
`legacy-peer-deps`, an override, or a Playwright downgrade.

## How to populate

Clone each repository at the pinned commit shown in `tools/upstreams/REPOS.lock.json`.

Install the Playground runtime, its `playground/tests` browser-test toolchain, and the reference CLI
with lifecycle scripts disabled (the package-local `.npmrc` files enforce this and the official npm
registry). Any required install action must first be added to the audited allowlist in the reference
bundle and invoked explicitly.
Playground `dev`, `build`, and `test` invoke their owned runtime preparation directly; do not move
that work into `pre*` or `post*` hooks because `ignore-scripts=true` intentionally disables them.

Use `cargo run -p xtask -- verify-mermaid-reference` for the clean-clone static contract (bundle,
locks, provenance, admission evidence, and generated projections). After populating `repo-ref/`
and running scriptless npm installs, add `--materialized` to verify checkout commits, installed
package versions, and content hashes.

Verify the selected ZenUML graph without regenerating evidence:

```bash
cd playground
npm run verify:dependencies
npm run verify:zenuml-browser-admission
npm run verify:zenuml-candidate
```

When a security-critical source named by `ZENUML_BROWSER_SECURITY_EVIDENCE.json` changes, run the
real desktop/mobile Chromium probes and record their observed results before regenerating candidate
evidence:

```bash
cd playground
npm run record:zenuml-browser-admission
npm run verify:zenuml-candidate:online -- --write
```

Do not hand-edit probe counts, pass states, source hashes, or candidate summaries. The Rust verifier
derives them from the probe contract and per-project observations.

After reviewed render evidence confirms a new reference graph, run
`cargo run -p xtask -- gen-mermaid-reference --refresh-provenance` to update the owned upstream SVG
manifests. The ordinary generator deliberately leaves provenance stale so a dependency-only lock
change cannot silently re-attest existing baselines.
