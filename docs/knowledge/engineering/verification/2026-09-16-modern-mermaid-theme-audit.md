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

## 第一轮结构化统计（2026-09-16）

对 `themes.ts` 的 22 个顶层主题配置做了只读解析，结果为：225 个 theme variable
字段，CSS 原文合计约 223,377 字节。单个主题 CSS 约 1,333 至 18,646 字节；最大的是
Doodle、Aurora、Win95、Organic 和 Geometric Collage。该数字是参考源码的输入规模，
不是 Merman 运行时或发布包的体积估算。

当前 Merman catalog 的 10 个预设与参考主题只有 Brutalist、Spotless、Cyberpunk 三个
直接同名条目。Editor Light/Dark、One Dark、Gruvbox Light/Dark、Ayu Light/Dark 是
Merman 自有或兼容型预设，不应因为参考项目没有同名配置而视为缺失。参考项目的其他
19 个主题也不应仅凭名称直接加入 catalog；它们需要先证明 token 映射、终端语义、导出
行为和宿主字体条件。

参考 CSS 大量使用选择器级覆盖、`!important`、滤镜、阴影和背景图案。当前公共主题
模型对这些行为的处理应按机制分类：可表达的 typed paint/typography/geometry、宿主
CSS 能力边界、以及明确的 Unsupported/Unverified。不能以 CSS 字节数或选择器数量
推导 Merman 需要复制同等复杂度。

## 当前候选产物与初步运行数据

在当前候选已有产物中，macOS ARM64 CLI 二进制约 51.4 MiB，CLI archive 约 12.9 MiB，
LSP 二进制约 17.0 MiB，LSP archive 约 3.9 MiB；Node N-API `merman.node` 约 23.4 MiB，
Node WASM 原始文件约 19.2 MiB。Web 五个 WASM profile 的原始文件约 3.5、5.0、3.6、
15.2 和 13.4 MiB。上述是文件大小观察，不是跨版本回归结论。

使用已构建的 macOS ARM64 CLI 对一个含分支、主题覆盖和边标签的 Flowchart 做了 3 个
预设的单进程 CLI smoke。SVG 五次运行的 wall-clock 约为 0.01–0.06 秒，PNG 单次约为
0.02–0.19 秒；首次运行明显受进程/字体初始化影响。这组样本太小，只能作为后续基线
命令的 sanity check，不能据此声称性能提升或退化，也不能据此修改预算。
