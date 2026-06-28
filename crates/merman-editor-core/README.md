# merman-editor-core

Protocol-neutral editor intelligence for Merman.

This crate is an internal Rust reuse layer shared by protocol adapters such as `merman-lsp` and
browser adapters such as `merman-wasm`. External editors should normally integrate through the LSP
server rather than depending on this crate directly.
