---
type: "Session Handoff"
title: "Flowchart ELK inside-self-loop source-backed audit"
description: "Session Handoff for Flowchart ELK inside-self-loop source-backed audit."
timestamp: 2026-06-18T08:30:18Z
tags: ["elk", "flowchart", "self-loop", "compound", "source-backed"]
source_session: "019ed8bc-c507-7e60-b55c-be437ae35a80"
---

# Summary

当前这轮把 ELK source-backed 的 inside-self-loop 链路补齐到和 ELK 源码一致的方向：

- `insideSelfLoops.activate` 仅在开启时允许节点因为 inside self-loop 生成 nested graph。
- `inside_self_loops_yo` 边现在会被导入到源节点自己的 nested graph，而不是停留在外层图。
- `merman-layout-elk` 的 source-backed 输入侧补了 `ElkInputEdge` 新字段默认值。
- 相关测试、`cargo fmt --all` 和 `xtask check-flowchart-elk-source-backed-probes` 都已通过。

# Verified State

- `cargo test -p merman-elk-layered --tests`
- `cargo test -p merman-layout-elk --tests`
- `cargo run -p xtask -- check-flowchart-elk-source-backed-probes`
- `cargo fmt --all`

# Open Threads

- 继续对照 ELK 源码，看 compound/recursive 层还有没有其他必须 port 的语义缝隙。
- 评估 `repo-ref/mermaid` 侧是否需要把 inside self-loop 的公开配置面补出来，还是维持仅 source-backed 内部语义。

# Next Action

继续从 ELK 源码往下 port 下一段 compound/recursive 语义，同时保持 source-backed 探针稳定。

# Citations

- [importer.rs](../../../../crates/merman-elk-layered/src/importer.rs)
- [compound.rs](../../../../crates/merman-elk-layered/src/compound.rs)
- [lib.rs](../../../../crates/merman-layout-elk/src/lib.rs)
