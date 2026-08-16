---
title: "Tree-sitter Mermaid Distribution Simplification and Release Plan"
type: "refactor"
date: "2026-08-16"
artifact_contract: "ce-unified-plan/v1"
artifact_readiness: "implementation-ready"
product_contract_source: "ce-plan-bootstrap"
execution: "code"
origin_plan: "docs/plans/2026-08-14-001-feat-tree-sitter-mermaid-language-plan.md"
---

# Tree-sitter Mermaid Distribution Simplification and Release Plan

## Goal Capsule

- Objective: retain the mature 35-family Tree-sitter grammar while replacing its research-grade
  evidence platform with a small, conventional language-package surface that can be published to
  crates.io and npm, distributed as language WASM, and adopted by editors without making
  Tree-sitter a second Merman semantic engine.
- Semantic authority: `merman-core` remains the only owner of strict validity, semantic models,
  diagnostics, IR, rendering, navigation identity, and refactoring safety. Tree-sitter owns only a
  tolerant incremental CST, recovery behavior, generated parser artifacts, and editor queries.
- Maintenance bar: one standard grammar corpus, one Merman-to-Tree-sitter full-fixture oracle, a
  focused incremental/scanner suite, query compilation smoke, and one representative smoke per
  shipped binding. Do not create another fixture DSL, proof receipt, support lattice, or runtime
  matrix.
- Distribution target: publishable Rust and npm packages named `tree-sitter-mermaid`, release-built
  native Node prebuilds, a versioned ABI-compatible language WASM asset, a conventional portable C
  build/install surface, and canonical/editor query files.
  Actual registry publication and downstream pull requests require separate credentials or
  repository-owner authorization.
- Playground decision: do not replace the existing Monaco semantic-token provider. A future
  optional Tree-sitter CST/query inspector may load the same npm-shipped WASM lazily, but the
  default Playground highlighting path remains Merman editor-core.

## Problem Frame

The grammar implementation is no longer the risky part. It has family-local modules, a bounded
external scanner, currently ABI-14 generated artifacts, and structured coverage for all 35 public
Mermaid families. The release refactor will make the ABI a deliberate compatibility decision and
validate migration to the Tree-sitter 0.26.12 default ABI 15. The risk has shifted to
maintainability:

- `crates/xtask/src/cmd/tree_sitter_mermaid.rs` and its admission module exceed 4,000 lines and
  reimplement package identity, evidence, receipt, query, header, legal, and metrics validation.
- `distribution/tree-sitter-mermaid/test/queries/` contains hundreds of generated golden files and
  four profile-specific verification engines.
- header receipts, artifact receipts, metrics receipts, support tiers, schema snapshots, edit-trace
  JSON, and repeated all-family binding tests prove overlapping facts.
- the Cargo crate currently packages most of this internal evidence, while Cargo and npm
  publication are intentionally disabled.
- the package advertises Rust, Node, C, and WASM behavior, but `tree-sitter.json`, npm exports,
  package contents, and browser-facing WASM semantics do not describe one conventional product.

The refactor therefore removes validation infrastructure rather than grammar structure. Maturity is
measured by real inputs and consumer loading, not by the number of self-referential evidence files.

## Product Contract

### Requirements

- R1. No production Merman crate may depend on `tree-sitter`, `tree-sitter-language`, or
  `tree-sitter-mermaid`.
- R2. Every repository fixture that Merman accepts with the applicable render context and strict
  parse must select the expected public-family Tree-sitter root without `ERROR`, missing nodes, or
  broad recovery nodes.
- R3. Merman-invalid fixtures are not required to fail Tree-sitter. Syntax-recovery expectations
  remain in focused Tree-sitter corpus/adversarial cases.
- R4. Standard Tree-sitter corpus files are the grammar and recovery golden authority. Do not
  duplicate those trees in Rust, Node, WASM, editor, or metadata fixtures.
- R5. Incremental tests cover header-family replacement, one ordinary non-scanner edit, and the
  external-scanner mechanics that can diverge after serialization/restart. Scanner tests retain
  maximum, overflow, corrupt-state reset, and representative family-switch cases.
- R6. Canonical queries compile against the generated language. Canonical highlights execute once
  for every public family using the small family/root fixture table and produce at least the
  expected header/family capture; injections, locals, and tags use a few genuinely applicable
  representative sources. Neovim, Helix, and Zed query files remain pre-1.0 adoption assets, but
  per-family/per-surface applicability matrices and exact capture forests are deleted.
- R7. Rust, Node, C, and WASM smoke tests each install or load the shipped artifact and parse one
  representative Mermaid source. They do not rerun the 35-family grammar suite.
- R8. Generation uses the pinned package-local Tree-sitter CLI and explicitly targets ABI 15 for
  both native parser and language WASM. Parser freshness and WASM freshness are separate checks;
  WASM compilation is not repeated during every native grammar check. Changing ABI again requires
  a versioned compatibility decision and Rust, Node, C, WASM, Neovim, Helix, and Zed load smokes.
- R9. The Rust crate is crates.io-publishable and contains only build inputs, generated parser
  sources, canonical queries, licenses, and required documentation.
- R10. The npm package is publishable and contains the Node binding and release-built prebuilds,
  grammar source, generated parser sources, query assets, legal material, and a root-level
  `tree-sitter-mermaid.wasm` export. It does not ship internal tests, oracle dependencies, metrics,
  or evidence receipts.
- R11. `tree-sitter.json` truthfully declares the bindings that are maintained by this package and
  points at canonical query locations.
- R12. Package versions stay aligned across Cargo, npm, and `tree-sitter.json`; the exact selected
  Mermaid/ZenUML and Tree-sitter ABI/toolchain identities live in one small compatibility manifest
  or release document rather than multiple digest graphs.
- R13. crates.io, npm, and GitHub releases are the initial publication channels. One protected,
  subdirectory-aware release job stages the crate, npm tarball with native prebuilds, grammar-only
  source archive, language WASM, checksums, and provenance from one candidate build before any
  authorized publish step. Python, Go, Swift, and a dedicated TypeScript runtime are outside the
  initial release.
- R14. The npm package includes TypeScript declarations for the Node binding. Browser consumers use
  `web-tree-sitter` plus the shipped `.wasm`; they do not need a second TypeScript parser wrapper.
- R15. Playground integration remains optional and lazy. The existing Merman semantic-token path
  remains the default because it already supplies semantic highlighting and shares authority with
  rendering/LSP behavior.
- R16. The C product is a portable source distribution with standard Make, CMake, install, and
  pkg-config entry points. Its package smoke configures, builds, installs, and links from the staged
  grammar-only source archive; no platform-specific C binary package is promised.
- R17. Pre-1.0 semver treats named-node/field removals, canonical capture removals, ABI changes, and
  supported Mermaid-baseline changes as minor releases; compatible fixes and additive query
  captures are patch releases. Release tags use `tree-sitter-mermaid-vX.Y.Z` and Cargo, npm, and
  `tree-sitter.json` versions must match.
- R18. Tree-sitter CI ownership includes grammar/query/binding/package paths plus public Mermaid
  fixtures, render-context rules, family catalogs, and pinned Mermaid/ZenUML baseline changes used
  by the one-way oracle.
- R19. Keep only a wide parser-table/WASM-size gross-regression ceiling from generation summary and
  one bounded scheduled fuzz lane. Neither recreates timing/RSS receipts or blocks ordinary release
  candidates on noisy performance measurements.

### Acceptance Examples

- AE1. A strict-valid fixture under `fixtures/sequence/` is parsed by Merman, mapped through the
  existing family catalog, then parsed by Tree-sitter as exactly one `sequence_diagram` with no
  error or broad recovery node.
- AE2. A parser-only or intentionally invalid fixture rejected by strict Merman parsing is skipped
  by the full-fixture oracle; its Tree-sitter recovery is owned by a focused corpus case if needed.
- AE3. Replacing `flowchart TD` with `sequenceDiagram` on an edited old tree produces the same named
  CST as a fresh parse and leaves no Flowchart root.
- AE4. `cargo package -p tree-sitter-mermaid --list` contains no `test/`, `tests/`, header-oracle,
  metrics, receipt, npm lockfile, or language-WASM payload.
- AE5. `npm pack ./distribution/tree-sitter-mermaid --dry-run --json` contains the Node binding and
  staged prebuilds, grammar/query sources, generated parser, legal files, and root
  `tree-sitter-mermaid.wasm`, but no
  test matrices, build directories, or oracle dependencies.
- AE6. A clean Rust consumer loads `tree_sitter_mermaid::LANGUAGE`; a clean Node consumer loads the
  package root without a local compiler on a supported release platform; a C smoke installs and
  links the staged source archive; and `web-tree-sitter` loads the npm-shipped WASM. Each parses one
  Flowchart fixture.
- AE7. The Playground continues to obtain semantic tokens from `@mermanjs/web`/editor-core without
  adding `web-tree-sitter` or the language WASM to its default bundle.

## Key Technical Decisions

### KTD1. Keep two parsers but only one semantic authority

Tree-sitter and Merman intentionally parse the same source for different products. This is not two
semantic implementations: Tree-sitter produces a tolerant CST for editors; Merman produces strict
semantics and rendering behavior. Mermaid upgrades therefore require grammar maintenance in two
places, but the existing Merman fixture corpus makes the Tree-sitter side a one-way compatibility
projection instead of an independently specified language.

### KTD2. Use existing test engines only

Use `tree-sitter test`, `cargo nextest`, `node:test`, and existing package tooling. Delete receipt
producers, matrix verifiers, metrics runners, and metadata contracts whose only purpose is to prove
other generated evidence. The full-fixture oracle belongs in the Rust integration tests because it
already needs `merman-core`, render-context lookup, and the Rust Tree-sitter runtime.

### KTD3. Prefer canonical queries plus thin adoption adapters

The package's stable query contract is the canonical query set referenced by `tree-sitter.json`.
Retain the current Neovim, Helix, and Zed real loader query files as pre-1.0 adoption assets, but
delete their applicability matrices, capture forests, pinned-editor downloads, and custom profile
verifiers. Their long-term compatibility is ultimately owned by downstream integrations. Compile
them locally; do not maintain a 35 x 9 x profile proof matrix.

### KTD4. Publish one npm package, not a separate browser SDK

The standard npm grammar package serves two roles: a source-built Node binding and an asset carrier
for grammar/query/WASM files. Node receives `index.d.ts`; browser consumers pair the exported WASM
asset with `web-tree-sitter`. A separate TS package would duplicate identity and versioning without
adding parser capability.

### KTD5. Adopt ABI 15 and keep WASM separate from the native freshness loop

Tree-sitter 0.26.12 defaults to ABI 15, and the target editor/runtime set accepts it. Use ABI 15
explicitly so official tooling cannot silently choose the compatibility contract for us. Native
parser generation is fast and runs on ordinary grammar changes. WASM generation is a Linux
release/CI lane with its own freshness and load smoke. This preserves browser distribution without
making every local check download or invoke a WASI SDK.

### KTD6. Build release products once and keep C source-native

The protected release workflow stages one candidate set, then promotes those exact files to
crates.io, npm, and GitHub only when separately authorized. Standard checksums and CI provenance are
enough; do not rebuild a custom receipt graph. C consumers receive generated source plus standard
Make/CMake/pkg-config installation, not another binary matrix.

## Implementation Units

### U1. Replace the proof platform with a thin conformance boundary

**Files:**

- `distribution/tree-sitter-mermaid/tests/conformance.rs`
- `distribution/tree-sitter-mermaid/tests/incremental.rs`
- `distribution/tree-sitter-mermaid/tests/scanner_protocol.rs`
- `distribution/tree-sitter-mermaid/tests/adversarial.rs`
- `distribution/tree-sitter-mermaid/tests/queries.rs`
- `distribution/tree-sitter-mermaid/Cargo.toml`
- `crates/xtask/src/cmd/mod.rs`
- `crates/xtask/src/main.rs`
- `crates/xtask/Cargo.toml`
- `crates/xtask/src/cmd/tree_sitter_mermaid.rs`
- `crates/xtask/src/cmd/tree_sitter_mermaid/`
- `distribution/tree-sitter-mermaid/metadata/`
- `distribution/tree-sitter-mermaid/test/conformance/`
- `distribution/tree-sitter-mermaid/test/edits/`
- `distribution/tree-sitter-mermaid/test/schema/`
- `distribution/tree-sitter-mermaid/test/queries/`
- `distribution/tree-sitter-mermaid/tests/{header_dispatch,mechanics_metrics,node_schema,u3_semantics,u4_semantics,u5_semantics,u6_semantics,u7_semantics}.rs`

**Work:**

- make `conformance.rs` enumerate active repository `.mmd` fixtures with the existing fixture scanner
  rules, apply render context, run strict Merman parsing, and assert Tree-sitter root/error/recovery
  behavior only for strict-valid inputs;
- retain one small public-family/root fixture table for expected Tree-sitter roots and binding/query
  representative sources;
- replace JSON edit-trace loading with focused direct Rust edit cases;
- keep scanner serialization coverage proportional to state risk;
- make the lean query test run canonical highlights once per public family, while keeping only a
  handful of representative non-highlight query cases;
- remove the dedicated xtask command, support lattice, receipts, header oracle/evidence, metrics
  metadata, schema snapshots, staged semantic-era tests, and query applicability/golden forests.

**Test scenarios:** AE1-AE3; unknown/private/deferred fixture directories are excluded; a deliberate
wrong root or injected broad recovery node fails with the fixture path; all 35 public IDs are present
in the small root table and match the Merman catalog.

### U2. Normalize bindings, queries, and public package APIs

**Files:**

- `distribution/tree-sitter-mermaid/tree-sitter.json`
- `distribution/tree-sitter-mermaid/package.json`
- `distribution/tree-sitter-mermaid/Cargo.toml`
- `distribution/tree-sitter-mermaid/bindings/rust/{lib.rs,build.rs}`
- `distribution/tree-sitter-mermaid/bindings/node/{index.js,index.d.ts,binding.cc,binding_test.js}`
- `distribution/tree-sitter-mermaid/bindings/c/`
- `distribution/tree-sitter-mermaid/queries/`
- `distribution/tree-sitter-mermaid/wasm/`
- `distribution/tree-sitter-mermaid/README.md`

**Work:**

- expose the conventional Rust `LANGUAGE`, `NODE_TYPES`, and canonical query string constants;
- make the Node entry match the standard language binding shape and remove runtime receipt/query
  profile APIs;
- declare maintained bindings truthfully in `tree-sitter.json`;
- make canonical query paths first-class and retain exactly the Neovim, Helix, and Zed loader query
  files as pre-1.0 adoption assets;
- expose the committed language WASM as the root npm asset/export rather than a Node-only
  pseudo-browser binding;
- add standard package-root Make/CMake/install/pkg-config files for the portable C surface;
- tighten Cargo/npm file allowlists and enable packaging for the unscoped `tree-sitter-mermaid`
  identities without performing publication.

**Test scenarios:** AE4-AE6; package versions match; Node TypeScript declarations match the runtime
shape; query files compile; npm consumers can resolve the WASM asset without installing the
generator.

### U3. Simplify generation, CI, and release operations

**Files:**

- `distribution/tree-sitter-mermaid/scripts/generate.py`
- `distribution/tree-sitter-mermaid/scripts/package_smoke.py`
- `distribution/tree-sitter-mermaid/scripts/`
- `.github/workflows/tree-sitter-mermaid.yml`
- `.github/workflows/release-independent-crate.yml`
- `scripts/test_release_workflow_security.py`
- `docs/development/TREE_SITTER_MERMAID.md`
- `docs/release/TREE_SITTER_MERMAID.md`
- `docs/release/MERMAID_UPGRADE_PLAYBOOK.md`
- `docs/release/PUBLISH_ORDER.md`
- `docs/adr/0082-tree-sitter-language-boundary.md`

**Work:**

- reduce native generation to one pinned explicit-ABI-15 generation plus committed-artifact
  comparison;
- separate WASM generation/freshness from native generation;
- reduce package smoke to one representative source per shipped channel;
- add a protected subdirectory-aware release workflow that builds once, stages crate/npm/source/WASM
  products, prebuilds, checksums, and provenance, and keeps publish jobs authorization-gated;
- make CI run one grammar corpus lane, one Rust integration lane, one Node/package lane, and one
  WASM lane without nested duplicate commands;
- include public fixtures, render-context rules, family catalogs, and pinned baseline metadata in the
  Tree-sitter CI path owner;
- retain only a wide generated table/WASM size ceiling and a scheduled bounded fuzz job;
- update release and upgrade docs to describe crates.io, npm, WASM, editor adoption, and the lean
  Merman fixture oracle, including concrete Neovim `location`, Helix `subpath`, and Zed `path/rev`
  examples;
- amend ADR-0082 to remove the evidence lattice/receipt architecture while preserving the semantic
  boundary.

**Test scenarios:** generation drift and accidental ABI drift are detected; WASM load is proven
independently; clean package contents match allowlists; staged release products share one version
and candidate build; the CI path selector selects the Tree-sitter owner for grammar, query, binding,
package, fixture, render-context, family-catalog, and baseline changes.

### U4. Regenerate, verify, and commit reviewable units

**Files:** generated parser/JSON/header/WASM artifacts, `Cargo.lock`, package lockfiles, legal
projections, and any CI/release projections required by U1-U3.

**Work:**

- regenerate only after source and package layouts settle;
- refresh locks and generated legal material through their repository-owned tools;
- commit plan/research, conformance simplification, and distribution/release standardization as
  separate Conventional Commits;
- leave registry publication and downstream repository mutations unperformed.

**Validation:**

- `cargo fmt --all -- --check`
- `cargo nextest run --locked -p tree-sitter-mermaid --no-fail-fast`
- `cargo clippy --locked -p tree-sitter-mermaid --all-targets -- -D warnings`
- package-local `tree-sitter test`
- native generated-artifact freshness check
- Node binding smoke
- language-WASM smoke
- `cargo package --locked -p tree-sitter-mermaid --list`
- `npm pack ./distribution/tree-sitter-mermaid --dry-run --json`
- repository legal/package projection checks affected by the changed manifests
- `git diff --check`

## Sequencing and Dependencies

1. U1 first, because deleting duplicate authorities determines the public package contents and
   prevents new work from binding itself to receipts that will disappear.
2. U2 second, because package APIs and query locations define generation inputs and consumer smoke.
3. U3 third, after the final public surface is known.
4. U4 last; generated parser/WASM, locks, and legal reports are refreshed once per settled source
   shape rather than after every intermediate edit.

## Risks and Mitigations

- **Full-fixture failures expose real grammar gaps.** Fix family grammar or add a narrowly justified
  core-invalid classification; do not add generic recovery or an exclusion manifest.
- **Large parser tables remain expensive.** Preserve the current measured table unless real editor
  loading or incremental measurements justify another grammar refactor. Do not reintroduce complex
  metrics gates solely to defend an arbitrary threshold.
- **Downstream editors use different query conventions.** Keep canonical queries stable and treat
  adapter files as pre-1.0 adoption aids; downstream PRs can own their final placement.
- **Unscoped registry names may be claimed before release.** Recheck npm/crates.io immediately before
  publication. If npm is unavailable, use a scoped npm fallback without renaming the Rust crate or
  language symbol.
- **Playground duplication.** Do not add Tree-sitter to the default Monaco worker. Any future
  experiment must demonstrate a distinct user-facing capability such as CST inspection or
  offline query debugging.

## Sources

- `docs/plans/2026-08-14-001-feat-tree-sitter-mermaid-language-plan.md`
- `docs/adr/0082-tree-sitter-language-boundary.md`
- Tree-sitter parser configuration: https://tree-sitter.github.io/tree-sitter/cli/init.html
- Tree-sitter query model: https://tree-sitter.github.io/tree-sitter/3-syntax-highlighting.html
- Tree-sitter CLI WASM build: https://tree-sitter.github.io/tree-sitter/cli/build.html
- Official `tree-sitter-json` package layout:
  https://github.com/tree-sitter/tree-sitter-json
- Official grammar release workflow:
  https://github.com/tree-sitter/workflows
- `web-tree-sitter` runtime guidance:
  https://github.com/tree-sitter/tree-sitter/tree/master/lib/binding_web
