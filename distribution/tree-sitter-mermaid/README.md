# tree-sitter-mermaid

This directory owns Merman's independently versioned tolerant Tree-sitter language package for
Mermaid source. It is intentionally adjacent to, and not part of, Merman's semantic parser stack.

The current `0.1.0` package is a dry-run-only development surface. All 35 public families are at
the `conformant` support tier. Conformant support requires pinned, runtime-replayed header
dispatch plus family-local corpus, recovery, incremental, node-schema, query, conformance,
binding, fuzz, and metrics evidence recorded in `metadata/support.json`.

The package includes receipt-bound portable, Neovim, Helix, and Zed query profiles. C, Rust, Node,
and language-WASM consumers validate the portable profile bundle, while fixed downstream harnesses
exercise each editor's declared surfaces. The complete profile applicability matrices and exact
captures are recorded under `test/queries/`.

Run the boundary and contract gate from the repository root:

```console
cargo run --locked -p xtask -- verify-tree-sitter-mermaid
```

The pinned Mermaid header oracle has its own dependency closure and lockfile. Replay it without
adding Mermaid to the language package runtime:

```console
npm ci --ignore-scripts --prefix distribution/tree-sitter-mermaid/scripts/header-oracle
npm run check:header-oracle --prefix distribution/tree-sitter-mermaid
```

The source package carries the exact upstream notices and MIT license texts under
`THIRD_PARTY_NOTICES.md` and `THIRD_PARTY_LICENSES/`. Mermaid and ZenUML use package-local
historical baseline components so a repository baseline upgrade can report `drifted` without
rewriting this package's selected source identity.

The package pins Tree-sitter CLI/Rust/web runtime `0.26.12`, source-built Node runtime `0.25.1`,
and generated ABI 14. The Merman parsers, IR, analysis, editor core, and LSP remain authoritative
for validity, semantic construction, diagnostics, navigation identity, and safe refactoring.

Development workflow and release dry-run checklists live in
`docs/development/TREE_SITTER_MERMAID.md` and `docs/release/TREE_SITTER_MERMAID.md`.
