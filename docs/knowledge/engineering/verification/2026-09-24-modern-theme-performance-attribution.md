---
type: Audit Report
title: Modern Mermaid README theme capability and performance attribution
timestamp: 2026-09-24
source_commit: 4c1a10733
reference_commit: a021cbce37fc0b07a9f4791c28e983101ea06f2d
comparison_revision: v0.8.0-alpha.6
---

#结论

当前分支已经具备输出一部分接近 Modern Mermaid README 展示效果的基础能力，但还不能把自己描述成“支持 README 中的主题集合”。现有能力最接近 Brutalist、Spotless、Cyberpunk 三类效果；Ghibli、Memphis、HandDrawn 尚未成为当前公开 preset。更严格地说，当前 Cyberpunk 的复杂 glow 可以在 standalone SVG 中生成，但同一场景的 PNG/PDF 导出会触发 `max_total_svg_conversion_filter_primitives = 128` 的硬上限，因此还不能作为跨目标的完成验收。

性能方面需要分成两个结论：

- **主题视觉机制本身的增量目前是合理的。** 旧主题实现关闭字体处理与当前分支对比，普通 SVG 端到端仅约 `-0.16%` 到 `+1.97%`，主题 API 约在 `±2%` 内；移除 embedded fonts 后二进制也下降。
- **alpha.6 到当前版本的整体回归不合理地归因成“主题成本”。** 同输入阶段 profile 显示 parse 基本稳定，主要增量发生在 SVG 生成和最终 admission/finalization。当前端到端比 alpha.6 慢约 `1.76x` 到 `2.95x`，默认 CLI stripped 增长约 `19.00%`。这应被视为版本区间回归，需要继续拆解，而不是接受为主题功能的必然成本。

#README 级主题效果的当前边界

Modern Mermaid README 的主题截图实际展示了六种设计语言：

| README 示例 | 主要视觉机制 | 当前状态 |
| --- | --- | --- |
| Brutalist | 粗轮廓、硬偏移阴影、强对比和 ordinal 色板 | 有 preset，可作为首批验收样式；仍需按 family/target 完成 qualification |
| Cyberpunk | 青紫霓虹、链式 glow、深色网格和径向背景 | 有完整 recipe；当前有 Flowchart、Sequence、XY Chart 公共场景；PNG/PDF 复杂场景受滤镜硬上限限制 |
| Ghibli | 米色纸张、棕色线条、无边框卡片、柔和阴影 | 当前无公开 preset |
| Memphis | 黑色粗线、硬阴影、鲜艳 ordinal 色、几何纹理 | 当前无公开 preset |
| Spotless | 纸张/网格、手工墨线、全大写、紧凑圆角 | 有 preset，可作为首批验收样式；字体仍是宿主能力 |
| HandDrawn | 手写字体和不规则线条滤镜 | 当前无公开 preset；字体和不规则滤镜都需要单独的宿主/目标验收 |

这意味着第一阶段应固定一个小而可证明的展示集合，而不是直接承诺 Modern Mermaid 的 24 个主题。建议第一阶段以 `brutalist`、`spotless`、`cyberpunk` 为三个 recipe archetype，分别覆盖：硬阴影、纸张/手工风、霓虹/多层背景。Ghibli、Memphis、HandDrawn 应作为后续 recipe 设计工作，不应只通过改名或复制颜色进入 catalog。

当前 README fence 执行探针共运行 `34 × 5 × 3 = 510` 个输出组合。以当前 CLI 和五个已存在 preset（editor-light、editor-dark、brutalist、spotless、cyberpunk）执行时，有 477 个组合成功，33 个失败：

- 18 个 Cyberpunk PNG/PDF 组合因累计 SVG filter primitive 为 132 或 136，超过 128 的 native export 硬上限；
- 15 个 GitGraph 组合因输入包含当前 parser 不接受的中文分支引用而失败，这属于输入/diagram parser 问题，不是主题机制问题。

这个探针只能证明“能否执行并生成合法签名”，不能替代视觉 qualification。真正的 README 级验收需要固定场景、主题、family、target 和宿主字体条件，并检查机制与语义。

#性能归因

## 同口径 alpha.6 阶段 profile

两边使用相同 Mermaid fixture、默认 feature、默认 `Renderer::new()` 路径和 8 秒阶段 profile。当前值为每秒完成次数，越低表示越慢：

| 场景 | 阶段 | alpha.6 | 当前 | 当前相对吞吐 |
| --- | --- | ---: | ---: | ---: |
| flowchart_tiny | parse | 1,588,757 | 1,484,895 | 93.5% |
| flowchart_tiny | layout | 581,397 | 446,600 | 76.8% |
| flowchart_tiny | render | 288,600 | 169,900 | 58.9% |
| flowchart_tiny | end-to-end | 226,345 | 76,686 | 33.9% |
| sequence_medium | parse | 273,550 | 278,049 | 101.6% |
| sequence_medium | layout | 109,419 | 163,856 | 149.8% |
| sequence_medium | render | 73,200 | 49,700 | 67.9% |
| sequence_medium | end-to-end | 56,920 | 24,578 | 43.2% |
| class_medium | parse | 104,154 | 105,886 | 101.7% |
| class_medium | layout | 26,773 | 23,408 | 87.4% |
| class_medium | render | 15,200 | 12,300 | 80.9% |
| class_medium | end-to-end | 13,178 | 7,510 | 57.0% |

parse 阶段没有数量级变化；sequence 的 layout 甚至更快。由此可以排除“主题配置解析让所有阶段都变慢”这个简单解释。render 阶段的增量与 SVG 输出、文字准备、效果 materialization 和 evidence 记录相关；end-to-end 还额外包含最终 standalone artifact 的校验和 target admission。

## 当前代码中可对应的成本

当前 SVG public path 在 `render_svg_target` 中会继续执行 `finalize_standalone_for_target_admission`。该路径包括：

1. prepared text ledger 和 terminal receipt 的确认；
2. SVG resource closure fingerprint；
3. XML well-formed 校验；
4. reference/resource budget 和 resvg compatibility 检查；
5. 主题 evidence、native filter receipt、宿主字体和 target admission digest 的绑定；
6. document/receipt 的 SHA-256 摘要。

现有 CPU sampling 已观察到 `finalize_standalone_with_portability`、`StandaloneSvgArtifact::finalize_exact`、`check_svg_resource_budget_with_controls` 和 `validate_well_formed_element` 占据当前 Class medium 端到端样本的重要比例，而 alpha.6 没有对应的 finalization symbols。这个证据支持以下归因排序：

- **高可信：** 输出安全契约、资源闭包、XML/reference 校验、target admission 和摘要计算是当前端到端回归的主要来源；
- **中可信：** 主题 effect 实例化和 per-terminal filter/evidence 记录增加了 SVG render 成本，尤其是 Cyberpunk；
- **低可信：** 仅把 theme recipe 编译、颜色 token、font-family 字符串或 preset catalog 解释成主要回归来源。已有主题 API 基准不支持这种解释。

因此合理的优化方向不是删除主题验收证据，而是把普通 SVG、strict/resvg-safe SVG、PNG/PDF native export 的必需工作分层，并测量每一层的独立成本。任何削弱资源限制或 target admission 的改动都不能直接作为性能优化合入。

#建议的验收门槛

README 级主题验收应按 `theme × family × target × host` 建立 cell，而不是只检查 preset 名称：

1. **视觉机制**：背景层、颜色 token、ordinal palette、圆角/线宽、阴影或 glow、字体名称分别有可观察证据；
2. **语义保持**：节点/边/消息/系列数量、marker、标签和顺序不因主题改变；
3. **目标分离**：standalone SVG、PNG、PDF 分开验收；SVG 成功不能自动推导 PNG/PDF 成功；
4. **字体边界**：字体名称和排版值可进入 recipe，字体字节不打包，缺失字体必须显示 HostDependent 或明确 residual；
5. **资源预算**：每个 effect graph、每个输出目标和聚合 filter primitive 数量都在预算内；超预算应是可解释的失败；
6. **固定展示集**：至少为 Brutalist、Spotless、Cyberpunk 各固定一组 README 风格场景，并保存 source、recipe fingerprint、target 和输出摘要；
7. **资格状态**：只有上述证据完整时才填写 `qualified_cells`，否则保留 Available/HostDependent/Unverified。

当前建议的产品判断是：主题能力可以继续推进 README 级展示，但先把三个已有 archetype 做成跨目标、跨 family 的小矩阵；性能上先关闭“主题重构整体回归”这个未决项，优先对 finalization/resource validation 和 effect materialization 做消融测量。只有当普通无效果主题的 alpha.6 回归被压回可解释范围，并且 Cyberpunk 的 PNG/PDF 预算得到明确策略后，才适合把更多 README 主题加入公开 catalog。
