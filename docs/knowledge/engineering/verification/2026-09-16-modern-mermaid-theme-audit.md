---
type: Audit Baseline
title: Modern Mermaid theme capability audit baseline
timestamp: 2026-09-16
related_plan: docs/plans/2026-09-15-theme-c7a-c7b-replan.md
git_branch: refactor/presentation-theme-model
git_commit: 29d3f4da4
tags: theme,preset,modern-mermaid,audit
---

# 审计范围

本记录建立第二阶段功能审计的输入基线，不代表主题能力已经通过，也不改变 C7a
发布门槛。参考仓库位于 `repo-ref/modern_mermaid`，目录实际名称使用下划线。
本轮只读取参考实现和当前 Merman catalog，没有修改参考仓库。

## 参考主题输入

`src/utils/themes.ts` 声明 22 个主题：

`linearLight`、`linearDark`、`notion`、`cyberpunk`、`monochrome`、`ghibli`、
`spotless`、`brutalist`、`glassmorphism`、`softPop`、`darkMinimal`、`wireframe`、
`memphis`、`noir`、`material`、`aurora`、`win95`、`doodle`、`organic`、`hightech`、
`kawaii`、`geometricCollage`。

每个主题都同时提供 Mermaid `themeVariables`、`themeCSS` 和字体栈。静态统计结果为：

- 22 个主题配置；24 个变量/CSS/字体声明（包含对象结构中的两个嵌套配置）；
- CSS 文本约 1.3 KiB 至 18.6 KiB；选择器约 12 至 107 个；
- 字体栈使用 Inter、系统字体、Noto Sans SC、Open Sans、Roboto、JetBrains Mono、
  Comic Sans、Courier New 等。参考实现依赖消费者或宿主字体，没有将这些字体自动变成
  Merman 发布资源的理由。

参考实现的 CSS 明确覆盖 Flowchart、Sequence、XY Chart、节点/边/cluster、actor、note、
axis、legend 等局部终端；部分主题还使用背景图案、阴影、滤镜和 `!important`。这些
CSS 选择器不能直接当作 Merman typed 支持声明，必须按终端语义逐项归类。

## 参考图表示例输入

`MERMAID_EXAMPLES.md` 当前包含以下主要族：Flowchart、Sequence、Class、State、ER、Gantt、
Pie、Git Graph、User Journey、Mindmap、Timeline，以及高级 Flowchart/Sequence 示例。
示例包含中文、英文、日文和法文文本，并覆盖 subgraph、classDef、注释和备注等样式用法。

## 当前 Merman 预设基线

当前 Rust catalog 有 10 个条目：Editor Light、Editor Dark、One Dark、Gruvbox Light、
Gruvbox Dark、Ayu Light、Ayu Dark、Brutalist、Spotless、Cyberpunk。公开 qualification
仍只对实际 artifact、宿主和声明图表/输出单元建立，不因参考仓库存在主题名称就自动扩展。

## 后续审计方法

下一步将从参考主题中抽取 token、CSS 终端和示例族，建立以下矩阵：

1. token 是否有 typed 对应及明确 Unsupported/Unverified 结果；
2. 默认值、显式清除、透明色和 light/dark 隔离是否可观察；
3. SVG、PNG、PDF、ASCII 和各 binding 是否保持同一语义；
4. 现有十个预设在默认、深色、高对比、文档/演示、品牌色和复杂导出场景的直接可用性；
5. 主题编译、support discovery、catalog qualification 和渲染的体积、延迟、吞吐及内存基线。

审计结论必须区分“参考实现使用了 CSS”与“公共主题模型承诺支持该行为”。未测量的
体积或性能变化不用于放宽预算，也不触发新的 proof framework 或通用抽象。
