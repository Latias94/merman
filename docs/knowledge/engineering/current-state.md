---
type: Current State
status: active
---

# Current State

- Goal: 完成 merman 的 Flowchart ELK 适配：以 Mermaid 与 Eclipse ELK 源码为依据持续 port 到成熟可默认使用，补齐 source-backed layered/compound 语义、重构适配层边界、收敛 ELK fixture，并保持默认 render 路径稳定。
- Branch: main
- Last verified: 2026-06-18
- Done: Flowchart ELK source-backed probes are green; compound parent-end external dummy net-flow semantics were ported closer to ELK source; inside-self-loop edges now route into the source node nested graph when ELK `insideSelfLoops.activate` is on; targeted compound/importer/layout tests pass.
- In progress: continue source-backed semantic convergence around compound/self-loop boundary; assess whether any higher-level adapter boundary still needs source-backed rework.
- Blocked: none
- Next action: decide whether to port further ELK recursive/compound behavior or stage and continue with the next source-backed gap.

# Citations

- [compound.rs](../../../../crates/merman-elk-layered/src/compound.rs)
- [importer.rs](../../../../crates/merman-elk-layered/src/importer.rs)
- [pipeline.rs](../../../../crates/merman-elk-layered/src/pipeline.rs)
- [layout.rs](../../../../crates/merman-layout-elk/src/lib.rs)
