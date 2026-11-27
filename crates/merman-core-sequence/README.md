# merman-core-sequence

Tokenization, grammar actions, and semantic model construction for Mermaid sequence diagrams.
The `editor-facts` feature exposes the same spanned lexical events for editor projection.

This crate exposes a workspace-internal service provider interface (SPI). It is published in
lockstep with Merman and expects an exact version match with the Merman crates that use it.

Use `merman-core` for detection, inline YAML policy, resource admission, source-mapped
diagnostics, and editor and compatibility projections.
