# merman-core-flow

Stateful tokenization and flat grammar actions for Mermaid Flowchart and Swimlane. The independent
`diagram-agentflow` feature exposes the shared presentation directive lexer. `editor-facts`
enables lexical spans and recovery observations used by Core's editor projection.

This crate exposes a workspace-internal service provider interface (SPI). It is published in
lockstep with Merman and expects an exact version match with the Merman crates that use it.

Use `merman-core` for Mermaid source preprocessing, typed model construction, sanitization, and
diagnostics mapped to original source coordinates.
