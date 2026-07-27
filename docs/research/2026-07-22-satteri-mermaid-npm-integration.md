# Satteri Mermaid and Merman Node/N-API Comparison

**Originally researched:** 2026-07-22

**Revalidated and expanded:** 2026-07-28

**Status:** design and release evidence; neither Merman Node transport candidate is an admitted
public product.

## Scope and evidence boundary

This report investigates the migration described in
[the Satteri Mermaid article](https://xingwangzhe.fun/posts/satteri-mermaid-npm-package/) and compares
the resulting `@xingwangzhe/satteri-mermaid@0.7.1` package with the current private
`merman-node` / `@mermanjs/node` candidates.

The article is treated as a lead, not as authoritative evidence. Package identity, API, targets,
and size were checked against:

- the immutable
  [`satteri-mermaid` v0.7.1 source](https://github.com/xingwangzhe/satteri-mermaid/tree/53d6be9609ac2fb666dd73f93129e821fd3f95ea);
- the
  [npm registry record](https://registry.npmjs.org/@xingwangzhe%2Fsatteri-mermaid/0.7.1) and packed
  tarball;
- the pinned
  [`mermaid-rs-renderer` v0.3.1 source](https://github.com/1jehuang/mermaid-rs-renderer/tree/2f993bd79a55235eb59a34d807852276ba25bea7);
- the direct Merman/Satteri product-call comparison at Merman source
  `4264f2aad83370a4c16a1d7404154b71d8d16372`;
- the latest Merman N-API/Node-WASM transport comparison at
  `5f540c08db635d4f0ccc8e62429c0e385e95a485`, report SHA-256
  `a5ef6ad899033010209de2ae9cb25ffadad13c3f57fc56cbd351f777e8119226`;
- the isolated Merman native-size experiment at `e311f9e6a`, report SHA-256
  `85293fafb973285aa347f05216ca3a639031b5d1d915e318c51f10ef5691577c`; and
- Merman's dated, reproducible
  [Node transport admission record](../performance/NODE_TRANSPORT_ADMISSION.md).

The Merman candidate is actively changing. Each measurement below is bound to its own source
revision, lockfile, and artifact hash. The direct Satteri lane, current transport lane, and isolated
size experiment are separate evidence and must not be merged into one synthetic result; package
and target observations remain implementation-state facts rather than release promises.

## Executive conclusion

1. Satteri solved a real, narrow product problem well: trusted Mermaid fences in a Satteri/Astro
   static site become inline SVG during the build, with no client-side Mermaid runtime. Its
   MDAST-placeholder/HAST-render split is the most reusable part of the design.
2. Satteri is not a general Mermaid Node SDK and does not bind Merman. It is a synchronous SSG
   plugin over `mermaid-rs-renderer` (`mmdr`) through one napi-rs `render()` function.
3. Merman's Node candidate is a substantially broader transport: a 41-entry Mermaid 11.16 family
   catalog, 35 primary SVG admission records,
   semantic/layout/planning operations, math and two optional layout engines, typed errors,
   deterministic resource profiles, async work, a bounded queue, disposal, and queued
   cancellation. That breadth also makes its current native artifact much larger.
4. A direct 30-fixture synchronous N-API comparison now exists. Merman wins 13 and Satteri 17;
   the median Merman/Satteri ratio is 1.247x, or 24.7% more Merman time, while the 0.458x geometric
   mean favors Merman because complex Flowchart wins dominate. This is workload evidence, not a
   universal winner or an output-equivalence claim.
5. The published Satteri package has several verifiable contract defects: a missing CommonJS export
   file, a Node engine declaration inconsistent with its selected Node-API level, platform and
   diagram-list drift, unsafe URI schemes in generated links, silent render-error fallback, and
   size claims that do not match the tarball.
6. Merman should borrow Satteri's Markdown integration pattern, not its binding or distribution
   design. Keep the SSG plugin above a general Node engine and use the existing exact
   optional-platform-package topology.

## What Satteri actually ships

`@xingwangzhe/satteri-mermaid@0.7.1` was published on 2026-07-19. The repository was created in June
2026 and the tag contains eight commits, so it is active but very new. Its
[`Cargo.toml`](https://github.com/xingwangzhe/satteri-mermaid/blob/53d6be9609ac2fb666dd73f93129e821fd3f95ea/Cargo.toml)
depends on `mermaid-rs-renderer = "0.3.1"` and napi-rs, while its
[`package.json`](https://github.com/xingwangzhe/satteri-mermaid/blob/53d6be9609ac2fb666dd73f93129e821fd3f95ea/package.json)
declares `satteri >= 0.8.0` as a peer dependency.

The call chain is:

```text
Markdown fence
  -> Satteri MDAST visitor stores source in ctx.data and emits an empty placeholder
  -> other Markdown transforms run without seeing the Mermaid source
  -> Satteri HAST visitor restores the source
  -> synchronous napi-rs render(code, options)
  -> mermaid-rs-renderer 0.3.1 parse/layout/SVG pipeline
  -> raw inline SVG
```

The two-phase source preservation is implemented in
[`src/plugin.ts`](https://github.com/xingwangzhe/satteri-mermaid/blob/53d6be9609ac2fb666dd73f93129e821fd3f95ea/src/plugin.ts).
The native layer in
[`src/lib.rs`](https://github.com/xingwangzhe/satteri-mermaid/blob/53d6be9609ac2fb666dd73f93129e821fd3f95ea/src/lib.rs)
exports one synchronous function and converts every Rust error into a generic JavaScript error
string. There is no renderer instance, async task, worker pool, cancellation token, resource
profile, parse result, layout result, or typed error object.

## API and runtime comparison

| Concern | Satteri Mermaid 0.7.1 | Current Merman Node candidate |
| --- | --- | --- |
| Product state | Published Satteri/Astro plugin | Private admission candidate; not published or supported |
| Rust renderer | `mermaid-rs-renderer` 0.3.1 | Merman 0.8 alpha line, targeting `mermaid@11.16.0` |
| Native bridge | napi-rs, one synchronous `render()` | napi-rs `NativeEngine` plus a Node-targeted WASM control candidate |
| Main API | `mermaidMdast()`, `mermaidHast()`, `renderMermaidSVG()` | `createNodeEngine()`, async/sync SVG helpers, generic typed operations, runtime catalog, `dispose()` |
| Scheduling | Runs on the calling Node thread | napi-rs `AsyncTask` for async work; explicit sync path for SSG |
| Backpressure | None | Configurable `concurrency` and `maxQueue`; typed saturation error |
| Cancellation | None | Removes queued work only; already-running Rust work is not preempted |
| Error behavior | Direct API throws a generic error; plugin catches all errors and silently emits a code block | Versioned wire envelope and typed operation, capability, queue, target, and lifecycle errors |
| Outputs | SVG only | Candidate recipe exposes SVG plus semantic JSON, layout JSON, and SVG capability planning |
| Runtime policy | Implicit native process behavior | Deterministic by default; explicit binding options and runtime catalog |
| Resource policy | None | Seven family-neutral limits with `interactive`, `constrained`, `trusted-native`, and explicit unbounded profiles |
| Module surface | ESM import intended; declared CJS export is broken | ESM-only candidate surface |

Merman's async behavior is visible in
[`napi_transport.rs`](../../crates/merman-node/src/napi_transport.rs), while queue and lifecycle
semantics are owned by
[`bounded-executor.mjs`](../../platforms/node/src/bounded-executor.mjs). Both transports execute the
same binding-core request in [`wire.rs`](../../crates/merman-node/src/wire.rs), rather than
duplicating parse or render behavior in JavaScript.

This extra machinery is useful for editors, servers, and batch tools, but a one-off trusted static
blog build may reasonably prefer Satteri's much smaller synchronous API.

## Diagram and rendering capability

The authoritative `mermaid-rs-renderer` 0.3.1
[`DiagramKind`](https://github.com/1jehuang/mermaid-rs-renderer/blob/2f993bd79a55235eb59a34d807852276ba25bea7/src/ir.rs)
contains 23 families:

`flowchart`, `sequence`, `class`, `state`, `er`, `pie`, `mindmap`, `journey`, `timeline`, `gantt`,
`requirement`, `gitgraph`, `c4`, `sankey`, `quadrant`, `zenuml`, `block`, `packet`, `kanban`,
`architecture`, `radar`, `treemap`, and `xychart`.

Satteri does not add diagram implementations; it forwards source to that parser. Its README list is
not authoritative: it contains 24 names, includes unsupported `info` and `venn`, and omits supported
`zenuml`. The article's statement that the native version lost `xychart` is also stale for the
published 0.7.1 dependency. Because the HAST plugin catches render errors, an unsupported family is
quietly restored as a raw code block instead of failing the build.

Merman's current catalog contains 41 Mermaid 11.16 entries and its primary SVG inventory contains
35 admission records. Relative to the mmdr 0.3.1 list, Merman
also includes `swimlane`, `info`, `eventmodeling`, `treeView`, `ishikawa`, four Railroad dialects,
`venn`, `wardley`, and `cynefin`. The catalog is owned by
[`family.rs`](../../crates/merman-core/src/family.rs), and Node exposes the count through its runtime
catalog instead of maintaining a package-local list.

The capability sets still are not equivalent:

- Merman's Node recipe includes SVG, Cytoscape and ELK layouts, and math rendering.
- Satteri exposes mmdr's five theme presets, selected colors, typography, spacing, aspect ratio,
  and approximate text metrics.
- Satteri's claim of "full mermaid-rs parameter coverage" is too broad. The wrapper does not expose
  mmdr's complete nested configuration, and it has no general node-border-width option despite the
  article's border-width claim.
- mmdr explicitly describes itself as early development whose visual output may not match
  Mermaid CLI. It does not declare a pinned Mermaid semantic compatibility release in its
  [v0.3.1 README](https://github.com/1jehuang/mermaid-rs-renderer/blob/2f993bd79a55235eb59a34d807852276ba25bea7/README.md).
  Merman's family and SVG evidence is instead organized around the pinned Mermaid 11.16 baseline.

## Distribution, ABI, and dependency closure

### Published Satteri package

The npm tarball was measured on 2026-07-27 with `npm pack
@xingwangzhe/satteri-mermaid@0.7.1 --json`:

| Measure | Result |
| --- | ---: |
| Compressed tarball | 8,310,468 bytes |
| Registry unpacked size | 19,931,056 bytes |
| Files | 13 |
| macOS arm64 `.node` | 4,206,544 bytes |
| Linux arm64 glibc `.node` | 4,528,624 bytes |
| Linux x64 glibc `.node` | 5,392,952 bytes |
| Windows x64 `.node` | 5,768,704 bytes |

All four binaries live in the root package, so every supported user installs roughly 19.9 MB. The
article's `<1 MB` native-binary claim does not match any published 0.7.1 binary.

Actual binary coverage is macOS arm64, Linux x64/arm64 glibc, and Windows x64. The README additionally
claims macOS x64, while the loader additionally advertises Windows arm64; neither artifact exists.
There is no musl build.

The package declares Node `>=18.17.0`, but
[`Cargo.toml`](https://github.com/xingwangzhe/satteri-mermaid/blob/53d6be9609ac2fb666dd73f93129e821fd3f95ea/Cargo.toml)
selects napi-rs `napi10`. The official
[Node-API version matrix](https://nodejs.org/api/n-api.html#node-api-version-matrix) places Node-API
10 at Node 22.14.0+, while Node 18.17.0 introduces Node-API 9. The current binding may happen not to
exercise every version-10 call, but the declared engine floor and compiled API choice are not a
defensible compatibility contract without a Node 18/20 runtime matrix.

The tarball has npm provenance, which is positive. Rust reproducibility is weaker: the tag commits
no `Cargo.lock`, uses semver ranges for napi-rs and mmdr, and the release workflow builds without a
recorded Rust dependency closure. It also enables mmdr's default `cli` and `png` features even
though the binding exports only SVG; mmdr's own embedding guidance recommends disabling those
defaults.

### Merman candidate package

Merman uses a thin ESM root package with exact optional dependencies on one package per target.
The current declared matrix is macOS x64/arm64, Linux x64 glibc/musl, and Windows x64. It lacks
Linux arm64, which Satteri does ship. Each platform package declares exact `os`, `cpu`, and, on
Linux, `libc` metadata in [`package-surfaces.json`](../../platforms/node/package-surfaces.json).

The source-bound artifact used by the final transport run is 21,223,312 raw bytes. Satteri's
same-host binary is 4,206,544 bytes, so the individual Merman binary is 5.05x larger. The npm
distribution boundary is much closer: Merman's root plus matching platform package measured
8,966,029 packed and 22,906,407 installed bytes, while Satteri's all-platform root tarball is
8,310,468 packed and 19,931,056 unpacked bytes. Satteri has the smaller binary; Merman avoids
installing unrelated target binaries.

The isolated `e311f9e6a` capability-leaf experiment decomposes Merman's own artifact:

| Merman N-API lane | Raw bytes | Gzip `-9` bytes | Normal packages |
| --- | ---: | ---: | ---: |
| SVG only | 15,771,392 | 6,563,389 | 132 |
| SVG + Cytoscape | 16,184,784 | 6,760,083 | 133 |
| SVG + ELK | 17,028,528 | 7,089,266 | 134 |
| SVG + math | 19,635,312 | 8,112,389 | 186 |
| Complete SVG | 21,256,336 | 8,819,018 | 189 |
| Satteri 0.7.1 | 4,206,544 | 1,822,210 | unknown |

Within Merman, independent Cytoscape, ELK, and math lanes add 413,392, 1,257,136, and 3,863,920
bytes. Their arithmetic sum is 5,534,448 bytes, while the combined complete-over-SVG increase is
5,484,944 bytes because the linked combination shares 49,504 bytes. This is not a like-for-like
decomposition of the Merman/Satteri gap. The products report 41 and 23 diagram-family catalogs
using unverified classification parity, and the experiment does not separately measure bridge
code. Even Merman's SVG-only lane is 3.75x Satteri, so the residual cause remains open.

The gap is code and static data, not an unstripped-symbol accident. Both complete binaries expose
one global symbol. In the matched size experiment, Merman contained 13,336,980 bytes of `__text`
and 5,346,000 bytes of constant sections, versus Satteri's 3,176,372 and 552,308 bytes. After the
strongest tested profile, code plus constants still explained 90.46% of the remaining raw gap.

Satteri 0.7.1 declares fat LTO and symbol stripping; its release sources do not declare one release
codegen unit. Merman's canonical napi build already passes `--strip`, which napi CLI 3.7.4
implements with linker `-s`. The isolated Merman profile matrix measured:

| Complete-SVG profile | Raw bytes | Change from default |
| --- | ---: | ---: |
| Cargo release defaults + napi CLI `--strip` | 21,256,336 | baseline |
| Fat LTO | 21,239,824 | -0.08% |
| Fat LTO + one codegen unit | 18,998,416 | -10.62% |
| Above + Cargo `strip = "symbols"` | 18,998,416 | no further saving |

On this Darwin arm64 lane, adding Cargo `strip = "symbols"` did not reduce raw bytes after the
existing linker strip. It still changed the binary and increased gzip output by 2,735 bytes, so
this does not prove byte equivalence or cross-target redundancy. The
[rustc codegen guidance](https://doc.rust-lang.org/rustc/codegen-options/index.html#strip) documents
the general debugging trade-off; this experiment did not observe a further diagnostics loss. Fat
LTO plus one codegen unit is the measured release-profile candidate, but it still needs
cross-target correctness, latency, memory, and build-time acceptance before becoming policy.

Merman's build receipt records the exact Cargo lock digest, dependency closure, artifact hashes,
runtime capability catalog, and operation probes. The size experiment's 189-package complete lane
is pinned evidence for that revision, not a release promise or proof that every byte is necessary.

The Merman candidate selects Node-API 8, but its JavaScript package does not yet declare
`engines.node` even though it uses APIs such as `structuredClone`. The public minimum Node version
must be chosen, declared, and tested before admission.

## Performance evidence

These measurements use different lanes and must not be collapsed into one speed score.

### Merman N-API versus Node-WASM

The final schema-2 transport run used the same 4,001-case trusted corpus, binding options, complete
static-SVG recipe, product facade, and measured source on Apple M4 Pro:

| Measure | Node-WASM | N-API | N-API effect |
| --- | ---: | ---: | ---: |
| Warm successful-SVG p50 | 0.3189 ms | 0.2903 ms | -9.0% |
| Warm successful-SVG p95 | 1.6305 ms | 1.3467 ms | -17.4% |
| Warm successful-SVG mean | 0.9266 ms | 0.8117 ms | -12.4% |
| Engine init through first SVG p50 | 96.05 ms | 7.39 ms | 12.99x faster |
| Parent process through result p50 | 137.74 ms | 47.39 ms | 2.91x faster |
| Four-request batch p50 | 1.4387 ms | 0.2418 ms | 5.95x faster |
| Peak RSS | 638,189,568 B | 240,648,192 B | -62.3% |
| Packed / installed | 6,157,111 / 17,784,897 B | 8,966,029 / 22,906,407 B | +45.6% / +28.8% |

The warm boundary includes the public facade call plus SHA-256 and byte-length evidence projection.
Cold and concurrent boundaries stop before that projection, so only the warm rows include evidence
bookkeeping. These are harness-level operation timings, not isolated renderer CPU measurements.

All 4,001 semantic or typed-error outcomes match, as do all SVG structure signatures. The corpus
contains 3,897 successful SVGs and 104 matching typed failures. Exact geometry and raw bytes differ
for 426 successful SVGs. Their cause is unattributed; the current validator reports them but does
not make them an admission gate. The decision remains inconclusive because only macOS arm64 has
runtime/install evidence. The five concurrency batches are directional evidence, not a
cross-target throughput claim.

### Merman N-API versus Satteri N-API

The direct comparison used `MermanNodeEngine.renderSvgSync(source)` and Satteri's
`renderMermaidSVG(source, {})` over 30 shared fixtures. It performed three warmups, six alternating
AB/BA rounds, equal calibrated iteration counts, and 283,656 raw timed calls. Both candidates
returned well-formed SVG for all 30 inputs. The harness passed the same source string to both
public facades; Satteri's wrapper then applied `String.trim()` and removed one trailing LF from
every fixture. The benchmark preserves that public-wrapper behavior instead of bypassing it.
The selection reuses the native comparison's 30 ratio-eligible standard fixtures; it excludes
`flowchart_large`, Info, Treemap, and XYChart because the native reference was missing or used
different fixture bytes.

| Aggregate | Result |
| --- | ---: |
| Merman faster / Satteri faster | 13 / 17 |
| Median fixture Merman / Satteri | 1.247x |
| Geometric mean Merman / Satteri | 0.458x |

The median fixture favors Satteri by 24.7%, while the geometric mean favors Merman by about 2.18x
because complex Flowchart cases dominate. For example, `flowchart_ports_heavy` measured 1.54 ms in
Merman and 471.28 ms in Satteri, while `mindmap_medium` measured 294.31 us versus 83.33 us and
`requirement_medium` measured 254.63 us versus 80.33 us. This is not a universal winner.

The direct run proves successful public-call execution and timing from the same source arguments;
it does not prove byte-identical renderer inputs, equivalent SVG geometry, DOM, theme, or Mermaid
semantics. The independent native Criterion
[checkpoint](../performance/renderer_comparison_2026-07-28_75c9fd156_vs_mmdr.md) adds strict
byte-identical fixture gating: Merman leads 18 of 30 comparable rows, with a 0.664x median and
0.297x geometric mean. It identifies Requirement and Mindmap as the only mmdr leads above both the
10% relative and 50 us absolute triage thresholds.

The correctness receipt recorded different raw, structure, and geometry digests for all 30 direct
outputs; Merman emitted 669,856 bytes in total and Satteri 335,730 bytes. Both products use napi-rs,
and the separate Merman transport run does not show a broad local latency penalty for its N-API
candidate. That control also changes target and runtime, however, so it is not an isolated bridge
A/B. The direct Satteri run measures two complete product call stacks: JS facade, marshalling,
binding, renderer, allocation, build profile, and different output all remain confounders. A fair
next step is stage attribution and visual/DOM comparison for the slow families, plus the same
transport run on every shipped target. Satteri embeds mmdr 0.3.1, while the native Criterion
checkpoint pins later revision `7ff1196`; it also measures direct Rust rather than the two public
N-API facades. Do not merge or average those aggregates.

## Security and reliability boundary

Satteri is appropriate only for trusted build input in its current form.

`mermaid-rs-renderer` escapes XML attribute syntax, but its
[`parse_click_line`](https://github.com/1jehuang/mermaid-rs-renderer/blob/2f993bd79a55235eb59a34d807852276ba25bea7/src/parser.rs)
accepts arbitrary URI schemes and its
[`link_attrs`](https://github.com/1jehuang/mermaid-rs-renderer/blob/2f993bd79a55235eb59a34d807852276ba25bea7/src/render.rs)
serializes them into both `href` and `xlink:href`. Therefore a `javascript:` click target can reach
the inline SVG. The plugin adds no sanitizer after rendering.

It also has no source/model/layout/output budgets, timeout, cancellation, or concurrency limit.
Finally, `replaceWithSVG()` catches every render error without logging and emits a raw Mermaid code
block. That keeps a personal blog build alive, but it hides unsupported syntax and can publish a
page that depends on client Mermaid even when the site did not include it.

Merman's default strict rendering paths sanitize dangerous link schemes and its binding contract
exposes limits for source bytes, semantic items/text/depth, layout work, SVG bytes, and SVG
elements. These are workload controls, not a sandbox. As
[`OPTIONS_JSON.md`](../bindings/OPTIONS_JSON.md) states, public or multi-tenant services still need
host timeouts, memory and concurrency quotas, and process-level preemption or isolation.

## Use-case decision table

| User need | Best current choice | Reason |
| --- | --- | --- |
| Trusted Sätteri/Astro static blog on a published target | Satteri, after accepting or fixing the listed package/security issues | Turnkey MDAST/HAST integration and zero client JS |
| Generic Node SSG today | Neither Merman candidate is public; use a controlled Rust/CLI integration or a reviewed adapter | Do not present private admission code as a supported package |
| Editor preview, lint, semantic tooling, or layout inspection | Merman architecture once Node is admitted | Typed semantic/layout operations, async queue, lifecycle, and resource catalog |
| Public or multi-tenant rendering | Merman with `constrained` policy plus outer isolation; Satteri is not suitable as published | Satteri has no budgets and passes unsafe URI schemes |
| Need `info`, Venn, Swimlane, Wardley, Cynefin, Event Modeling, TreeView, Ishikawa, or Railroad | Merman | These are outside mmdr 0.3.1's 23-family implementation |
| Need Linux arm64 glibc now | Satteri | Merman's current candidate matrix lacks this target |
| Need macOS x64 or Linux x64 musl | Merman candidate topology, after admission | Satteri publishes neither binary |
| Small trusted SVG-only native binding | Satteri/mmdr has the smaller current native binary | Merman's complete recipe pays for broader semantics, layout, math, and policy |

## What Merman should borrow

1. Add a separate Satteri/Markdown integration package only if demand exists. Preserve Mermaid
   source in MDAST data and render after transformations in HAST; do not put Markdown-specific
   behavior in the Node transport.
2. Offer a concise synchronous helper for explicit SSG builds alongside the engine API. Keep the
   async engine as the server/editor default.
3. Make fallback an explicit policy such as `onError: "fail" | "warn-and-code" | "code"`, with
   failure as the release/build default. Never swallow a typed renderer error silently.
4. Keep exact per-target optional packages. Do not aggregate every native binary into the root
   tarball, and do not add postinstall downloads.
5. Evaluate fat LTO plus one codegen unit as the measured release-profile candidate. Do not adopt
   Cargo symbol stripping from one Darwin raw-size result; rerun cross-target size, compression,
   latency, RSS, build-time, and diagnostics evidence.
6. Declare and test a minimum Node version. Keep the napi-rs feature level at or below that
   contract.
7. Run lightweight loader/API/package contract tests on ordinary changes. Reserve the full corpus,
   cross-platform install matrix, and comparative performance harness for admission and release
   gates rather than every PR.

## Actionable findings

### Satteri package

- **High:** `exports.require` points to `dist/index.cjs`, but the published tarball contains no such
  file. CommonJS package loading is broken.
- **High:** `engines.node >=18.17.0` conflicts with the selected `napi10` build contract; Node 18 and
  20 support must not be claimed without correction and runtime evidence.
- **High:** arbitrary click URI schemes, including `javascript:`, are serialized into inline SVG.
- **High:** render failures are swallowed and silently converted to code blocks.
- **Moderate:** README/platform claims exceed the actual binaries; the loader also advertises a
  missing Windows arm64 binary.
- **Moderate:** diagram documentation is internally inconsistent and disagrees with mmdr 0.3.1.
- **Moderate:** `<1 MB`, general border-width control, and complete parameter-coverage claims do not
  match the published artifact or binding source.
- **Moderate:** the release has npm provenance but no locked or recorded Rust dependency closure.

### Merman candidate

- **High before admission:** the public Node minimum version is undeclared and only macOS arm64 has
  runtime/install evidence.
- **Moderate:** fat LTO plus one codegen unit saves 10.62% in the isolated size experiment, but it
  still needs cross-target latency, correctness, memory, and build-time admission.
- **Moderate:** Linux arm64 is absent, while the candidate declares targets that have not yet
  passed their required runtime matrix.
- **Moderate:** 426 of 3,897 successful Node transport SVGs have exact geometry and raw-byte drift
  between N-API and Node-WASM; structure and semantic outcomes match, but the cause is unresolved.

## Reproduction notes

Commands used for this 2026-07-27 audit included:

```console
git clone --depth 1 --branch v0.7.1 https://github.com/xingwangzhe/satteri-mermaid.git
git clone --depth 1 --branch v0.3.1 https://github.com/1jehuang/mermaid-rs-renderer.git
npm view @xingwangzhe/satteri-mermaid@0.7.1 ... --json
npm pack @xingwangzhe/satteri-mermaid@0.7.1 --json
tar -xzf xingwangzhe-satteri-mermaid-0.7.1.tgz
file package/*.node
otool -L package/mermaid-rs.darwin-arm64.node
xcrun dyld_info -exports package/mermaid-rs.darwin-arm64.node
strings package/mermaid-rs.darwin-arm64.node
shasum -a 256 xingwangzhe-satteri-mermaid-0.7.1.tgz package/*.node
```

The downloaded tarball SHA-256 was
`5e1b9a21f1c85a90c376d0eee2dd9d6e8ce9c28e3df261acf665d9f5a4866535`.
The direct Satteri timing report has SHA-256
`89e1eaeeaba8a45f9e6fa84989816efc5bdef8d828ff208f1b80eacd4493b00f`; its correctness dry run has
SHA-256 `554d3764a66754e2503e1f0ae3dd11a2e4295f32aac07b513dca7455b4b811f0`.
The final Merman transport report has SHA-256
`a5ef6ad899033010209de2ae9cb25ffadad13c3f57fc56cbd351f777e8119226`.
The current `75c9fd156` mmdr comparison JSON has SHA-256
`7a4099daa933c964267367e27fc162dd1cdf47a4f95f1bde55e587f28928e000`.

The Satteri addon was executed only in the isolated benchmark worktree. Its Node 18/20 compatibility
finding remains source-contract evidence rather than an observed runtime failure because the
benchmark host used Node 26.5.0.

## Primary source inventory

- [Satteri article](https://xingwangzhe.fun/posts/satteri-mermaid-npm-package/)
- [`satteri-mermaid` v0.7.1 source](https://github.com/xingwangzhe/satteri-mermaid/tree/53d6be9609ac2fb666dd73f93129e821fd3f95ea)
- [`satteri-mermaid` release workflow](https://github.com/xingwangzhe/satteri-mermaid/blob/53d6be9609ac2fb666dd73f93129e821fd3f95ea/.github/workflows/release.yml)
- [`@xingwangzhe/satteri-mermaid@0.7.1` registry metadata](https://registry.npmjs.org/@xingwangzhe%2Fsatteri-mermaid/0.7.1)
- [`mermaid-rs-renderer` v0.3.1 source](https://github.com/1jehuang/mermaid-rs-renderer/tree/2f993bd79a55235eb59a34d807852276ba25bea7)
- [Node-API version matrix](https://nodejs.org/api/n-api.html#node-api-version-matrix)
- [Merman Node candidate README](../../platforms/node/README.md)
- [Merman Node transport admission evidence](../performance/NODE_TRANSPORT_ADMISSION.md)
- [Merman Node package surface](../../platforms/node/package-surfaces.json)
- [Merman binding options and resource contract](../bindings/OPTIONS_JSON.md)
