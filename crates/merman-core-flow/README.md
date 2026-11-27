# merman-core-flow

Stateful tokenization, grammar actions, and semantic model construction for Mermaid Flowchart,
Swimlane, and Agentflow. The independent `diagram-agentflow` feature parses Agentflow source
and reuses the shared presentation directive lexer. `editor-facts` enables lexical spans and
observations used by Core's editor projection. `presentation` enables construction of the
families' render context.

This crate exposes a workspace-internal service provider interface (SPI). It is published in
lockstep with Merman and expects an exact version match with the Merman crates that use it.

Use `merman-core` for source preprocessing, resource admission, and diagnostics mapped to original
source coordinates.
