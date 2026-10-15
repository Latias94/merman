---
type: Audit Report
title: Theme acceptance analysis after embedded-font retirement
timestamp: 2026-09-23
source_commit: 3b760a0c7
comparison_branch: main
preserved_branch: preserve/embedded-fonts-theme
---

#结论

后续以 [September 24 correctness and attribution checkpoint](2026-09-24-modern-theme-performance-attribution.md) 为准：它修正了阶段单位、默认校验路径和主题视觉范围，并增加当前源码的 PDF 像素与 CPU 采样证据。

当前主题能力已经达到“宿主字体主题”的内部验收线，但还没有达到 alpha 发布闭环。

本次判断以 `docs/plans/2026-09-16-1618-refactor-theme-product-boundaries-plan.md` 的 R1–R15 和
AE1–AE6 为准，并把内嵌字体边界改为当前产品决策：主题保留字体名称与排版值，使用宿主字体，
对主题内字体字节返回明确的 InvalidArgument；完整内嵌字体实现保留在
`preserve/embedded-fonts-theme`，不再是当前产品的验收要求。

`main` 当前指向 `3a2adebf8`，还没有当前分支的 `diagram_theme` 和
`merman-theme-acceptance` 模块。因此 `main...HEAD` 的总 diff 不能作为主题行为回归基线；
字体退役切片使用 `dfe54f279` 与保留分支对照；主题重构整体则须对照重构前版本并核实输入、能力和输出语义，不能用已经含主题实现的基线代替。

#当前已经成立的能力

- 十个公开 preset 都能编译并导出完整 recipe；当前 catalog 保持 `alpha`，没有公开
  `qualified_cells`。
- ThemeRecipeV1 的导出、导入、未知版本、混合选择、重复字段和资源上限路径已有 Rust、CLI、
  Web、Node、Python、Apple、Flutter、C/UniFFI 相关证据。
- family design scope 已公开区分 `base_only`、dedicated 和未审核范围；切换 family 不会
  偷换 preset。
- 主题诊断能区分 Applied、Residual、Unsupported、Unverified、HostDependent 和 Rejected；
  support discovery 不再被当作实际终端应用证明。
- 当前最小 SVG profile 不再包含主题字体解析、WOFF2 解码、native shaping 和 SVG font injection
  链路。资源字体在 compiler/binding 边界明确拒绝，宿主 `fontFamily`、字号、字重和间距仍可用。
- 当前分支的 67 项主题 acceptance tests、16 项聚焦字体/主题测试、依赖闭包、feature matrix、
  binding/native ABI projections、平台绑定和 SVG structure gate 已通过。
- 发布产物测量显示：相对旧版启用字体实现，剥离符号的 CLI 减少 705,520 字节，FFI dylib 减少
  1,306,520 字节，FFI staticlib 减少 2,993,632 字节。该测量是 macOS arm64 单机证据。

#性能与二进制大小证据（2026-09-23）

这次测量把两种问题分开：

- **字体退役切片的运行时变化**：`dfe54f279`（旧代码、关闭 `embedded-fonts`）对比当前分支；两边使用相同的 `svg` profile、相同 fixture、相同宿主字体路径。
- **字体实现总成本**：旧代码开启 `embedded-fonts` 对比当前分支，使用前一轮 macOS arm64 release artifact 测量。

字体退役前后的三轮 Criterion 基准中位数如下；正值表示当前分支更慢。这不能测量主题实现相对无主题实现的成本：

| 基准 | 旧代码关闭字体 | 当前分支 | 变化 |
| --- | ---: | ---: | ---: |
| `end_to_end/flowchart_tiny` | 103.40 µs | 104.70 µs | +1.26% |
| `end_to_end/sequence_medium` | 322.22 µs | 326.61 µs | +1.36% |
| `end_to_end/class_medium` | 1.0813 ms | 1.1026 ms | +1.97% |
| `end_to_end/mindmap_medium` | 308.53 µs | 308.05 µs | -0.16% |
| `render/flowchart_tiny` | 31.46 µs | 31.68 µs | +0.70% |
| `render/sequence_medium` | 122.09 µs | 121.18 µs | -0.75% |

主题 API 的单轮精确测量也没有显示回归：`theme_compile_definition/default` -0.84%、
`theme_materialize/default` -0.72%、`theme_compile_preset/cyberpunk` +1.06%、
`theme_export_preset/editor-light` -0.48%、`theme_catalog/reused_engine` +0.92%、
`theme_catalog/fresh_engine` -0.01%。与完整旧实现（开启 `embedded-fonts`）相比，当前分支的
主题 API 结果同样在约 ±2% 噪声范围内：Cyberpunk preset 编译为 -1.27%，catalog reused 为
-1.34%，fresh engine 为 -2.24%。这些不是发布承诺的精确上限，只用于确认没有数量级回归。

二进制结果来自同一台 macOS arm64、Rust 1.95.0、release profile、串行构建：

- 对旧实现开启字体功能，当前分支的 CLI stripped 减少 705,520 字节（-1.59%）。
- FFI dylib stripped 减少 1,306,520 字节（-5.62%）。
- FFI staticlib 减少 2,993,632 字节（-4.20%）。
- 旧代码关闭字体功能与当前分支相比，CLI stripped 只减少 133,656 字节（-0.30%），FFI dylib stripped 减少 231,528 字节（-1.04%）。这只能量化字体关闭后再删除相关实现的体积变化，不能推导主题模型整体成本很小，也不能证明 alpha.6 到当前的主要增量来自字体。

仅就“退役 embedded-font 实现”这个改动切片，性能和体积结果可接受：普通主题渲染没有可见回归，字体实现的体积负担已被移除。此结论不能外推到主题重构整体或 alpha.6 到当前的完整版本区间；后者的对照见下节。R6 仍未关闭，冷启动、内存和最终发布 archive 的跨消费者复测也尚未闭环。

#alpha.6 默认配置到当前版本的整体成本对照（2026-09-23）

用户指出前一轮没有使用主题重构前的版本作为基线，这是正确的。前一节的旧代码对照回答的是“退役 embedded-font 实现是否有回归”，不能代表主题重构整体成本。以下补充正式 `v0.8.0-alpha.6` tag 与当前提交的默认配置对照。

alpha.6 没有主题系统，也没有 `embedded-fonts` feature。它的默认 `merman` feature 是 `complete-svg`，包含 `math`；Math 自己依赖 `ratex-svg/embed-fonts` 以绘制数学公式，这不是主题字体支持。当前 `merman` 默认仍是 `complete-svg`。两边都使用各自的默认 feature 编译 `pipeline` 基准；benchmark 使用 `Renderer::new()` 的默认主题路径，没有显式注入自定义 preset 或 ThemeRecipe。输入相同，Criterion 设为 20 samples、1 秒 warm-up、2 秒 measurement；下表为三轮的中位数。

| 基准 | alpha.6 | 当前 | 变化 |
| --- | ---: | ---: | ---: |
| `end_to_end/flowchart_tiny` | 33.96 µs | 104.64 µs | +208.2% |
| `end_to_end/sequence_medium` | 137.41 µs | 331.73 µs | +141.4% |
| `end_to_end/class_medium` | 578.52 µs | 1.058 ms | +82.9% |
| `render/flowchart_tiny` | 13.31 µs | 30.41 µs | +128.5% |
| `render/sequence_medium` | 37.57 µs | 117.47 µs | +212.7% |

这是明显的 alpha.6 到当前版本性能差异，不能视为噪声，也不能判定为主题单独造成：基准的输入相同，但 SVG 输出字节/hash 已变化；期间还有大量 renderer、字体测量和产品能力变化。严格比较工具因此将大部分 fixture 标成不可比；这组手动精确 benchmark 只作为版本区间的回归信号。现有 alpha.6 U10 决策级证据也确认 Class tiny/medium 存在版本区间回归，但同样没有把原因单独归给主题。其可匹配的阶段数据中，Class parse 仅约 +0.6% 至 +17.9%，Class render 约 +37.9% 至 +62.3%；layout 阶段因投影身份改变未能比较。这把调查重点指向 render 路径，但仍不足以把增量归因给主题，而非同期 renderer 变化。

默认 `merman-cli` release artifact 的单机 macOS arm64 对照：

| 形态 | alpha.6 | 当前 | 变化 |
| --- | ---: | ---: | ---: |
| 原始二进制 | 41,582,304 B | 49,786,864 B | +8,204,560 B（+19.73%） |
| `strip -Sx` 后 | 36,772,040 B | 43,759,592 B | +6,987,552 B（+19.00%） |

两个版本默认 CLI 的 normal dependency closure 都是 317 个 unique package names；集合并非相同。当前新增集合中，`merman-theme-contract` 和 `serde_json_canonicalizer` 属于主题交换路径；`palette_math` 来自同期 `roughr` 依赖更新，其余 workspace package 变化也不能只归给主题。package 数量相同不代表依赖成本为零，也无法解释静态链接后的体积变化。

**修正判断：**前一节证明字体退役本身没有可测的主题 API 或普通 SVG 渲染回归，并减少了体积；但它不能证明“主题重构整体没有性能/体积负担”。按 alpha.6 到当前默认配置的整体结果，CLI 增长约 19%，普通图渲染慢约 1.8–3.1 倍，超出“主题功能应只有少量成本”的直觉范围。由于两个版本的输出语义和实现也变化了，目前应把这视为需要归因的版本区间回归，而不是接受为主题必然成本。下一步应固定同一 Mermaid 输入与输出语义，逐阶段比较 parse/layout/render，并做主题 plumbing 的开/关或可控消融；在完成归因前，R6 的性能和 footprint 仍是未关闭项。

#R1–R15 状态

| 要求 | 状态 | 判断 |
| --- | --- | --- |
| R1 公开 Cyberpunk 三个场景的完整 SVG/PNG/PDF 产品 | 部分满足 | Flowchart、Sequence、XY Chart 有完整的 host-dependent 场景证据和 18 个 native 观察；没有 Portable 声明，也没有当前提交后的最终发布 archive 证据。 |
| R2 preset 与 complete recipe 的等价交换 | 基本满足 | Rust、CLI、Web 和已安装消费者已有 round-trip 与编辑证据；仍需把当前字体边界后的最终候选重建纳入同一归档。 |
| R3 真实应用与支持状态分离 | 满足 | 状态、诊断、target admission、support discovery 和 residual 测试均已存在。 |
| R4 普通 SVG 不支付字体处理成本 | 满足 | 当前 profile 不含主题字体实现；host font 可用；字体字节显式失败；依赖和产物已实测。 |
| R5 普通渲染不执行无请求的 native certification | 基本满足 | 字体拆除分支已删除 absent prepared-ledger/native catalog 路径；仍需将最终候选的 strict/export owner 记录绑定到 release archive。 |
| R6 性能、内存和 footprint 成本闭环 | 部分满足 | 依赖闭包和二进制大小已测；latency、cold start、memory、完整 SVG/PNG/PDF 成本尚未在当前提交后完成同一套 U10 归档。 |
| R7 ABI、版本和未发布 API 边界 | 基本满足 | 生成契约、feature matrix、native ABI 和平台检查通过；需要最终 release-preflight 重新确认。 |
| R8 十个 preset 的基本可用性和限定 qualification | 部分满足 | 十个 preset 可编译，Brutalist/Spotless/Cyberpunk 有命名的 host-dependent qualification；其余七个没有 public qualification cells，基本可用性仍是有限证据。 |
| R9 同一源码的真实安装消费者与 archive replay | 部分满足 | Python、Node、Web、Typst、CLI、C ABI、Apple、Flutter 有历史/当前消费者记录，但字体拆除后的最终统一 archive 尚未重建。 |
| R10 复用 owner、删除重复 machinery | 满足本次切片 | 内嵌字体 runtime、验收和 fuzz machinery 已删除，保留分支承载历史实现。整个主题计划仍有其他未完成 machinery，不属于本次字体边界。 |
| R11 24 个参考主题的 portfolio/application review | 基本满足 | 现有 portfolio 与 Modern Mermaid audit 已完成来源和 family scope 分析；它不是全部主题的 runtime qualification。 |
| R12 family scope 与语义隔离 | 基本满足 | family design metadata、selection UI 和跨 family 保持选择证据存在；未审核 family 仍不应被解释为完整设计。 |
| R13 preset selection、family switch、customization、save/import | 部分满足 | 选择、family switch、完整 recipe 保存/导入和局部编辑已有证据；品牌级复杂 preset 参数、实际字体缺失旅程、binding 结果元数据仍有缺口。 |
| R14 一个可持久化的 canonical exchange | 基本满足 | ThemeRecipeV1 和 direct import 已落地；所有已声明安装消费者的最终同源重建仍需归档。 |
| R15 自定义主题分发与资源/许可说明 | 部分满足 | 文档说明 simple/complete、host fonts、README/LICENSE 和不提供 registry；自定义 recipe 的 identity/design metadata 和完整分发体验仍未定案。 |

#AE1–AE6 状态

- **AE1：部分满足。** Cyberpunk 完整 recipe 在 Flowchart、Sequence、XY Chart 上通过
  host-dependent SVG/PNG/PDF 观察和 fresh-consumer round-trip；当前没有 Portable 或跨主机字体一致性承诺。
- **AE2：满足核心边界。** 无字体资源的最小 SVG 可以渲染，主题字体字节返回明确错误，不会触发已删除的字体处理。绑定层仍没有统一暴露严格 portability policy，这是消费者体验缺口。
- **AE3：基本满足。** effect、terminal、resource-limit 和 strict negative tests 已存在；需要在最终 release archive 上重放一次，避免只依赖 source checkout。
- **AE4：满足。** Clear、transparent、source ownership、复用 engine 和跨请求隔离已有 CLI/consumer tests。
- **AE5：基本满足。** 选择保持、family scope 展示和 unreviewed 范围处理已有 Playground/consumer 证据；所有 family 的视觉完整性没有被声称。
- **AE6：部分满足。** Sequence 语义、Class/ER marker 和 XY 后续 ordinal 有针对性测试；完整跨 family、跨 profile、跨消费者的发布级矩阵仍未关闭。

#验收标准应如何定义

当前主题不应再用“是否能把更多颜色渲染出来”作为验收标准。建议固定为七层：

1. **输入与交换**：recipe/schema/version/duplicate/size/resource errors 可预测，导出文件能被 fresh consumer 直接导入。
2. **语义应用**：每个请求的 facet 有 Applied、Residual、Unsupported、Unverified 或 NotApplicable 的明确结果；空诊断不能自动等于 Portable。
3. **设计范围**：可用性、`family_designs`、实际应用和 qualification 分开；base-only 与 unreviewed 必须保留给用户看见。
4. **输出目标**：SVG、PNG、PDF 分开验证；每个结果绑定 recipe、source、profile、target、host 和字体资源假设。
5. **宿主字体边界**：字体名称可以使用；字体字节必须显式拒绝；不打包替代字体；不承诺跨主机文本像素一致。
6. **负向与资源**：Clear、transparent、source override、unsupported effect、缺失 terminal、超预算、取消和 malformed input 都必须失败或产生真实 residual。
7. **发布闭环**：只在同一源码、同一 profile、同一 recipe fingerprint、同一 consumer/archive 和所需 host evidence 全部匹配时填写 public qualification cells。

#进入发布前仍需关闭的事项

1. 用当前 `d3c895b3e` 重建一次统一 release candidate archive，重新跑已声明的 Web、Node、Python、Typst、C/UniFFI、Apple、Flutter 和 CLI 消费者。
2. 将 U10 结果补齐到同一来源：latency、cold start、memory、SVG/PNG/PDF、依赖闭包和实际包体积；当前的尺寸测量只能关闭 footprint 的一部分。
3. 决定是否为绑定层暴露严格 theme portability policy 和 execution evidence；当前 Rust 核心有能力，部分 Web/WASM 适配器仍只返回 data，丢掉了 metadata。
4. 完成 R13 的真实小品牌编辑和缺失宿主字体旅程，确认错误和 scope 信息对用户可见。
5. 继续保持 preset `qualified_cells` 空或 host-dependent，直到 recipe/profile/source/target/host 的最终证据归档；不要因为 acceptance tests 全绿就把它们提升为 Portable。

#不应再作为当前目标

- 当前产品不应继续要求 `embedded-fonts` feature、WOFF2 decoder、native shaping 或 bundled replacement fonts。
- 当前产品不应把所有十个 preset 或所有 33 个 family 宣称为完整 Modern Mermaid reproduction。
- 当前产品不应把单机二进制下降、依赖包数量下降或 support discovery 数量当作主题质量或 Portable qualification。
- 当前产品不应把历史保留分支的字体验收结果混入当前发布证据。
