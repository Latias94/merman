# tree-sitter-mermaid

This directory owns Merman's independently versioned tolerant Tree-sitter language package for
Mermaid source. It is intentionally adjacent to, and not part of, Merman's semantic parser stack.

The current `0.1.0` package is a dry-run-only development surface. All 35 public families are at
the `structured` support tier. Structured support requires pinned, runtime-replayed header
dispatch plus family-local corpus, recovery, incremental, node-schema, query, and conformance
evidence recorded in `metadata/support.json`.

The package includes a receipt-bound portable highlight profile at
`queries/portable/highlights.scm`. C, Rust, Node, and language-WASM consumers compile the same
profile, while Node and language-WASM replay the U3-U7 capture goldens. This proves the portable
highlight delivery contract for all structured families; complete family-by-surface editor
coverage remains gated by the later `query-complete` support tier.

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
