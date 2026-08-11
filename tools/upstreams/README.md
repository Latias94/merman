# Upstream checkouts

This repository uses optional local checkouts under `repo-ref/` for parity work. They are ignored,
are not submodules, and must resolve to the selected revisions in `REPOS.lock.json`.

Typical selected checkouts include:

- `repo-ref/mermaid` for Mermaid `11.16.1`;
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

## Baseline generation

After reviewed render evidence confirms the selected graph, run:

```bash
cargo run -p xtask -- gen-mermaid-reference --refresh-provenance
```

The refresh renders every primary upstream SVG family and records the selected reference identity.
The ordinary generator deliberately leaves provenance unchanged so a lock-only edit cannot relabel
existing baselines.
