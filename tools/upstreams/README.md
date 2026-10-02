# Upstream checkouts

This repository uses optional local checkouts under `repo-ref/` for parity work. They are ignored,
are not submodules, and must resolve to the selected revisions in `REPOS.lock.json`.

Typical selected checkouts include:

- `repo-ref/mermaid` for Mermaid `12.0.0`;
- `repo-ref/dompurify` for the selected sanitizer source;
- `repo-ref/zenuml-core` for the selected ZenUML Core `3.50.1` source;
- the selected Dagre, Graphlib, Cytoscape, and layout sources listed in the lock.

## Standing selected-reference contract

`MERMAID_REFERENCE_BUNDLE.json` describes only the current graph: selected package versions and
integrities, selected source commits, runtime registrations, workspace/lock ownership, installed
content digests, built-in registry inputs, and generated projections. Registry source digests
use UTF-8 source with CRLF normalized to LF, matching Git blobs across checkout platforms. It deliberately
contains no oracle, candidate, deferred-major, browser-admission, or attestation payload.

`MERMAID_SELECTION_DECISION.json` is the compact reviewed decision receipt. The bundle stores only
its path and SHA-256. The receipt binds the previous and current selection identity digests, their
exact changed fields, the official npm command/version and package identities used during
admission, the behavior outcome, and the admission output digest. The one bootstrap receipt marks
its digest as a historical aggregate because the original npm stdout was not archived; it does not
pretend that aggregate is official-tool output.

Packages loaded by reference execution, including the Puppeteer browser-driver closure and the
esbuild compiler, are selected packages rather than ambient tooling. They carry registry integrity,
source identity, installed-content digests, lock verification, and selection-receipt coverage
alongside Mermaid and its runtime companions.

The executable behavior oracle is the selected npm graph. `tools/mermaid-cli/reference-runtime.mjs`
builds a self-contained browser artifact from Mermaid's module entry point using the selected
esbuild compiler and the reference CLI's installed packages. Baseline generation, renderability
audits, and benchmark/debug consumers share this builder. Published `mermaid.js` and
`mermaid.esm.mjs` bundles embed their own dependency copies, so changing an npm override alone
does not update those bundles.

The renderer probes the built artifact's Mermaid version and verifies that a strict HTML-label
render calls its selected DOMPurify instance. It records the artifact SHA-256 and compiler identity,
then executes the verified bytes. Package and artifact drift block baseline promotion.

Mermaid 12.0.0 owns ELK in its standard runtime. Do not register the legacy ELK adapter that the
reference CLI still carries transitively: it replaces core layout/shape registrations and can
break Agentflow and Usecase.

Tidy Tree 1.0.1 and ZenUML 1.0.1 remain external companions. Their published package tags resolve
to `a86a2bf4d8fd2a9045f564b5b37c4c70cde18ca6`, independently of Mermaid 12.0.0's
`98a0945418c76238f15df2afaddbba4272656c3b`. The bundle records each package's own source and
installed-content identity. Historical baseline and source-corpus records retain their original
versions until new target output is generated and reviewed.

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
version. It pins Node `24.21.0` and npm `12.0.2`, installs packages with lifecycle scripts disabled,
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

### Staging runtime projections during an upgrade

Before promoting a new bundle, the existing generators can project an isolated installation
using a temporary bundle descriptor under `target/`. Only its release runtime identity and
reference workspace are used by this mode; successful projection does not admit the complete
dependency graph or the selection receipt. The installed Mermaid version and package digest
must match the descriptor. Both output paths must be explicit:

```text
cargo run -p xtask -- gen-default-config --reference-bundle target/upgrade/bundle.json --out target/upgrade/default_config.json --shape-out target/upgrade/default_config_shape.json
cargo run -p xtask -- gen-theme-snapshot --reference-bundle target/upgrade/bundle.json --out target/upgrade/theme_variables.json --audit-out target/upgrade/theme_oracle.json
```

Without `--reference-bundle`, generators continue to enforce the selected workspace pins.
Keep staged output outside accepted baseline directories until the implementation is ready.
SVG generation uses the owned reference artifact through the shared Puppeteer renderer and
registers only the external diagram and layout plugins explicitly selected in the reference workspace.
An absent theme remains absent; the runner no longer supplies the CLI's explicit `default`
theme or registers its transitive ELK plugin over Mermaid's built-in implementation.
