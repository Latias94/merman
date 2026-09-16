---
type: Audit Report
title: Presentation theme capability and impact audit
timestamp: 2026-09-16
related_plan: docs/plans/2026-09-15-theme-c7a-c7b-replan.md
related_baseline: docs/knowledge/engineering/verification/2026-09-16-modern-mermaid-theme-audit.md
git_branch: refactor/presentation-theme-model
tags: theme,audit,preset,performance,modern-mermaid
---

# 审计结论状态

本报告是 C7a 候选准备之后的第二阶段审计初稿。它已经包含真实参考输入、当前 typed
coverage、预设范围和候选产物大小；性能部分只包含可重复的初步 smoke，不能被解释为
完整基准或发布冻结。C7a 的 Linux/Windows/Apple Swift 5.9 宿主证据仍按发布计划保留
为未验证，不能用本机结果替代。

# 已完成能力

- 主题定义、token expansion、complete spec、materialized wire、authoring diagnostics、
  support/catalog、qualification 和 execution evidence 已统一为未发布 v1。
- 33/33 图表族已有至少一个 typed 消费路径；生产 Legacy family route 为 0。
- Block、Class、Flowchart marker 等已完成的退休边界有独立验收证据。
- 默认值、显式 clear、transparent、ordinal/source ownership 和 writer-owned terminal
  evidence 已进入公共模型及跨 transport 测试。
- Web、Node、Python、Typst、UniFFI、Native C ABI 和 Flutter 已有共享 authoring/support
  contract 见证；Node N-API/WASM、Web、Typst 和 macOS ARM64 CLI/LSP 有安装或 archive
  replay 证据。

# 参考主题功能矩阵

参考仓库 `repo-ref/modern_mermaid` 的 `src/utils/themes.ts` 有 22 个主题、225 个变量
字段和约 223 KiB CSS 输入。`MERMAID_EXAMPLES.md` 覆盖 Flowchart、Sequence、Class、
State、ER、Gantt、Pie、Git Graph、User Journey、Mindmap 和 Timeline，并包含多语言文本、
subgraph、classDef、备注和高级示例。

| 机制 | 参考输入观察 | Merman 当前判断 | 证据/限制 |
| --- | --- | --- | --- |
| Canvas/background、surface、text、border、line、accent | 22 个主题均提供基础颜色；部分使用背景图案 | **已支持（typed 基础）** | 当前 palette/paint 与 writer 终端证据；背景图案属于宿主 CSS，不是统一 portable paint |
| Font stack、font size、weight | 每主题都有字体栈，含 Inter、系统字体、Roboto、Comic Sans 等 | **有限支持** | FontStack/FontSize 的 family coverage 由机制矩阵决定；字体可用性与测量后端仍是宿主条件，不内嵌 Inter |
| 明暗主题与透明色 | Linear、Cyberpunk 等显式 darkMode；多主题使用 rgba/透明 | **已支持但需按输出验证** | authoring/resource golden 覆盖 clear/transparent；原生 usvg 的 CSS4 语法仍需独立 PNG/PDF 矩阵 |
| Flowchart 节点、边、cluster、edge label | 参考 CSS 逐项覆盖，常用 `!important` | **已支持基础 typed；高级 CSS 有限** | Flowchart typed terminal 与 evidence 已验证；滤镜、任意 CSS selector 不属于同等公共保证 |
| Sequence actor、activation、note、loop | 18/22 主题出现相关 selector | **已支持 typed 基础** | Sequence terminal 语义有 writer/evidence；参考 CSS 的任意 selector 需归类为 host CSS |
| Class relation、成员/标题文字 | 13/22 主题出现 class/relation selector | **已支持但机制不全** | Class Text 已 typed 并退休 bridge；具体关系/背景细节按 coverage matrix 逐项判断 |
| XY Chart series、axis、title、legend、ticks | 22/22 主题出现 plot/axis/chart selector | **有限支持** | Merman 有 typed XY Chart paint，但参考 CSS 的选择器级覆盖和任意 series CSS 不等于 portable contract |
| Pie、Gantt、ER、GitGraph、Journey、Mindmap、Timeline | 参考示例覆盖；主题 CSS 主要依赖 Mermaid 生成 DOM | **按 family 分级** | 每族有 typed surface 不代表所有 mechanism 已支持；使用 support discovery 和 coverage matrix |
| shadow、filter、text-shadow、渐变、背景图案 | 15/22 主题使用滤镜/阴影，12/22 有背景相关样式 | **当前有限或 Unsupported** | 这些是 CSS/渲染器能力，不应静默投影为普通 paint；需返回明确 residual/Unsupported |
| 任意 `themeCSS`、`!important`、DOM selector | 22/22 主题都含 CSS 文本或 selector 级定制 | **不作为 portable typed 能力** | Web 可保留受约束 CSS 能力；原生导出和跨 transport 不承诺任意 CSS 等价 |

# 当前预设判断

当前 catalog 有 10 个预设：Editor Light、Editor Dark、One Dark、Gruvbox Light、Gruvbox
Dark、Ayu Light、Ayu Dark、Brutalist、Spotless、Cyberpunk。参考项目与其直接同名的只有
Brutalist、Spotless、Cyberpunk；这三个已经有受限 host-dependent qualification，不能
推广成 Portable 或全族合格。

| 场景 | 当前判断 | 需要补的证据 |
| --- | --- | --- |
| 默认编辑与文档 | Editor Light/Dark、One Dark 具备合理基础 palette | 用参考示例族跑 SVG/PNG/PDF，并检查文字可读性与默认字体条件 |
| 深色主题 | Editor Dark、One Dark、Ayu Dark、Cyberpunk 有候选覆盖 | 检查 edge label、note、sequence、export 对比度；保留 host-dependent 边界 |
| 高对比 | Brutalist、Cyberpunk 的视觉意图较强 | 需要可读性/对比度和复杂族验证，不能只看截图 |
| 演示/品牌色 | Brutalist、Spotless、Cyberpunk 可作为候选起点 | 需要品牌 token 替换、透明/clear、PNG/PDF 和长文本测试 |
| 复杂图表 | catalog 有十个候选，但 qualified cells 仍限定在实际 artifact/profile | 不应把未 qualification 的 preset/族/输出宣传为已支持 |
| 参考项目的 Ghibli、Memphis、Material、Aurora 等主题 | 当前没有对应 Merman preset | 先证明公共模型和宿主导出能表达，再决定是否新增；不按名称直接扩展 |

# 性能、体积和架构影响的当前证据

当前候选本地文件观察：macOS ARM64 CLI 二进制约 51.4 MiB、CLI archive 约 12.9 MiB、
LSP 二进制约 17.0 MiB、Node N-API 约 23.4 MiB、Node WASM 约 19.2 MiB；Web 五个 WASM
profile 约 3.5–15.2 MiB。参考主题 CSS 输入合计约 223 KiB，不能直接与这些产物相加。

一个含分支和边标签的 Flowchart 通过 CLI 运行三个 preset：SVG 五次 wall-clock 约
0.01–0.06 秒，PNG 单次约 0.02–0.19 秒；首次运行受进程和字体初始化影响。这只是
sanity baseline。后续正式基准必须固定输入、预热规则、运行次数、输出目标、字体环境和
内存采样，并与重构前可复现提交比较。

当前架构影响判断：typed family owner、共享 authoring/support fixture、artifact/profile
admission 和 writer-owned evidence 增加了代码与验证量，但没有证据要求引入第二套 proof
engine。预设 catalog 和 acceptance 记录的维护成本是真实成本；应在 C7a 后删除已完成
迁移的临时行，保留历史 authority，不为了减少文件数强行合并 family owner。

# 明确不支持或暂缓

- 任意 Mermaid `themeCSS` selector、滤镜、阴影、背景图案和 DOM-specific `!important`
  不承诺跨 SVG/PNG/PDF/ASCII/宿主一致。
- 参考仓库中的 22 个主题不自动成为 Merman preset；未建立 recipe、qualification cells
  和 artifact binding 的主题保持未发布/未资格状态。
- 未在 Linux/Windows/Swift 5.9 上执行的产物不宣称已验证。
- 未完成 benchmark 的 XY Chart 终端缓存、字体数据库复制等优化不进入本轮功能结论。

# C7b 延后项

C7a 之后再评估剩余 family mechanism breadth、长尾预设、参考 CSS 机制分类、跨宿主视觉
比较和未测量性能优化。C7b 不应通过增加新的通用 proof framework 或恢复 Legacy provider
来推进。

# 下一步

1. 从 `MERMAID_EXAMPLES.md` 选择每个主要族的最小代表输入，运行现有 preset 和显式
   authoring spec，记录 Applied/Unsupported/Unverified 及输出目标。
2. 对 Brutalist、Spotless、Cyberpunk、Editor Light/Dark 建立默认、深色、高对比、长文本
   和复杂导出矩阵；其余预设先做 discovery，不直接 qualification。
3. 以当前候选和明确的历史基线分别测量冷启动、预热渲染、SVG/PNG/PDF 吞吐、主题编译、
   discovery 和大图表内存；只有归因后的结果才进入预算或架构决策。

# 代表性参考示例真实渲染（2026-09-16）

从 `MERMAID_EXAMPLES.md` 选取了 11 个主要族的最小代表输入：Flowchart、Sequence、Class、
State、ER、Gantt、Pie、Git Graph、Journey、Mindmap、Timeline。使用当前 macOS ARM64
CLI 和 `editor-light`、`editor-dark`、`brutalist`、`cyberpunk` 四个预设，分别执行 SVG
和 PNG 输出，共 88 次执行：

- SVG：44/44 通过；
- PNG：44/44 通过；
- 未出现解析、主题编译或导出失败。

输出大小随族和预设变化，已保存在本轮临时矩阵结果中
`/tmp/merman-modern-theme-matrix-results.json`。这证明当前代表性 Mermaid 输入能通过
候选 CLI 的基础 SVG/PNG 渲染路径；它不证明参考项目 22 个主题均已移植，不证明所有
family mechanism 已 typed 支持，也不改变 preset qualification 的 artifact/profile
范围。PDF、ASCII、长文本、多语言和 CSS 特殊机制仍需单独验证。
