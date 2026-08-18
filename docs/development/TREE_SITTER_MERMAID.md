# Tree-sitter Mermaid Development

`distribution/tree-sitter-mermaid` is an independently versioned syntax package and the canonical
syntax-highlighting implementation for Merman's native LSP and Playground. It is not a replacement
for Merman's strict parsers.

Tree-sitter owns tolerant CST structure, recovery, incremental reparsing, queries, generated
artifacts, distribution bindings, and base syntax captures. Merman remains the sole authority for
strict validity, semantic construction, diagnostics, completion, navigation identity, safe
refactoring, IR, and rendering. A recovered Tree-sitter tree is useful editor state, not proof that
Mermaid accepts the source.

See ADR-0084 for the superseding highlighting decision and ADR-0083 for the preserved strict
language boundary.

## Runtime adapters and ownership

The grammar and `queries/portable/highlights.scm` are shared; runtime lifecycle and coordinate
projection are platform-local:

- `merman-lsp` uses the Rust grammar crate and Tree-sitter runtime, owns incremental syntax state,
  maps portable captures to the standard LSP legend, and preserves full/range/delta protocol state;
- the Playground uses `web-tree-sitter`, the language WASM, and the same query in a dedicated
  syntax worker, then projects captures to Monaco's UTF-16 token transport; and
- neither adapter may infer Mermaid syntax from regular expressions or Merman semantic facts, nor
  use `locals.scm` or `tags.scm` as navigation or rename evidence.

Syntax state is independent of `AnalysisGeneration` and semantic-worker readiness. Markdown and MDX
hosts use a lightweight syntax-side fence segmenter and parse only Mermaid fence bodies. Tree-sitter
failure is explicit; adapters do not fall back to the retired Merman lexical highlighter.

## Production dependency boundary

Native Tree-sitter dependencies are allowed only in these exact artifact profiles:

- `lsp-library`;
- `lsp-stdio-release`.

They remain forbidden from core, analysis, editor-core, render, IR, Web and WebAssembly, CLI, and
language-binding artifact closures. The Playground loads the external browser runtime and staged
grammar/query assets; it does not add Tree-sitter to Merman's Rust WebAssembly closure.

The repository-owned closure check enforces this exception by profile identity:

```console
python3 scripts/verify_artifact_dependency_closures.py --profile lsp-library --profile lsp-stdio-release
```

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

The package's Node-side WASM validation uses `--liftoff-only`: it checks the browser asset's bytes,
ABI, load, and representative parse without exercising Node 24's optimizing compiler on the large
generated lexer. Browser execution remains owned by the real Chromium smoke, while Node package
consumers are tested through the native binding under the default runtime.

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
  family sources with compact expected body capture classes, cover a small exact-span set, and run
  a few applicable injection/locals/tags examples;
- one representative load/parse smoke for Rust, Node, C, and language WASM.

Do not recreate support tiers, artifact/header receipts, schema snapshots, edit-trace DSLs,
per-editor applicability matrices, exact capture forests, or timing/RSS proof runners.

Use `cargo fmt --all -- --check` and `git diff --check` before committing. Prefer serial Cargo runs
when the shared machine is busy.

## Queries and editor adoption

`queries/portable` is the canonical package query contract referenced by `tree-sitter.json` and by
Merman's LSP and Playground adapters. Neovim, Helix, and Zed directories are pre-1.0 adoption
assets. External editors ultimately own their runtime query copies, so publication of the npm or
Cargo package does not update those editor users.

The monorepo Playground stages the canonical language WASM and query through its build instead of
installing the package root and invoking the native Node binding hook. External browser consumers
use the published npm package. Registry publication remains a protected release action and must not
be inferred from ordinary development work.

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
