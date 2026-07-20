# Mermaid Upgrade Playbook

Use this playbook together with `.agents/skills/align-mermaid-release` when selecting a Mermaid
release or changing any behavior-owning companion. A release is a pinned behavior graph, not a
single npm version. Completion means parser, semantics, editor services, headless output, browser
reference execution, examples, provenance, and package surfaces all describe the same graph.

## 1. Freeze The Delivery Boundary

Record the requested Mermaid release, exact tag and commit, current branch and dirty-tree ownership,
available `repo-ref` checkouts, and whether the task includes implementation, commits, or external
delivery. Do not publish, push, or move release tags as an implied part of alignment.

Read the repository skill, its admission checklist, `tools/upstreams/README.md`,
`tools/upstreams/MERMAID_REFERENCE_BUNDLE.json`, `tools/upstreams/REPOS.lock.json`, relevant ADRs,
and package-surface documentation before changing the graph.

## 2. Resolve Oracle, Candidate, And Latest Stable

For Mermaid and every external diagram or layout dependency, record:

- **oracle**: the exact package and source selected by the Mermaid workspace lock;
- **latest-compatible candidate**: the highest stable version satisfying the host package's
  published range;
- **latest-stable delta**: a newer stable version outside that range.

A compatible range establishes candidacy, not behavioral compatibility. Select the candidate only
after its named parser, renderer, security, resource, corpus, and host-integration matrix passes
without unexplained deltas. Retain the oracle if it does not. Treat an outside-range major as
separate behavior-port work rather than silently selecting it because it is newer.

An outside-range release still needs an independently validated deferred-admission artifact. Bind
its exact package integrity, source commit, host ranges, selected-graph impact, and a complete owned
behavior-work inventory. A deferred artifact must make no parser, renderer, editor/LSP, Playground,
feature, or release claim; changing any of those flags is a new admission workflow.

## 3. Generate And Materialize One Reference Graph

Move the source graph and all reference workspaces together. Before running the generator:

1. Update `tools/upstreams/MERMAID_REFERENCE_BUNDLE.json` with exact package versions, integrity,
   tarball and attestation URLs, source tags, commits, runtime registration hashes, selected
   companions, and the expected Playground/reference-CLI lock hashes.
2. Update every selected source checkout and commit in `tools/upstreams/REPOS.lock.json`. A bundle
   source with a `checkoutPath` must have the same repository and commit in the lock; the lock must
   not retain the previous Mermaid tag as an active source.
3. Update `playground/package.json` and `tools/mermaid-cli/package.json`, including their exact
   `overrides`, then regenerate both package locks with lifecycle scripts disabled. The reference
   CLI's direct Mermaid, CLI, layout, external-diagram, and behavior-source versions must resolve to
   the selected graph rather than whatever an upstream range happens to install.
4. Recompute the descriptor's workspace hashes from those reviewed manifests, locks, and reference
   config. Do not copy old hashes forward.

Then generate the owned projections:

```bash
cargo run -p xtask -- gen-mermaid-reference
```

This command owns `crates/merman-core/src/generated/mermaid_reference.rs`,
`crates/xtask/src/generated/mermaid_reference.rs`, and
`playground/src/generated/mermaid-reference.ts`. `crates/merman-core/src/baseline.rs` intentionally
re-exports the generated constants; verify that its public baseline still resolves to the new tag,
version, and suffix instead of hand-editing a second version source.

Never hand-edit generated Rust, TypeScript, runtime-version, provenance, or package projections.
Verify the clean-clone graph:

```bash
cargo run -p xtask -- verify-mermaid-reference
```

Materialize behavior-owning sources under ignored `repo-ref/` paths at descriptor-pinned commits.
Verify registry identity, archive integrity, publish provenance, and source commit before extraction
or execution. Install package graphs with lifecycle scripts disabled. Any necessary lifecycle action
requires source review and an explicit audited allowlist.

After source checkouts and scriptless installs exist, verify the executable graph as well:

```bash
cargo run -p xtask -- verify-mermaid-reference --materialized
```

Also prove both installed JavaScript graphs and the public Rust baseline:

```bash
npm ci --ignore-scripts --prefix playground
npm ls --all --prefix playground
npm ci --ignore-scripts --prefix tools/mermaid-cli
npm ls --all --prefix tools/mermaid-cli
cargo nextest run -p merman-core baseline
```

Only after reviewed browser/reference evidence demonstrates that the new graph produced the intended
baselines may provenance be re-attested:

```bash
cargo run -p xtask -- gen-mermaid-reference --refresh-provenance
cargo run -p xtask -- verify-mermaid-reference --materialized
```

If the release changes a companion selected through an override, the override and its lock entry are
part of the behavior graph. Removing or changing the override without regenerating and materially
verifying both workspaces is an incomplete upgrade.

## 4. Inventory Every Behavior Delta

Diff registration tables and behavior-owning sources, not only changelogs. Assign one disposition
and owner to every added, removed, or changed:

- diagram family, alias, external diagram, and layout module;
- grammar, preprocessing transform, recovery rule, semantic model, and resource limit;
- config, theme, security, URL, sanitizer, viewport, DOM, and generated-id behavior;
- lexical fact, diagnostic, completion, hover, symbol, rename, reference, and semantic token;
- fixture, example, browser requirement, package lock, runtime label, and provenance record.

Text measurement, font rendering, `getBBox()`, `foreignObject`, RoughJS, and CSS inheritance are
bounded artifact contracts. Keep raw, browser-computed, and export evidence separate. Do not use a
heuristic parser, fixture constant, broad normalizer, or comparator whitelist to hide a delta.

## 5. Admit Through Family Ownership

Parser-only support is incomplete. An admitted built-in family must close detection, grammar,
semantic construction, recovery, config/theme, layout, rendering, resources, editor facts, LSP,
WASM, Playground, examples, fixtures, and provenance. All editor capabilities consume one cached
family parse snapshot.

An external diagram additionally needs an exact plugin graph, typed runtime requirement, cold and
reused registration, isolated execution, a source-observed closed artifact type, strict parent-side
validation, failure recovery, and a family-owned Rust semantic/headless path. A validator failure
may not guess a second output format.

Isolation and security claims require machine-recorded browser observations for every configured
desktop/mobile project. Bind the probe contract, security-critical source hashes, expected and
observed values, pass state, and derived counts into the candidate evidence. Static tests or a prose
matrix are not substitutes. Re-record the evidence after any bound source changes and fail closed
when an observation or source digest is absent.

A layout module needs typed selection, config propagation, deterministic fallback, shared resource
enforcement, browser registration, and layout evidence. Removed syntax must update recovery and
editor behavior rather than leaving a transport-specific grammar behind.

Port observable behavior required by headless Merman. Do not port incidental JavaScript containers,
framework state, or caches unless they change observable behavior.

## 6. Decide Feature Ownership With Measurements

Diagram count is not a feature boundary. Prefer an existing semantic feature or a typed browser
runtime requirement. Before adding a Cargo feature, collect dependency tree, license, target,
clean-build, Web/Typst size, and public-package evidence. Measure clean builds in an isolated
`CARGO_TARGET_DIR`; do not delete the shared target directory.

Add a feature only for a genuinely optional dependency boundary whose platform, license, build, or
artifact cost cannot fit an existing capability. Record an explicit no-split decision when the
evidence does not justify one.

## 7. Refresh Generated Surfaces And Evidence

Regenerate and verify the reference graph, family catalogs, Playground example catalog, editor token
descriptor, WASM input digest, package locks, runtime labels, and readable status/provenance. Every
supported family should have source-backed positive and recovery examples; public transport changes
must switch LSP, WASM, Monaco, and the unpublished VS Code extension atomically.

Run focused parser, semantic, renderer, editor, and browser tests first. Then run the applicable
repository gates:

```bash
cargo run -p xtask -- verify-mermaid-reference --materialized
cargo run -p xtask -- verify-editor-token-descriptor
cargo run -p xtask -- verify-playground-example-catalog
cargo run -p xtask -- verify-web-diagram-catalog
cargo run -p xtask -- check-alignment
cargo run -p xtask -- verify --strict
cargo run -p xtask -- wasm-size-matrix --budget-file docs/release/WASM_SIZE_BUDGETS.json
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

The strict command owns Web contracts/build/smoke/prepack, Playground unit/lint/build and
desktop/mobile Chromium, VS Code package tests, generated contracts, and all release DOM modes.
Also prove package locks with `npm ls --all`, run reference CLI and LSP tests, and run any additional
platform or extension-host matrices affected by the change. A stale generated or WASM artifact must
fail before the UI starts.

## 8. Hand Off Without Hidden Residuals

Update the relevant ADRs, family/editor alignment records, package surfaces, Playground design,
generated status, and provenance. Report selected and rejected companions, admitted and removed
capabilities, feature evidence, artifact residuals, commands and results, and environment-only
skips. Keep external delivery as a separate explicitly authorized action.
