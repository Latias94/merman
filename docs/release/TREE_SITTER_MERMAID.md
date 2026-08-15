# Tree-sitter Mermaid Release Readiness

`tree-sitter-mermaid` is currently a dry-run-only package. This document is the local release
preflight for maintainers; it does not authorize publishing to npm, crates.io, editor registries, or
an external repository.

## Release boundary

- Package root: `distribution/tree-sitter-mermaid`
- Package version: `0.1.0`
- Language symbol: `mermaid`
- Generated language ABI: 14
- Tree-sitter CLI/Rust/web runtime: `0.26.12`
- Source-built Node runtime tested by consumers: `0.25.1`
- Support target: 35 public Merman families at `conformant`
- Semantic boundary: Merman parsers and IR remain authoritative; the Tree-sitter package owns only
  tolerant CST, queries, bindings, WASM, metadata, and release dry-runs.

## Required dry-run sequence

Run from the repository root unless a command specifies `--prefix`:

```console
npm ci --prefix distribution/tree-sitter-mermaid
npm run generate:check --prefix distribution/tree-sitter-mermaid
npm run metrics:check --prefix distribution/tree-sitter-mermaid
npm test --prefix distribution/tree-sitter-mermaid
npm run test:queries --prefix distribution/tree-sitter-mermaid
npm run build:wasm --prefix distribution/tree-sitter-mermaid
npm run test:wasm --prefix distribution/tree-sitter-mermaid
npm pack ./distribution/tree-sitter-mermaid --dry-run --json
```

Repository and robustness gates:

```console
cargo fmt --all -- --check
cargo nextest run --locked -p tree-sitter-mermaid --no-fail-fast
cargo nextest run --locked -p xtask --no-fail-fast
cargo run --locked -p xtask -- verify-tree-sitter-mermaid
cargo clippy --locked -p tree-sitter-mermaid -p xtask --all-targets -- -D warnings
python3 -m unittest scripts.test_ci_plan scripts.test_audit_plan
git diff --check
npm run test:corpus --prefix distribution/tree-sitter-mermaid
npm run test:incremental --prefix distribution/tree-sitter-mermaid
npm run test:downstream --prefix distribution/tree-sitter-mermaid
npm run test:adversarial --prefix distribution/tree-sitter-mermaid
npm run fuzz:regression --prefix distribution/tree-sitter-mermaid
cargo nextest run --locked -p tree-sitter-mermaid --test conformance --test incremental --test queries --test adversarial
cargo run --locked -p xtask -- verify-tree-sitter-mermaid --all-fixtures
```

Package and legal gates:

```console
cargo package --locked -p tree-sitter-mermaid --list
cargo package --locked -p tree-sitter-mermaid
python3 scripts/verify_crate_package_legal_materials.py
python3 scripts/verify_artifact_dependency_closures.py
cargo run --locked -p xtask -- verify --strict
npm run test:package-smoke --prefix distribution/tree-sitter-mermaid
```

Final whole-repository confidence gates:

```console
cargo nextest run --locked --workspace --no-fail-fast
cargo test --locked --workspace --doc
```

The package smoke installs the npm tarball into a clean Node consumer and extracts the Cargo crate
into a clean Rust/C consumer. It verifies Node, language-WASM, Rust, and C loading from committed
artifacts without installing `tree-sitter-cli` in the consumer.

## Package contents

The npm tarball must include only source package material, generated artifacts, query profiles,
metadata, and legal files needed by consumers. The Cargo crate must include the Rust/C build inputs
and the same generated artifacts. Both packages must exclude build products and install state such
as `.git`, `build`, `node_modules`, `target`, and `scripts/header-oracle/node_modules`.

Minimum required package material:

- `LICENSE`, `THIRD_PARTY_NOTICES.md`, and `THIRD_PARTY_LICENSES/**`
- `metadata/artifact-receipt.json`, `metadata/support.json`, and family/root fixtures
- `grammar.js`, `grammar/**`, `tree-sitter.json`, `src/parser.c`, `src/scanner.c`,
  `src/node-types.json`, and generated Tree-sitter headers
- `bindings/c/**`, `bindings/rust/**`, `bindings/node/**`, and `bindings/wasm/**`
- `queries/{portable,neovim,helix,zed}/**`
- `wasm/tree-sitter-mermaid.wasm`

The generator itself is not a runtime dependency. Consumers load committed C, committed WASM, or
source-built Node bindings from package contents.

## Deletion inventory

The following transitional implementation classes must remain absent before release readiness is
claimed:

| Transitional item | Final owner | Verification |
| --- | --- | --- |
| Broad family bodies named `unstructured_body`, `catch_all_body`, `raw_line`, or generic `unknown_statement` | Family-local named statements and named malformed recovery nodes | `cargo nextest run --locked -p tree-sitter-mermaid --test conformance --test adversarial` |
| Duplicate support/header manifests outside `metadata/support.json`, `metadata/headers.json`, and receipt-bound evidence files | Package metadata and generated receipts | `cargo run --locked -p xtask -- verify-tree-sitter-mermaid` |
| Stale generated parser, JSON, header, or WASM files | Transactional package generator | `npm run generate:check --prefix distribution/tree-sitter-mermaid` |
| Mechanics-spike one-off scripts or unchecked metrics fields | `scripts/mechanics_gate.js` plus `metadata/metrics/u2-mechanics.json` | `npm run metrics:check --prefix distribution/tree-sitter-mermaid` |
| Consumer tests that require `tree-sitter-cli` after install | Source-free package smoke | `npm run test:package-smoke --prefix distribution/tree-sitter-mermaid` |

Retained family-local recovery token names are acceptable only when they are tested as malformed
input recovery and cannot make valid structured source pass a tier gate through an opaque body.

## Downstream pin rule

Zed consumes grammars by Git commit. A generated parser change requires this sequence:

1. Commit the generated parser and query files.
2. Update `distribution/tree-sitter-mermaid/test/downstream/zed/extension.toml` to that commit.
3. Run `npm run test:downstream --prefix distribution/tree-sitter-mermaid`.

Do not pin Zed to the current working tree or to a commit whose `src/parser.c` bytes differ from the
checked-out parser.

## Publish blocker

Publication remains blocked until a separate release decision assigns registry ownership, package
names, credentials, dist-tags, and downstream submission policy. Passing this preflight only proves
that the local package is ready for review as a dry-run artifact.

