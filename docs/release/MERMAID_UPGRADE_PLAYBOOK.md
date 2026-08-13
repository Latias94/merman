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
`tools/upstreams/MERMAID_REFERENCE_BUNDLE.json`, `tools/upstreams/MERMAID_SELECTION_DECISION.json`,
`tools/upstreams/REPOS.lock.json`, relevant ADRs, and package-surface documentation before changing
the graph.

## 2. Discover Candidates Through Manual Admission

The standing bundle names exactly one selected package/source/runtime graph. It does not retain an
oracle, candidate, deferred-major, browser-result, or supply-chain-attestation graph. Ordinary CI,
Pages, and release verification must remain valid when no candidate files exist.

During a deliberate upgrade admission, evaluate these temporary identities for Mermaid and each
external diagram or layout dependency:

- **selected baseline**: the exact package and source already selected by the bundle;
- **latest-compatible candidate**: the highest stable version satisfying the host package's
  published range;
- **latest-stable delta**: a newer stable version outside that range.

A compatible range establishes candidacy, not behavioral compatibility. Use the read-only manual
Mermaid upgrade admission workflow to materialize the selected and candidate packages in temporary
directories, run the exact official command
`npm audit signatures --json --include-attestations --registry=https://registry.npmjs.org/`, and
compare the named parser, renderer, security, resource, corpus, host-integration, and browser probe
contracts. The admission code may reject a nonzero npm result, but it must not implement its own
DSSE, SLSA, certificate, or signature verifier.

Select a candidate only when every explained result passes. Retain the selected baseline when it
does not. Treat an outside-range major as separately scoped behavior-port work rather than silently
selecting it because it is newer. Admission reports and raw official-tool output remain workflow
artifacts; completed or rejected candidate/deferred graphs are not committed as standing inputs.

## 3. Generate And Materialize One Reference Graph

Move the source graph and all reference workspaces together. Before running the generator:

1. Update `tools/upstreams/MERMAID_REFERENCE_BUNDLE.json` with exact selected package versions,
   integrity, tarball URLs, source tags, commits, the ordered built-in diagram/default-layout
   registry inventory and source hashes, runtime registration hashes, selected companions, and the
   expected Playground/reference-CLI lock hashes. The bundle stores only the selection receipt path
   and SHA-256; it does not embed admission evidence. Re-extract the inventory from the pinned
   Mermaid checkout; do not infer it from Merman's local catalog or temporary admission reports.
2. Update every selected source checkout and commit in `tools/upstreams/REPOS.lock.json`. A bundle
   source with a `checkoutPath` must have the same repository and commit in the lock; the lock must
   not retain the previous Mermaid tag as an active source.
3. Update `playground/package.json` and `tools/mermaid-cli/package.json`, including their exact
   `overrides`, then regenerate both package locks with lifecycle scripts disabled. The reference
   CLI's direct Mermaid, CLI, layout, external-diagram, and behavior-source versions must resolve to
   the selected graph rather than whatever an upstream range happens to install.
4. Recompute the descriptor's workspace hashes from those reviewed manifests, locks, and reference
   config. Record `installedContentSha256` for every package that can participate in reference
   execution: Mermaid, the parser, the sanitizer, the reference CLI, every external diagram and
   layout module, each selected behavior package, and the complete browser-driver toolchain loaded
   by the renderer. Do not copy old hashes forward.

When the selected identity changes, write a new `MERMAID_SELECTION_DECISION.json` from the reviewed
admission outputs. Its previous/current identity digests, exact changed fields, npm version and
official command, verified package/version/integrity records, behavior result, and raw-output digest
must describe the transition from the trusted base. Do not include the receipt itself in the
selection identity. A bootstrap receipt is reserved for migrating an already-reviewed historical
selection: it must identify the historical Git object and aggregate digest and must not claim that
unarchived official npm stdout was reconstructed.

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
cargo run -p xtask -- verify-mermaid-reference --base <trusted-base-sha>
```

The base-aware form reads the trusted bundle through `git show`. A selected identity change must
match the receipt's previous/current identities and exact changes. An unchanged identity may not
replace its receipt, except for the one explicitly bound bootstrap migration. The bootstrap's
historical evidence commit must be an ancestor of the trusted base, and its Git object digest must
still match.

Materialize behavior-owning sources under ignored `repo-ref/` paths at descriptor-pinned commits.
Verify registry identity, archive integrity, source commit, and installed bytes before execution.
Official npm signature verification belongs to manual admission, not to the Rust standing verifier.
Install package graphs with lifecycle scripts disabled. Any necessary lifecycle action requires
source review and an explicit audited allowlist.

After source checkouts and scriptless installs exist, verify the executable graph as well. This
extracts the built-in diagram and default-layout registrations from the pinned Mermaid source and
compares their order and IDs with the bundle:

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
baselines may the primary SVG provenance be regenerated:

```bash
cargo run -p xtask -- gen-mermaid-reference --refresh-provenance
cargo run -p xtask -- verify-mermaid-reference --materialized
```

The refresh command performs a fresh render of every primary family. Each family is protected by
the canonical cross-process family lock and committed through the same-directory atomic SVG and
manifest transaction. It cannot relabel an existing SVG corpus with newer source metadata. The
generated provenance manifest records Chromium's resolved locale and timezone in addition to its
exact browser, runtime, operating-system, and font identities. A missing primary-family manifest or
an unhashed materialized companion fails verification closed.

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

Isolation and security claims require machine-recorded browser observations from the manual
admission workflow. Commit the stable probe contract, while the expected/observed values, pass state,
and derived counts remain in the workflow report. Static tests or a prose matrix are not substitutes.
Ordinary source edits belong to unit, integration, and focused browser tests; a new candidate, probe
contract, or claimed observation requires an explicit manual rerun. Mobile interaction emulation
remains an on-demand product check; it does not duplicate the admission matrix.

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
cargo run -p xtask -- verify-editor-language-contract
cargo run -p xtask -- verify-playground-example-catalog
cargo run -p xtask -- verify-web-diagram-catalog
cargo run -p xtask -- check-alignment
cargo run -p xtask -- verify --strict
cargo run -p xtask -- wasm-size-matrix --surface web \
  --web-package-root platforms/web/packages \
  --budget-file docs/release/WASM_SIZE_BUDGETS.json
cargo run -p xtask -- wasm-size-matrix --surface typst \
  --budget-file docs/release/WASM_SIZE_BUDGETS.json
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

The strict command owns Web contracts/build/smoke/prepack, Playground unit/lint/build and desktop
Chromium, focused Firefox/WebKit smoke, VS Code package tests, generated contracts, and all release
DOM modes. Run the focused mobile command when the change affects compact navigation, dialogs,
viewport gestures, or visual viewport behavior. Also prove package locks with `npm ls --all`, run
reference CLI and LSP tests, and run any additional platform or extension-host matrices affected by
the change. A stale generated or WASM artifact must fail before the UI starts.

## 8. Hand Off Without Hidden Residuals

Update the relevant ADRs, family/editor alignment records, package surfaces, Playground design,
generated status, and provenance. Report selected and rejected companions from the manual workflow,
admitted and removed capabilities, feature evidence, artifact residuals, commands and results, and
environment-only skips. Retain only the reviewed selection receipt in the standing graph; do not
promote rejected or completed candidate reports into live verification inputs. Keep external delivery
as a separate explicitly authorized action.
