# Contracts

This directory contains shared machine-readable contracts. Each descriptor has an implementation
or generator that consumes it; this index only identifies those owners.

| Contract | Consumers and maintenance |
| --- | --- |
| [ABI descriptors](abi/README.md) | Native C ABI generation, cross-platform host text measurement, and the independent WASI transport. |
| [Rename policy](editor-language/rename-policy-v1.json) | [The editor-language generator](../crates/xtask/src/cmd/editor_rename_policy.rs) projects the parser-backed safe-rename policy into Rust and Web analysis types. Use `gen-editor-language-contract` and `verify-editor-language-contract`. |

Generated files remain with their consuming surfaces. Capability IDs and artifact recipes belong
to [the capability descriptors](../capabilities/README.md). Tree-sitter syntax, queries, and
provenance belong to [the grammar distribution](../distribution/tree-sitter-mermaid/README.md).
