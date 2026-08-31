# Upstream checkouts

This repository uses optional local checkouts under `repo-ref/` for parity work. They are ignored,
are not submodules, and must resolve to the selected revisions in `REPOS.lock.json`.

Typical selected checkouts include:

- `repo-ref/mermaid` for Mermaid `11.17.2`;
- `repo-ref/dompurify` for the selected sanitizer source;
- `repo-ref/zenuml-core` for the selected ZenUML Core `3.50.1` source;
- the selected Dagre, Graphlib, Cytoscape, and layout sources listed in the lock.

## Standing selected-reference contract

`MERMAID_REFERENCE_BUNDLE.json` describes only the current graph: selected package versions and
integrities, selected source commits, runtime registrations, workspace/lock ownership, installed
content digests, built-in registry inputs, and generated projections. It deliberately contains no
oracle, candidate, deferred-major, browser-admission, or attestation payload.

`MERMAID_SELECTION_DECISION.json` is the compact reviewed decision receipt. The bundle stores only
its path and SHA-256. The receipt binds the previous and current selection identity digests, their
exact changed fields, the official npm command/version and package identities used during
admission, the behavior outcome, and the admission output digest. The one bootstrap receipt marks
its digest as a historical aggregate because the original npm stdout was not archived; it does not
pretend that aggregate is official-tool output.

Packages loaded by reference execution, including the Puppeteer browser-driver closure, are
selected packages rather than ambient tooling. They carry registry integrity, source identity,
installed-content digests, lock verification, and selection-receipt coverage alongside Mermaid and
its runtime companions.

The executable behavior oracle is the selected npm graph, not an assumption that every companion
was rebuilt from the Mermaid host tag. In the `11.17.2` graph, `@mermaid-js/layout-elk@0.2.3` is the
latest published ELK adapter and was built from its own package tag at commit
`293b1c153a6f94c3a4a1d9cd5eae4dde609f1ec4` (the Mermaid `11.17.0` release line). Its installed
artifact therefore remains authoritative for ELK DOM details, including `edges edgePath`, where it
differs from later `11.17.2` host source. The bundle records that package tag and the exact installed
content digest rather than relabeling the artifact as a `11.17.2` build.

Run the offline-capable standing gate with:

```bash
cargo run -p xtask -- verify-mermaid-reference
```

When reviewing a reference change, bind it to a trusted base bundle:

```bash
cargo run -p xtask -- verify-mermaid-reference --base <trusted-base-sha>
```

The transition gate reads the base bundle through `git show`. A changed selected identity requires
an exact previous/current receipt and field diff. An unchanged identity cannot replace its receipt;
the committed bootstrap is the only explicit exception for a base that predates receipts. Its
historical evidence commit must be an ancestor of the trusted base, and the referenced Git object
must still match the receipt digest.

After populating checkouts and installing the Playground and reference CLI with lifecycle scripts
disabled, verify materialized source and installed bytes with:

```bash
cargo run -p xtask -- verify-mermaid-reference --materialized
```

## Explicit upgrade admission

Candidate discovery, future-major evaluation, official signature verification, behavior
comparison, and browser security probes are manual upgrade work. They are not inputs to ordinary
CI, Pages, or release verification, and completed candidate/deferred evidence is not kept as a
live repository gate.

Use the read-only **Mermaid upgrade admission** workflow with an exact ZenUML Core candidate
version. It pins Node `24.6.0` and npm `11.17.0`, installs packages with lifecycle scripts disabled,
runs the official command
`npm audit signatures --json --include-attestations --registry=https://registry.npmjs.org/`,
compares the selected and candidate fixture behavior in Chromium, validates strict inline SVG, and
runs the browser probe contract in `ZENUML_BROWSER_ADMISSION_PROBES.json`. Its reports are workflow
artifacts for review, not standing repository inputs.

The same owner commands are available locally with the required exact toolchain:

```bash
cd playground
npm run admit:zenuml -- --candidate <exact-version> --output <candidate-report.json>
npm run admit:zenuml-browser -- --output <browser-report.json>
```

After the admission reports are reviewed and a new graph is selected, update the bundle and locks,
write a new selection receipt from the raw workflow outputs, regenerate projections, and run the
standing verifier with `--base` before refreshing upstream SVG provenance.

## Executable Cypress evidence

The retained new-family and Flowchart ELK Cypress scopes are historical Mermaid `11.16.1`
evidence. Mermaid `11.17.2` moved these tests from the old `cypress/` tree to the Playwright-based
`e2e/` tree, so the committed manifests intentionally keep their original source identity and
digests instead of being relabeled as `11.17.2`.

`tools/upstreams/cypress-collector/` remains an upgrade-only collector for those historical scopes.
It executes the selected historical checkout through its pinned Node, pnpm, and esbuild versions and
rejects unknown imports, helpers, runtime effects, skips, call-count drift, and toolchain drift.
Generated collection files belong under `target/` and are review inputs, not committed evidence.
After reviewing them, use `project-upstream-cypress-collection` to update the scope manifests under
`fixtures/_upstream/`. Ordinary alignment checks validate the historical manifests, their local
collector digests, and fixture routing without requiring the current `repo-ref/mermaid` checkout or
executing upstream JavaScript. See the collector README for exact commands.

## Baseline generation

After reviewed render evidence confirms the selected graph, run:

```bash
cargo run -p xtask -- gen-mermaid-reference --refresh-provenance
```

The refresh renders every primary upstream SVG family and records the selected reference identity.
The ordinary generator deliberately leaves provenance unchanged so a lock-only edit cannot relabel
existing baselines.
