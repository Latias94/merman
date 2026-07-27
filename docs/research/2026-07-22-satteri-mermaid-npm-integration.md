# Satteri Mermaid and Merman Node/N-API Comparison

**Originally researched:** 2026-07-22

**Revalidated and expanded:** 2026-07-27

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
- Merman source at `71cb231c84bb5ec296f5bf55b1f999f7c1e68582`, plus the in-progress private
  Node candidate files present in the working tree on 2026-07-27; and
- Merman's dated, reproducible
  [Node transport admission record](../performance/NODE_TRANSPORT_ADMISSION.md).

The Merman candidate is actively changing and its recorded binary predates the final refactor
state. Current API and target observations are therefore implementation-state facts, not release
promises.

## Executive conclusion

1. Satteri solved a real, narrow product problem well: trusted Mermaid fences in a Satteri/Astro
   static site become inline SVG during the build, with no client-side Mermaid runtime. Its
   MDAST-placeholder/HAST-render split is the most reusable part of the design.
2. Satteri is not a general Mermaid Node SDK and does not bind Merman. It is a synchronous SSG
   plugin over `mermaid-rs-renderer` (`mmdr`) through one napi-rs `render()` function.
3. Merman's Node candidate is a substantially broader transport: 35 Mermaid 11.16 families,
   semantic/layout/planning operations, math and two optional layout engines, typed errors,
   deterministic resource profiles, async work, a bounded queue, disposal, and queued
   cancellation. That breadth also makes its current native artifact much larger.
4. There is no valid Satteri-versus-Merman speed winner yet. Satteri's `~3 ms` is an undocumented
   author observation; mmdr's published numbers compare against fresh Chromium process cost; the
   Merman numbers use a different 3,995-case corpus and capability set.
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

Merman's current catalog covers 35 Mermaid 11.16 families. Relative to the mmdr 0.3.1 list, Merman
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

The 2026-07-23 admission run recorded 8,860,772 packed bytes and 22,718,975 installed bytes for the
root plus one macOS arm64 target. Its native file is therefore materially larger than Satteri's
same-host binary. That is expected from a broader implementation, but the current comparison is
also unfairly pessimistic: the standalone Merman candidate manifest has no release LTO/strip
profile, while Satteri explicitly enables LTO and symbol stripping.

Merman's build receipt records the exact Cargo lock digest, dependency closure, artifact hashes,
runtime capability catalog, and operation probes. The recorded artifact contained 195 resolved
packages, but it predates the final refactor and must not be quoted as the current release closure.
Rebuild after enabling release stripping and after the branch stabilizes before setting a size
budget.

The Merman candidate selects Node-API 8, but its JavaScript package does not yet declare
`engines.node` even though it uses APIs such as `structuredClone`. The public minimum Node version
must be chosen, declared, and tested before admission.

## Performance evidence

These measurements describe different workloads and must remain separate:

| Evidence | Workload and result | What it does not prove |
| --- | --- | --- |
| Satteri article | Claims approximately 3 ms per diagram and approximately 0 ms native initialization | No machine, corpus, repetitions, raw samples, or plugin benchmark is published |
| mmdr v0.3.1 README | Reports 2.71-4.67 ms CLI render times and 0.07-2.51 ms library times for four small families on an Intel Linux host | Its 100-1400x headline mostly compares with fresh Puppeteer/Chromium process cost, not warm `mermaid.render()` |
| Merman admission run | On Apple M4 Pro: napi warm p50 0.722 ms, mean 1.354 ms, p95 2.750 ms; cold p50 47.07 ms; peak RSS 238,977,024 bytes across a 3,995-case corpus | Different host, diagrams, options, features, output, and error set; no Satteri result was collected |
| Same-run Merman Node WASM | napi reduced warm mean by 8.7%, cold p50 by 41.2%, and peak RSS by 63.5% relative to the Node-targeted WASM candidate | One macOS arm64 host and stale pre-final-refactor artifacts; no transport was admitted |

The Merman data does support one narrow conclusion: for Merman's own complete static-SVG recipe on
that host, N-API improved cold start and memory substantially, while warm-render latency improved
modestly. It does not show that N-API is universally faster than WASM or that Merman is faster than
mmdr.

A fair follow-up comparison should:

1. use the intersection of the 23 mmdr families and identical source files;
2. separate module import, first render, warm synchronous render, async batch throughput, RSS, and
   installed footprint;
3. record successful/failed inputs and well-formed SVG before comparing time;
4. use only common options, while separately reporting Merman-only math/layout cases;
5. run isolated processes on each shared published target; and
6. publish the corpus digest, raw samples, tool versions, and package hashes.

Do not require SVG byte equality between independent renderers, but do not count a failed or
silently downgraded Satteri diagram as a fast render.

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
5. Add release LTO/stripping to the standalone Node candidate, then rerun dependency, size, RSS,
   and latency evidence. A native transport alone is not a size optimization.
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
- **Moderate:** the standalone candidate lacks release LTO/stripping, so its current 21 MB-class
  native file is not an acceptable final size baseline.
- **Moderate:** Linux arm64 is absent, while the candidate declares targets that have not yet
  passed their required runtime matrix.
- **Evidence gap:** no controlled same-corpus benchmark against Satteri/mmdr exists.

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
No third-party native addon was executed, no external Cargo build was run, and no Satteri
performance number was independently reproduced. Avoiding unreviewed native-code execution means
the Node 18/20 failure mode remains a contract finding rather than an observed runtime failure.

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
