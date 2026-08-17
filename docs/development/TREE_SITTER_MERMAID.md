# Tree-sitter Mermaid Development

`distribution/tree-sitter-mermaid` is an independently versioned syntax package. It is not a
replacement for Merman's strict parsers and must not enter the production dependency closure of
`merman-core`, `merman-analysis`, `merman-editor-core`, or `merman-lsp`.

Tree-sitter owns tolerant CST structure, recovery, incremental reparsing, queries, generated
artifacts, and distribution bindings. Merman remains the sole authority for semantic construction,
diagnostics, IR, rendering, navigation identity, and safe refactoring.

## Local setup

Install the pinned package-local generator without running the native install hook:

```console
npm ci --ignore-scripts --prefix distribution/tree-sitter-mermaid
npm rebuild tree-sitter-cli --prefix distribution/tree-sitter-mermaid
npm run build:node --prefix distribution/tree-sitter-mermaid
```

Committed parser artifacts must always be generated through the package-local CLI. Do not use a
global Tree-sitter installation.

## Grammar changes

For a Mermaid syntax change:

1. identify the exact rule in the pinned Mermaid 11.16.1 or ZenUML Core 3.50.1 source;
2. change the smallest family-local grammar rule and add or update a standard Tree-sitter corpus
   case;
3. regenerate the ABI-15 native parser;
4. run the focused family corpus and the Rust integration suite;
5. run the one-way full-fixture oracle before merge.

```console
npm run generate --prefix distribution/tree-sitter-mermaid
npm run test:corpus --prefix distribution/tree-sitter-mermaid
cargo nextest run --locked -p tree-sitter-mermaid --no-fail-fast
```

The corpus is the CST and recovery golden authority. Do not duplicate expected trees in JSON,
Rust, Node, WASM, or editor-specific fixtures. Valid structured input must not pass through a broad
`unstructured_body`, `catch_all_body`, `raw_line`, or generic `unknown_statement` fallback.

## Generated artifacts

Native generation and language-WASM generation are deliberately separate:

```console
npm run check:generated --prefix distribution/tree-sitter-mermaid
npm run generate:wasm --prefix distribution/tree-sitter-mermaid
npm run check:wasm --prefix distribution/tree-sitter-mermaid
```

Both paths explicitly select ABI 15. Native freshness is an ordinary grammar gate. WASM freshness
is a slower CI/release lane because it requires the pinned WASI SDK. The generator keeps only wide
gross size ceilings; noisy timing and RSS receipts are not release contracts.

## Test ownership

The maintained test layers are intentionally small:

- `tree-sitter test`: CST structure and local recovery;
- `tests/conformance.rs`: every strict-valid Merman fixture maps to the expected Tree-sitter family
  root without errors or broad recovery;
- `tests/incremental.rs`: a family switch, an ordinary edit, an indentation-scanner edit, parser
  cancellation/reuse, and invalid UTF-8 stability;
- `tests/scanner_protocol.rs`: scanner serialization, restart, maximum depth/indentation, overflow,
  corruption reset, and representative family switching;
- `tests/queries.rs`: compile every shipped query, execute canonical highlights for the 35 small
  family sources, and run a few applicable injection/locals/tags examples;
- one representative load/parse smoke for Rust, Node, C, and language WASM.

Do not recreate support tiers, artifact/header receipts, schema snapshots, edit-trace DSLs,
per-editor applicability matrices, exact capture forests, or timing/RSS proof runners.

Use `cargo fmt --all -- --check` and `git diff --check` before committing. Prefer serial Cargo runs
when the shared machine is busy.

## Queries and editor adoption

`queries/portable` is the canonical package query contract referenced by `tree-sitter.json`.
Neovim, Helix, and Zed directories are pre-1.0 adoption assets. Editors ultimately own their
runtime query copies, so publication of the npm or Cargo package does not update editor users.

After an immutable grammar release exists, update downstream integrations to the release commit
and the monorepo subdirectory:

- nvim-treesitter: `location = "distribution/tree-sitter-mermaid"`;
- Helix: `subpath = "distribution/tree-sitter-mermaid"`;
- Zed: `path = "distribution/tree-sitter-mermaid"`.

Compile all shipped queries locally in Rust; do not download and execute a fixed editor version
matrix for every grammar change.

## Fuzzing

Tree-sitter keeps three bounded cargo-fuzz targets:

- `tree_sitter_mermaid_parse` for arbitrary input and deterministic spans;
- `tree_sitter_mermaid_edits` for incremental/fresh tree equivalence;
- `tree_sitter_mermaid_scanner` for external scanner state and valid-symbol masks.

The scheduled workflow owns randomized discovery. Stable regressions belong in the corpus or
focused Rust tests rather than in a second package-local fuzz runner.
