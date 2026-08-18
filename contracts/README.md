# Contracts

This directory owns machine-readable contracts that are projected into multiple runtime and tool
surfaces.

- `abi/` owns the native ABI and host text-measurement protocol descriptors.
- `editor-language/` owns the parser-backed safe-rename policy shared by Rust and Web analysis
  payload types. Syntax highlighting is owned by the Tree-sitter Mermaid distribution.
- `tree-sitter/` contains composed language receipts. Merman projects public family IDs, internal
  variants, and authoring-header suggestions into them; each language package owns its accepted
  syntax, CST roots, query schema, evidence, and support tiers.

Edit the descriptors here and use their documented `xtask` generators. Generated Rust, C,
TypeScript, Kotlin, Dart, Python, and editor files remain in the surface that consumes them.
