---
name: align-mermaid-release
description: Align Merman to a selected Mermaid release and its companion behavior graph. Use when changing the pinned Mermaid baseline, evaluating compatible or latest companion packages, admitting added or removed diagrams and layouts, porting upstream parser/semantic/editor/LSP/render behavior, updating Playground or reference CLI integration, or deciding whether release dependencies justify a Cargo feature split.
---

# Align Mermaid Release

Treat a Mermaid release as a versioned behavior graph, not a package number. Preserve unrelated
work, keep generated projections derived from repository descriptors, and close every changed
capability across all Merman surfaces.

## Read the Authority

From the repository root, read:

- `docs/release/MERMAID_UPGRADE_PLAYBOOK.md`;
- `tools/upstreams/REPOS.lock.json` and `tools/upstreams/README.md`;
- `docs/FEATURES.md` and `docs/release/PACKAGE_SURFACES.md`;
- the selected Mermaid reference bundle and its digest-bound selection decision receipt located by
  the repository `xtask` help and verifier;
- relevant family alignment notes and architecture decisions;
- [the admission checklist](references/admission-checklist.md) in full.

Record the selected Mermaid release, exact tag and commit, current selection identity digest,
receipt digest, current working-tree state, available selected checkouts, trusted Git base, and
requested delivery boundary before changing files. If no release was selected, finish discovery
and ask for that decision before mutating the reference graph.

Completion criterion: the selected source identity, current descriptor, dirty-tree ownership, and
delivery boundary are explicit.

## Establish the Reference Graph

Use the repository generator named by the selected bundle and exposed by `xtask` help; do not
hand-edit generated projections or invent a second updater. The standing bundle contains only the
selected package/source/runtime graph and the path plus SHA-256 of its decision receipt. It must not
contain oracle, candidate, deferred-major, browser-result, or attestation payloads. Then run:

```bash
cargo run -p xtask -- verify-mermaid-reference --materialized
cargo run -p xtask -- verify-mermaid-reference --base <trusted-base-sha>
```

Use the manual Mermaid upgrade admission workflow to resolve three facts for every changed Mermaid
companion:

1. **Oracle**: the exact package, source commit, and integrity selected by the Mermaid workspace.
2. **Latest-compatible candidate**: the highest stable release satisfying the host plugin's
   published dependency range.
3. **Latest-stable delta**: a newer stable release outside that range, reported separately and
   never substituted implicitly.

Treat semver as candidacy, not behavioral proof. Materialize candidates in temporary state, invoke
the exact official npm command
`npm audit signatures --json --include-attestations --registry=https://registry.npmjs.org/`, and
retain its raw output only in the admission report. Select a Latest-compatible candidate only after
its parser, renderer, security, resource, and host-integration matrix has no unexplained delta.
Fail admission when the candidate cannot prove compatibility. Admit a Latest-stable delta outside
the range only as separately scoped behavior-port work; do not commit a deferred-major placeholder
to keep the current graph valid.

Completion criterion: the bundle and locks name one reproducible selected graph; workflow reports
give every evaluated candidate an admitted, rejected, or separately-scoped result; and a selection
change has one base-bound receipt whose previous/current identities and changed fields are exact.

## Materialize Sources Safely

Materialize selected Mermaid and behavior-owning companion sources under `repo-ref/` at
bundle-pinned commits. Acquire candidate registry metadata and tarballs only in temporary admission
state with lifecycle scripts disabled. Before extracting or executing package code, use official
package-manager verification for package identity, version, archive integrity, publish provenance,
and source identity; do not interpret DSSE, in-toto, SLSA, certificates, or transparency logs in
repository Rust code.

Treat lifecycle scripts as untrusted input. If a source package genuinely requires an install or
build action for evidence, inspect the script and dependencies first, record why source inspection
or an existing repository harness is insufficient, and add only the audited action to an explicit
allowlist. Keep downloaded checkouts and caches out of commits.

For retained Mermaid Cypress evidence, use the executable collector in
`tools/upstreams/cypress-collector/`. The repository does not maintain a Rust JavaScript evaluator:
the collector must execute the pinned Mermaid runtime and transpilation path. Install the pinned
checkout with lifecycle scripts disabled, run the collector through the exact Node and pnpm
versions declared by the collector scope catalog, and collect both retained scopes:

```bash
cd repo-ref/mermaid
npx --yes --package=node@22.14.0 --package=pnpm@10.30.3 -- \
  pnpm install --frozen-lockfile --ignore-scripts
cd ../..

npx --yes --package=node@22.14.0 --package=pnpm@10.30.3 -- \
  node tools/upstreams/cypress-collector/collect.mjs \
  --scope new-family \
  --output target/upstream-cypress-new-family.json

npx --yes --package=node@22.14.0 --package=pnpm@10.30.3 -- \
  node tools/upstreams/cypress-collector/collect.mjs \
  --scope flowchart-elk \
  --output target/upstream-cypress-flowchart-elk.json
```

Review call additions, removals, skips, runtime effects, and supplemental fixture identities before
projecting either collection. Project reviewed observations with
`project-upstream-cypress-collection --scope <scope> --input <file> --refresh`. Standing Rust checks
must validate only the committed manifests and local digests; they must not execute or statically
interpret upstream JavaScript.

Completion criterion: every selected package has verified source and tarball provenance, admission
reports bind the exact official tool/version/package/integrity/raw-output digest, and no unreviewed
lifecycle action executed.

## Inventory the Delta

Derive the delta from pinned source and generated inventories. Account for every added, removed, or
changed:

- built-in diagram registration and alias;
- external diagram and layout module;
- grammar rule, preprocessing rule, semantic model, config, theme, security, resource limit, and
  DOM contract;
- source-backed fixture, example, generated catalog, package lock, runtime label, and provenance
  record.

Classify browser-dependent text measurement, `getBBox()`, `foreignObject`, font, and hand-drawn
effects as explicit artifact contracts. Keep comparator normalization narrow and non-semantic.
Never use a heuristic parser, magic constant, or broad whitelist to conceal a source behavior
delta.

Completion criterion: every upstream registration and behavior delta has one owner and one
admission disposition; no changed surface is merely absent from the inventory.

## Admit Every Capability

Implement each admitted delta through the family-owned path appropriate to its kind:

- **Built-in diagram**: detection, grammar/preprocessing, semantic construction, validation,
  config/theme, layout, SVG, resource limits, editor facts, LSP features, Web bindings, Playground,
  fixtures, and provenance.
- **External diagram**: typed runtime requirement, exact plugin graph, cold and reused loading,
  semantic/headless behavior that Merman must own, source-observed closed artifact type and strict
  validator, isolated execution when package code is untrusted, Playground and reference CLI
  registration, failure projection, and security tests. A validation failure may not infer a second
  artifact format; a genuinely new format requires separate admission before selection.
- **Layout module**: typed selection and registration, config propagation, deterministic fallback,
  resource enforcement, reference CLI and Playground integration, and source-backed layout parity.
- **Removed or changed syntax**: parser recovery, diagnostics, editor lexemes, semantic tokens,
  completions, hover, symbols, rename/reference behavior, fixtures, and migration documentation.

When a changed syntax path uses a checked-in LALRPOP grammar, treat the `.lalrpop` source and its
generated Rust parser as one atomic projection. Run `cargo run -p xtask -- gen-lalrpop-parsers`;
never hand-edit `crates/merman-core/src/generated/lalrpop/*.rs`; then prove freshness with
`cargo run -p xtask -- verify-lalrpop-parsers`.

Port observable dependency behavior needed by headless Merman; do not port implementation-only data
structures that add no behavior. Parser-only support is incomplete. Keep one parser-derived fact
stream for semantics, editor services, LSP, Monaco, and VS Code rather than adding a regex or
transport-specific grammar. Preserve the repository's declared public ABI and editor/facts schema
constraints unless a separate architecture decision explicitly changes them.

Completion criterion: each admitted capability has source-backed positive, recovery, resource,
security, and cross-surface evidence, and every exposed surface agrees on its support level.

## Decide Feature Boundaries

Prefer the existing semantic capability that already owns the behavior. Diagram count alone is not
a feature boundary, and a browser-only lazy companion is a typed runtime capability rather than an
automatic Cargo feature.

Before adding a feature, record before/after evidence for:

- `cargo tree` on affected packages and feature combinations;
- supported native and WASM targets;
- dependency licenses and distribution obligations;
- clean-build time and cache-independent dependency cost, measured with an isolated
  `CARGO_TARGET_DIR` rather than deleting shared artifacts;
- browser and Typst artifacts against `docs/release/WASM_SIZE_BUDGETS.json`;
- every public package surface and build preset affected by the split.

Add a feature only when a genuinely optional dependency has platform, license, clean-build, or
artifact-size cost that cannot fit an existing semantic capability. Record a no-split decision with
the same clarity when the evidence does not justify one.

Completion criterion: the feature decision is evidence-backed, all affected target combinations
compile, and package-surface documentation matches the resulting graph.

## Verify and Hand Off

Start with focused family/parser/editor/browser tests, then run the applicable strict gates:

```bash
cargo run -p xtask -- verify-mermaid-reference
cargo run -p xtask -- verify-playground-example-catalog
cargo run -p xtask -- verify-web-diagram-catalog
cargo run -p xtask -- check-alignment
cargo run -p xtask -- verify-lalrpop-parsers
cargo run -p xtask -- verify --strict
cargo run -p xtask -- wasm-size-matrix --budget-file docs/release/WASM_SIZE_BUDGETS.json
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

The strict command owns release DOM comparisons in `structure`, `parity`, and `parity-root`, Web
contract/build/smoke/prepack, Playground unit/lint/build/browser, and VS Code package gates. Run
reference CLI, LSP, platform, and extension-host matrices whenever those surfaces changed. Run the
WASM size matrix and target build matrix whenever dependency or feature ownership changed. Use
`npm ls --all` in the Playground and reference CLI to prove that materialized companion graphs match
the descriptor. Use `cargo nextest` for Rust tests.

Update the upgrade playbook, relevant ADRs and family/editor records, package surfaces, Playground
design, generated status, and provenance. Report selected versus rejected companions, workflow
report digests, admitted capabilities, residual artifact contracts, feature evidence, commands and
results, and any environment-only skips. Do not promote completed candidate/deferred/attestation
reports into standing inputs. Prepare focused Conventional Commits only when the task authorizes
commits. Treat push, PR creation, package publication, and release as separate authority.

Completion criterion: all checklist rows are closed with reproducible evidence, strict gates pass
without unexplained exceptions, generated files are clean, and the handoff contains no external
delivery action outside the request.

When changing this skill, run the installed `skill-creator` `quick_validate.py` against this skill
directory. Validate release-specific evidence by running the repository commands above against the
selected source graph; do not replace that evidence with prose or source-shape assertions.
