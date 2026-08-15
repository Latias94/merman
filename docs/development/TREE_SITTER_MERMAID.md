# Tree-sitter Mermaid Development

`distribution/tree-sitter-mermaid` is an independently versioned Tree-sitter language package for
Mermaid source. It is adjacent to Merman's semantic parser stack and must stay out of the
production dependency closure for `merman-core`, `merman-analysis`, `merman-editor-core`, and
`merman-lsp`.

The package owns tolerant CST structure, generated Tree-sitter artifacts, query profiles, bindings,
WASM, support metadata, and release dry-runs. Merman's existing parsers remain the authority for
strict validity, semantic models, diagnostics, navigation identity, refactoring safety, and
rendering.

## Local setup

Install the package-local Node toolchain from the repository root:

```console
npm ci --prefix distribution/tree-sitter-mermaid
npm ci --ignore-scripts --prefix distribution/tree-sitter-mermaid/scripts/header-oracle
node distribution/tree-sitter-mermaid/node_modules/tree-sitter-cli/install.js
npm run build:node --prefix distribution/tree-sitter-mermaid
```

The generator is intentionally package-local and pinned. Do not use a globally installed
`tree-sitter` CLI for committed artifacts.

## Regeneration

After changing `grammar.js`, `grammar/**`, `queries/**`, package metadata, generated binding
templates, `src/scanner.c`, or package allowlists, regenerate through the package script:

```console
npm run generate --prefix distribution/tree-sitter-mermaid
```

For review, prove the checked-in artifacts are current without writing:

```console
npm run generate:check --prefix distribution/tree-sitter-mermaid
```

Generation is treated as a whole artifact transaction. The receipt binds source inputs, generated C
and JSON, C headers, WASM, query profiles, metadata, package manifests, and the exact selected
Mermaid/ZenUML and Tree-sitter runtime identities. Extra, missing, stale, or cross-version generated
files are failures, not warnings.

## Focused verification

Use these checks while iterating on package-owned code:

```console
npm run test:corpus --prefix distribution/tree-sitter-mermaid
npm run test:corpus:wasm --prefix distribution/tree-sitter-mermaid
npm run test:queries --prefix distribution/tree-sitter-mermaid
npm run test:incremental --prefix distribution/tree-sitter-mermaid
npm run test:adversarial --prefix distribution/tree-sitter-mermaid
npm run metrics:check --prefix distribution/tree-sitter-mermaid
cargo nextest run --locked -p tree-sitter-mermaid --no-fail-fast
cargo run --locked -p xtask -- verify-tree-sitter-mermaid --all-fixtures
```

Use `cargo fmt --all -- --check` and package-local JavaScript/Python syntax checks before a commit
that edits mixed-language tooling. Prefer serial Cargo runs when the machine is already busy.

## Support tiers

`metadata/support.json` is executable release evidence. A family may only move upward when its
own evidence rows support the higher tier:

- `recognized`: accepted headers route to the correct public family root.
- `structured`: legal source exposes named family structure and does not rely on a broad fallback.
- `query-complete`: structured plus every profile/surface has asserted captures or explicit N/A.
- `conformant`: query-complete plus admitted fixtures, recovery, incremental equivalence, schema,
  binding, fuzz, and metrics evidence.

Do not promote a family by prose. Run the metadata gate:

```console
cargo run --locked -p xtask -- verify-tree-sitter-mermaid
```

## Recovery and fallback policy

Valid structured source must not be accepted through a generic body fallback such as
`unstructured_body`, `catch_all_body`, `raw_line`, or generic `unknown_statement`. Family-local
malformed recovery nodes are allowed when they are named, finite, and preserve unaffected siblings.

The current retained names `git_graph_unknown_statement_keyword` and
`sequence_unknown_statement_head` are malformed-line recovery heads, not tier-enabling fallback
bodies. If a new family needs a similar recovery form, add a targeted conformance test explaining
why valid input cannot pass through it.

## Downstream queries

Portable queries are the schema baseline. Neovim, Helix, and Zed profiles adapt that schema for
their consumers and carry separate applicability matrices under `test/queries/`.

Run the downstream matrix after any public named node, field, capture, or query-profile change:

```console
npm run test:downstream --prefix distribution/tree-sitter-mermaid
```

The Zed fixture pins a repository commit because Zed consumes grammars from Git. If generated parser
bytes change, first commit the generated parser, then update
`distribution/tree-sitter-mermaid/test/downstream/zed/extension.toml` to a commit that contains the
matching parser.

## Robustness

Tree-sitter fuzzing is split by parser behavior:

- `tree_sitter_mermaid_parse`: arbitrary Mermaid inputs over all package-owned families.
- `tree_sitter_mermaid_edits`: byte/point edit sequences and fresh-tree equivalence.
- `tree_sitter_mermaid_scanner`: external scanner state, restart, overflow, and UTF-8 boundaries.
- `tree_sitter_mermaid_query`: arbitrary-tree query execution across all query profiles.

Run the bounded committed regression pass before release-oriented changes:

```console
npm run fuzz:regression --prefix distribution/tree-sitter-mermaid
```

