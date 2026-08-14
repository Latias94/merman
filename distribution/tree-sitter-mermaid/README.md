# tree-sitter-mermaid

This directory owns Merman's independently versioned tolerant Tree-sitter language package for
Mermaid source. It is intentionally adjacent to, and not part of, Merman's semantic parser stack.

The current `0.1.0` package is a dry-run-only development surface. Its support contract starts with
all 35 public families in the `planned` lifecycle and no support tier. A family is promoted only by
the executable evidence described in `metadata/support.json`; header recognition alone is not
semantic validity and is not structured CST support.

Run the boundary and contract gate from the repository root:

```console
cargo run --locked -p xtask -- verify-tree-sitter-mermaid
```

The package pins Tree-sitter CLI/Rust/web runtime `0.26.12`, source-built Node runtime `0.25.1`,
and generated ABI 14. The Merman parsers, IR, analysis, editor core, and LSP remain authoritative
for validity, semantic construction, diagnostics, navigation identity, and safe refactoring.
