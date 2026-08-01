# Audit of the Satteri `@mermanjs/web` WASM Claims

**Research value: high** -- Immutable package sources and tarballs expose the exact call paths, and
small same-host probes distinguish cold initialization from warm rendering well enough to reject
several categorical claims in the article.

**Audited:** 2026-07-27

**Article snapshot:** [Markdown source](https://xingwangzhe.fun/posts/satteri-mermaid-npm-package.md)
retrieved at 2026-07-27 21:20 CST. The page is dated 2026-07-19 but returned
`Last-Modified: Sun, 26 Jul 2026 23:01:50 GMT`; it is therefore a mutable snapshot, not an immutable
publication artifact.

**Merman snapshots:**

- published `@mermanjs/web@0.7.0`, commit
  [`00a2f291`](https://github.com/Latias94/merman/tree/00a2f291025913d612b6ca1ce9b6df6daf5bef94),
  which is what Satteri 0.5 actually locked;
- the 2026-07-27 browser-artifact experiment, based on `869a26d6`, versioned
  `0.8.0-alpha.4`, with in-progress Web-package changes present;
- the latest private Node transport artifact and schema-2 harness at `5f540c08d`, the direct
  Merman/Satteri timing artifact at `4264f2aad`, and the isolated native-size experiment at
  `e311f9e6a`; and
- Mermaid
  [`11.16.0` at `7c0cafcf`](https://github.com/mermaid-js/mermaid/tree/7c0cafcf42e76bfaf79d0cbbd12edb986612f014),
  the current repository baseline.

## Executive verdict

The article accurately reproduces Satteri 0.5's own Node-side WASM-loading workaround and roughly
reports the WASM artifact size. It does not establish that WASM costs 50 ms per warm diagram, that
Merman initialization inherently costs 300 ms, that Merman border width is unconfigurable, or that
Merman has only 7 themes and 16 color controls. Most of those numbers describe Satteri's narrowed
wrapper or mix cold and warm phases, rather than the `@mermanjs/web@0.7.0` contract.

The labels below have deliberately narrow meanings:

- **correct then**: supported by the package/source snapshot used by the article;
- **correct now**: supported by the current repository contract;
- **stale**: described an old integration but not the current repository contract; and
- **invalid framing**: compares different phases, categories, runtimes, or capability surfaces.

| Article claim | At the article snapshot | Current repository snapshot | Classification |
| --- | --- | --- | --- |
| WASM startup scans `node_modules`, synchronously reads 8.8 MB, and compiles it | That is the exact Satteri 0.5 wrapper path. It is not an intrinsic Merman call path. | Browser packages resolve a package-relative WASM URL and reject Node, Bun, and Deno before a custom loader runs. | **Correct then** for Satteri; **stale** for current Merman |
| WASM initialization is about 300 ms | No machine, samples, or phase boundary were published. Fifteen fresh Node processes measured 7.17 ms compile, 0.84 ms instantiate, and 45.96 ms through first render at p50; the outer process median was 87.87 ms. | Seven fresh browser realms measured 8.5 ms import and 22.9 ms initialization for current full Web. Network transfer and browser startup were excluded. | **Not reproduced** and **invalid as a package constant** |
| WASM renders one diagram in about 50 ms | The historical first render was 35.76 ms, but its warmed p50 was 0.212 ms on the same fixture. | Current browser first render is 49.5 ms for full and 48.8 ms for render; warmed p50 is 0.273 ms for both. | **Approximately correct for first render; false for warm render** |
| Border width is hardcoded to 1 px and cannot be changed | The default is 1 px, but 0.7.0 accepted `themeCSS`, `svg.scoped_css`, and Mermaid source styles. The Satteri wrapper did not forward those options. | The same override paths remain in the current contract, and `strokeWidth` is also covered by current theme tests. | Incorrect then and now |
| Mermaid / Merman / native have 5 / 7 / 5 theme presets | The 7 values were Merman host/editor presets. Merman 0.7 separately exposed 11 Mermaid themes. The native renderer did expose 5 presets. | Merman still exposes 11 Mermaid themes plus 7 host presets. Mermaid 11.16 source also registers 11 themes even though its theming guide still lists only 5. | **Invalid framing** |
| Merman has 16 color roles; native has 60+ parameters with full coverage | Satteri 0.5 declared 22 host roles, while Merman also accepted the full site config and upstream-derived theme-variable surface. Native Satteri had 64 direct color-string slots, heavily concentrated in Git and Pie, not full Mermaid-family coverage. | Current Merman has 22 host roles, a 271-key pinned default theme-variable object, explicit site config, and CSS escape hatches. Major colors are configurable; universal per-family color coverage is not proven. | Numerically false for Merman; "full coverage" is **invalid framing** |
| Native `.node` is below 1 MB | The exact macOS arm64 artifact in Satteri 0.7.0 is 4,206,544 bytes; the other published targets are 4.53-5.77 MB. | The source-bound private Merman macOS arm64 N-API artifact measured 21,223,312 bytes. It is not a published package. | Incorrect for the published Satteri package |

One present-day qualification matters: the
[npm registry metadata](https://registry.npmjs.org/%40mermanjs%2Fweb) still tags `0.7.0` as
`latest` and `0.8.0-alpha.3` as `alpha`; the repository's `0.8.0-alpha.4` browser-only package group
has not been published. The old Node workaround can therefore still work against the currently
published stable tarball even though it is contrary to the next repository contract.

## Version ledger

| Subject | Exact evidence | Consequence |
| --- | --- | --- |
| Satteri WASM generation | [`@xingwangzhe/satteri-mermaid@0.5.0`](https://registry.npmjs.org/%40xingwangzhe%2Fsatteri-mermaid/0.5.0), git head [`73ef075f`](https://github.com/xingwangzhe/satteri-mermaid/tree/73ef075f8a0a52c8683c582cf787ae2fac3b8502), published 2026-07-18 10:14 UTC | Its [`package.json`](https://github.com/xingwangzhe/satteri-mermaid/blob/73ef075f8a0a52c8683c582cf787ae2fac3b8502/package.json) requested `@mermanjs/web ^0.7.0`; [`bun.lock`](https://github.com/xingwangzhe/satteri-mermaid/blob/73ef075f8a0a52c8683c582cf787ae2fac3b8502/bun.lock) resolved exactly `0.7.0`. |
| Merman used by Satteri | [`@mermanjs/web@0.7.0` registry record](https://registry.npmjs.org/@mermanjs%2Fweb/0.7.0), git head `00a2f291`, published 2026-06-09 | The tarball's WASM is 8,685,215 bytes and registry unpacked size is 8,771,195 bytes. "8.8 MB WASM" is a reasonable decimal approximation only if it refers loosely to the artifact/package. |
| Merman semantic baseline then | [`REPOS.lock.json`](https://github.com/Latias94/merman/blob/00a2f291025913d612b6ca1ce9b6df6daf5bef94/tools/upstreams/REPOS.lock.json) at the 0.7.0 commit | Merman 0.7 targeted Mermaid `11.15.0`, commit `41646dfd`. |
| Native comparator in the article | Satteri [`0.7.0`](https://registry.npmjs.org/%40xingwangzhe%2Fsatteri-mermaid/0.7.0) at [`7dfe1bec`](https://github.com/xingwangzhe/satteri-mermaid/tree/7dfe1beccd57c88d38fc1502593c911943aa928a), published 2026-07-19 08:53 UTC | Its [`Cargo.toml`](https://github.com/xingwangzhe/satteri-mermaid/blob/7dfe1beccd57c88d38fc1502593c911943aa928a/Cargo.toml) selected `mermaid-rs-renderer = "0.3.1"`, tag [`2f993bd7`](https://github.com/1jehuang/mermaid-rs-renderer/tree/2f993bd79a55235eb59a34d807852276ba25bea7). |
| Article's browser Mermaid | CDN import `mermaid@11` in the article | A moving major tag does not identify the Mermaid minor, tarball, or code executed for the reported 200-500 ms observation. |
| Current Merman source | [`Cargo.toml`](../../Cargo.toml) and [`REPOS.lock.json`](../../tools/upstreams/REPOS.lock.json) | The working tree is `0.8.0-alpha.4` and targets Mermaid `11.16.0` at `7c0cafcf`; neither fact proves npm publication. |

Using stable Merman 0.7.0 in Satteri was defensible: it was npm `latest` on 2026-07-19. Merman
`0.8.0-alpha.3` already existed, but comparing a consumer's stable dependency with an alpha would
not automatically be fairer. The methodology, not that version choice, is the principal defect.

## 1. Cold initialization and call path

Satteri 0.5's
[`renderer.ts`](https://github.com/xingwangzhe/satteri-mermaid/blob/73ef075f8a0a52c8683c582cf787ae2fac3b8502/src/renderer.ts)
does exactly this:

```text
module import
  -> top-level await initRenderer()
  -> scan at most 20 ancestors of process.cwd()
  -> readFileSync(merman_wasm_bg.wasm)
  -> WebAssembly.compile(bytes)
  -> initMerman({ loader, wasm: compiledModule })
  -> cache completion in both Satteri and @mermanjs/web
  -> later renderSvg() calls are synchronous
```

The article is therefore correct about its wrapper's file lookup, synchronous read, and explicit
compile. It overgeneralizes that integration code into a Merman property:

- Satteri's `initPromise` prevents repeated wrapper initialization.
- Merman 0.7's own
  [`initMerman()`](https://github.com/Latias94/merman/blob/00a2f291025913d612b6ca1ce9b6df6daf5bef94/platforms/web/src/index.ts)
  also caches the initialized module and shares an in-flight promise.
- The current package generator creates `MERMAN_WASM_URL` with `new URL(..., import.meta.url)` and
  imports package-local glue; it does not search the filesystem. See
  [`build-surface-packages.mjs`](../../platforms/web/scripts/build-surface-packages.mjs).
- More importantly, the current [`surface-runtime.ts`](../../platforms/web/src/surface-runtime.ts)
  rejects Node, Bun, and Deno before invoking even a caller-supplied loader. The current
  [`Web README`](../../platforms/web/README.md) reserves any future Node transport for a separate
  package.

The 300 ms number remains possible as an author-local observation on another machine, but the
article publishes no hardware, runtime, repeated samples, or definition of initialization. It
cannot be treated as a package constant.

## 2. Warm per-diagram rendering

The article labels approximately 50 ms as "single-diagram render time," but does not say whether it
is the first call after instantiation, a warmed call, or an end-to-end plugin/build measurement.
That distinction changes the result by more than an order of magnitude.

The phase-classification experiment used:

- macOS 26.5.1, Apple M4 Pro, arm64, 51,539,607,552 bytes RAM;
- Node 26.5.0;
- Microsoft Edge 150.0.4078.99 through Playwright 1.61.1 for current browser packages;
- the immutable `@mermanjs/web@0.7.0` npm tarball and current working-tree Web artifacts;
- the same three-node flowchart source:

  ```mermaid
  flowchart TD
    A[Start] --> B[Process]
    B --> C[Done]
  ```

- 15 fresh Node processes for the historical package;
- seven fresh browser realms for each current package; and
- calibrated warm batches with 12 raw per-render samples.

The historical row reproduces the Satteri 0.5 Node integration. The current rows use a no-store
local HTTP origin and execute inside a real browser. Browser startup, network transfer, and remote
cache behavior are deliberately outside the measured boundaries.

| p50 measurement | Historical `@mermanjs/web@0.7.0` in Node | Current `@mermanjs/web` in Edge | Current `@mermanjs/web-render` in Edge |
| --- | ---: | ---: | ---: |
| API/module import | 0.427 ms | 8.5 ms | 7.4 ms |
| WASM file read | 0.940 ms | included in browser initialization | included in browser initialization |
| `WebAssembly.compile` | 7.174 ms | included in browser initialization | included in browser initialization |
| WASM initialization after compile/import | 0.840 ms | 22.9 ms | 20.3 ms |
| First render | 35.764 ms | 49.5 ms | 48.8 ms |
| Warm render | 0.212 ms | 0.273 ms | 0.274 ms |
| Fresh Node outer process | 87.871 ms | not applicable | not applicable |

The current full and render artifacts were 12,364,163 and 11,689,729 bytes respectively. Their
warm latency is indistinguishable at this scale; the render package saves approximately 0.67 MB
because it removes analysis, editor, and ASCII APIs while retaining the complete SVG, Cytoscape,
ELK, and math contract. These are working-tree artifacts bound by source and input digests, not
release-tag builds.

The measurements do **not** establish a general Merman performance win. One small flowchart does
not represent all families, the historical package ran in Node while the current packages ran in a
browser, and the current candidate has a broader capability closure. They establish three narrower
points:

1. the article's 50 ms figure is not a defensible warm-render constant; and
2. the current browser package still has an approximately 50 ms first-render cost on this host,
   while repeated renders are approximately 0.27 ms; and
3. the published 300 ms initialization claim was not reproduced even when process overhead was
   included.

The Merman 0.7
[`benchmark guidance`](https://github.com/Latias94/merman/blob/00a2f291025913d612b6ca1ce9b6df6daf5bef94/platforms/web/README.md)
requires both browser renderers to be initialized, then compares the same fixtures, themes,
viewport, warmups, and measurement windows in one browser. It explicitly keeps native CLI
benchmarks separate. The article instead places browser user-perceived latency, Node WASM
build-time work, and Node N-API build-time work in one row.

## 3. Border width

There is a kernel of truth: Merman 0.7's generated flowchart stylesheet used a 1 px default for
ordinary node and edge strokes. "Default is 1 px" is not equivalent to "hardcoded and impossible to
override."

The 0.7.0 binding contract already documented both:

- Mermaid `site_config.themeCSS`; and
- post-render `svg.scoped_css`.

The immutable
[`OPTIONS_JSON.md`](https://github.com/Latias94/merman/blob/00a2f291025913d612b6ca1ce9b6df6daf5bef94/docs/bindings/OPTIONS_JSON.md)
uses `.node rect { stroke-width: 2px; }` as the example for each path. A direct tarball probe also
confirmed:

```text
svg.scoped_css ".node rect { stroke-width: 7px; }"  -> present in output: true
site_config.themeCSS "... stroke-width: 6px; ..."   -> present in output: true
Mermaid source "style A stroke-width:5px"           -> inline node style present: true
```

Satteri 0.5's
[`plugin.ts`](https://github.com/xingwangzhe/satteri-mermaid/blob/73ef075f8a0a52c8683c582cf787ae2fac3b8502/src/plugin.ts)
serialized only `svg.pipeline` and a narrowed `host_theme` object. Its public options did not expose
`site_config`, `themeVariables`, `themeCSS`, or `svg.scoped_css`. The accurate criticism is:
"Satteri 0.5 did not expose Merman's border-width controls."

The article's native side is also overstated. Satteri 0.7.0
[`RenderOptions`](https://github.com/xingwangzhe/satteri-mermaid/blob/7dfe1beccd57c88d38fc1502593c911943aa928a/src/lib.rs)
has `pie_stroke_width` and `pie_outer_stroke_width`, but no general node-border-width theme option.
Changing a border color is not changing its width. A direct published-package probe confirmed that
`primaryBorderColor` changes output, while passing an unknown `strokeWidth: 7` option produces
byte-identical output to the default and no 7 px stroke.

The current Merman contract still exposes `themeCSS` and `svg.scoped_css` in
[`OPTIONS_JSON.md`](../bindings/OPTIONS_JSON.md). Current renderability tests additionally verify
that `themeVariables.strokeWidth` reaches representative flowchart and block edge styles; see
[`theme_renderability_smoke.rs`](../../crates/merman/tests/theme_renderability_smoke.rs).

## 4. Mermaid themes

The article compares three different concepts under one "theme presets" heading:

| Surface | Actual named presets/themes |
| --- | --- |
| Merman 0.7 Mermaid themes | 11: `default`, `base`, `dark`, `forest`, `neutral`, `neo`, `neo-dark`, `redux`, `redux-dark`, `redux-color`, `redux-dark-color` |
| Merman 0.7 host/editor presets | 7: `editor-light`, `editor-dark`, `one-dark`, `gruvbox-light`, `gruvbox-dark`, `ayu-light`, `ayu-dark` |
| Satteri native / mmdr 0.3.1 presets | 5: `modern`, `default`/`base`, `dark`, `forest`, `neutral` |

Both Merman arrays are adjacent in the 0.7.0
[`index.ts`](https://github.com/Latias94/merman/blob/00a2f291025913d612b6ca1ce9b6df6daf5bef94/platforms/web/src/index.ts),
and the README explicitly says host presets are separate from Mermaid theme names. Satteri's 7-item
[`HostThemePreset`](https://github.com/xingwangzhe/satteri-mermaid/blob/73ef075f8a0a52c8683c582cf787ae2fac3b8502/src/plugin.ts)
shows how the categories were conflated.

The same 11 + 7 split remains in current
[`public-catalog.ts`](../../platforms/web/src/public-catalog.ts). It also matches Mermaid 11.16's
authoritative
[`MermaidConfig.theme` union](https://github.com/mermaid-js/mermaid/blob/7c0cafcf42e76bfaf79d0cbbd12edb986612f014/packages/mermaid/src/config.type.ts)
and
[`themes/index.js` registry](https://github.com/mermaid-js/mermaid/blob/7c0cafcf42e76bfaf79d0cbbd12edb986612f014/packages/mermaid/src/themes/index.js).

Mermaid's generated
[`theming guide`](https://github.com/mermaid-js/mermaid/blob/7c0cafcf42e76bfaf79d0cbbd12edb986612f014/docs/config/theming.md)
still lists only the five legacy themes and says only `base` is modifiable. That documentation is
internally behind the same release's type and runtime registries. The article's "5" matches that
guide, but not the actual 11.16 source surface. Its moving `mermaid@11` CDN import further prevents
an exact historical minor-version claim.

## 5. Color override coverage

### What existed in Merman 0.7

Satteri 0.5's `ThemeRoles` interface contains **22**, not 16, semantic host roles. Those roles are a
convenience palette for editor integration, not the full Merman theme surface.

The underlying Merman 0.7 TypeScript API also accepted:

- top-level `site_config`, including Mermaid `themeVariables` and `themeCSS`;
- `host_theme.themeVariables`; and
- SVG-scoped CSS.

Its pinned Mermaid 11.15 default theme snapshot contained 271 keys. Not every key is a color, and a
large configuration object is not proof that every renderer consumes every override, but it makes
the article's "16 color controls" an audit of Satteri's chosen wrapper fields, not of Merman. The
snapshot is preserved in
[`theme_variables_11_15_0.json`](https://github.com/Latias94/merman/blob/00a2f291025913d612b6ca1ce9b6df6daf5bef94/crates/merman-core/src/generated/theme_variables_11_15_0.json).

### What the native wrapper exposed

Satteri 0.7.0's Rust `RenderOptions` contains 77 fields. Sixty-six are `Option<String>`; after
excluding `theme` and `font_family`, 64 are direct color strings. The "60+" count is therefore
numerically plausible if every indexed palette slot is counted separately.

It is not "full coverage":

- 29 of those 64 fields are Git graph palette or label slots;
- 17 are Pie palette, text, or stroke colors; and
- the remaining 18 cover general surfaces and Sequence diagrams.

The wrapper has no corresponding family-specific color model for Architecture, C4, Gantt,
Requirement, Quadrant, Radar, or several other supported families. The underlying mmdr 0.3.1
[`Theme`](https://github.com/1jehuang/mermaid-rs-renderer/blob/2f993bd79a55235eb59a34d807852276ba25bea7/src/theme.rs)
has the same concentration. "64 exposed color slots" is supportable; "all colors across all
families" is not.

### What can be said about current Merman

The current repository provides strong evidence for **major** color customization:

- the generated Mermaid 11.16
  [`theme-variable artifact`](../../crates/merman-core/src/generated/theme_variables_11_16_0.json)
  contains 271 default keys with exact source provenance;
- [`theme.rs`](../../crates/merman-core/src/theme.rs) checks all 11 default snapshots and generated
  override-oracle cases;
- the renderability smoke covers visible theme signals in 26 representative families; and
- `site_config`, `host_theme.theme_variables`, `themeCSS`, and scoped CSS remain public binding
  paths.

It would still be incorrect to promise that every visible color in every family is semantically
overrideable:

- theme normalization admits values against the pinned known-key shape rather than arbitrary
  unknown keys;
- some families also use diagram-specific config, source-level styles, or renderer-local
  structure; and
- representative smoke coverage is intentionally not an exhaustive cross-product of every theme
  variable, family, shape, and state.

The defensible current statement is: **Merman supports the official theme-variable shape and major
visible colors across a broad family set, with CSS escape hatches; absolute all-color coverage is
not demonstrated.**

## 6. Native binary size

The article's "`8.8 MB` WASM versus `<1 MB` native binary" row combines a reasonably rounded WASM
measurement with a native number that is not present in the published package.

| Artifact | Raw bytes | Gzip `-9` bytes | Publication boundary |
| --- | ---: | ---: | --- |
| Historical `@mermanjs/web@0.7.0` WASM | 8,685,215 | not measured here | Published npm artifact used by Satteri 0.5 |
| Current `@mermanjs/web` WASM | 12,364,163 | not measured here | Unpublished working-tree full candidate |
| Current `@mermanjs/web-render` WASM | 11,689,729 | not measured here | Unpublished working-tree render candidate |
| Satteri 0.7.0/0.7.1 macOS arm64 `.node` | 4,206,544 | 1,822,210 | Published npm artifact |
| Merman source-bound transport `.node` | 21,223,312 | not measured in that run | Private internal candidate; not published |
| Merman complete-SVG size-control `.node` | 21,256,336 | 8,819,018 | Isolated default-profile experiment at `e311f9e6a` |
| Merman complete-SVG LTO + one-CGU `.node` | 18,998,416 | 8,306,113 | Isolated profile experiment; not adopted |

Satteri's other published native files are also above 1 MB: Linux arm64 glibc is 4,528,624 bytes,
Linux x64 glibc is 5,392,952 bytes, and Windows x64 is 5,768,704 bytes. All four files are shipped
inside the root package. Version 0.7.0's npm tarball is 8,310,476 bytes compressed and 19,931,028
bytes unpacked; version 0.7.1 is 8,310,468 and 19,931,056 bytes. The consumer therefore does not
receive a one-platform sub-package of less than 1 MB.

The native-size difference is real, but it is not evidence that N-API itself is intrinsically five
times smaller for Satteri. Both products use napi-rs. The isolated feature matrix measured:

| Merman N-API capability lane | Raw bytes | Increment over SVG | Normal packages |
| --- | ---: | ---: | ---: |
| SVG | 15,771,392 | baseline | 132 |
| SVG + Cytoscape | 16,184,784 | +413,392 | 133 |
| SVG + ELK | 17,028,528 | +1,257,136 | 134 |
| SVG + math | 19,635,312 | +3,863,920 | 186 |
| Complete SVG | 21,256,336 | +5,484,944 | 189 |

Satteri wraps `mermaid-rs-renderer` 0.3.1's declared 23-type synchronous SVG surface. Merman's
complete artifact reports 41 diagram families and includes broader Mermaid 11.16 semantics,
Cytoscape, ELK, math/RaTeX and embedded font data, sanitizer/config behavior, resource policies,
and generic binding operations. Those counts have not been normalized to one classification.
Within Merman, optional capabilities account for the 5.48 MB complete-over-SVG increase; they do
not directly explain the cross-product gap. Merman's SVG-only base is still 3.75x Satteri, so the
residual cause remains open.

The remaining gap is linked code and static data, not an exported-symbol accident:

- both complete binaries expose one global symbol;
- default-profile Merman has 13,336,980 bytes of `__text` and 5,346,000 bytes of constant sections,
  versus Satteri's 3,176,372 and 552,308 bytes;
- fat LTO alone saved only 16,512 raw bytes;
- fat LTO plus one codegen unit saved 2,257,920 bytes, or 10.62%; and
- adding Cargo `strip = "symbols"` after the napi CLI's linker `-s` saved zero additional raw
  bytes on Darwin arm64, while changing the binary and increasing gzip output by 2,735 bytes.

The strongest tested Merman artifact remains 4.52x Satteri. Code plus constants explain 90.46% of
the remaining 14,791,872-byte gap. The strip lane does not establish byte equivalence,
cross-target redundancy, or an observed diagnostics change; the
[rustc guidance](https://doc.rust-lang.org/rustc/codegen-options/index.html#strip) documents the
general backtrace and profiling trade-off. Fat LTO plus one codegen unit is worth a cross-target
release experiment, not an automatic profile change.

The correct product conclusion is therefore: Satteri currently has the smaller published native
artifact, while Merman's private complete candidate pays for substantially more behavior and still
has a large executable-code/static-data frontier. It would be equally misleading to repeat
`<1 MB`, attribute the gap to N-API, or dismiss all of the remaining gap as unavoidable.

## 7. Direct Node N-API performance

The article publishes no reproducible native benchmark, so a separate experiment compared the
actual `@xingwangzhe/satteri-mermaid@0.7.1` addon with Merman's private N-API addon on Apple M4 Pro.
Both used their public synchronous SVG facades over 30 shared source fixtures. The protocol used
three warmups, six alternating AB/BA rounds, equal calibrated iteration counts, and 283,656 raw
timed calls. Both candidates returned well-formed SVG for all 30 inputs. The harness passed the
same source string to both facades, but Satteri's public wrapper applied `String.trim()` and removed
one trailing LF from every fixture. That wrapper behavior was recorded rather than bypassed.
The selection reuses the native comparison's 30 ratio-eligible standard fixtures, excluding
`flowchart_large`, Info, Treemap, and XYChart where the reference was missing or fixture bytes
differed.

| Aggregate | Result |
| --- | ---: |
| Merman faster / Satteri faster | 13 / 17 |
| Median fixture Merman / Satteri | 1.247x |
| Geometric mean Merman / Satteri | 0.458x |

The median fixture favors Satteri by 24.7%; the geometric mean favors Merman by about 2.18x because
complex Flowchart cases dominate. The largest material Satteri leads were:

| Fixture | Merman | Satteri | Merman / Satteri |
| --- | ---: | ---: | ---: |
| `mindmap_medium` | 294.31 us | 83.33 us | 3.53x |
| `requirement_medium` | 254.63 us | 80.33 us | 3.17x |
| `c4_medium` | 189.83 us | 73.42 us | 2.59x |
| `sequence_tiny` | 154.98 us | 56.17 us | 2.76x |
| `kanban_medium` | 119.88 us | 39.33 us | 3.05x |
| `class_tiny` | 105.29 us | 29.33 us | 3.59x |

Merman's largest wins were complex Flowcharts: `flowchart_ports_heavy` measured 1.54 ms versus
471.28 ms, and `flowchart_medium` measured 4.15 ms versus 106.16 ms. Merman also led the medium
Sequence, Class, State, and ER fixtures. The direct run proves successful public-call execution
from the same source arguments; it does not prove byte-identical renderer inputs or visual,
geometry, DOM, theme, or Mermaid-semantic equivalence. Its correctness receipt recorded different
raw, structure, and geometry digests for all 30 outputs; Merman emitted 669,856 bytes in total and
Satteri 335,730 bytes.

A second same-source experiment compared Merman's own N-API and Node-WASM transports across 4,001
cases. All semantic or typed-error outcomes and SVG structure signatures matched. For successful
SVG samples, N-API reduced warm p50 from 0.3189 to 0.2903 ms and p95 from 1.6305 to 1.3467 ms.
Engine initialization through first SVG fell from 96.05 to 7.39 ms, and peak RSS fell from
638,189,568 to 240,648,192 bytes. N-API was larger, and 426 SVGs retained exact-geometry/raw-byte
drift with no attributed cause. The warm timer includes the public facade call plus SHA-256 and
byte-length evidence projection; cold and concurrent timers stop before that projection. The
transport remains unselected because only one target has runtime/install evidence, while the
current validator retains geometry/raw drift as a report residual rather than an admission gate.

Both Satteri and Merman use napi-rs, and the separate Merman transport run does not show a broad
local latency penalty for its N-API candidate. That control also changes target and runtime, so it
is not an isolated bridge A/B. The direct comparison does not isolate renderer and layout from
facade, marshalling, binding, allocation, build-profile, or different-output costs. It also does
not establish that every Merman diagram is faster. Satteri embeds mmdr 0.3.1; the separate native
Criterion report uses later mmdr revision `7ff1196` and a direct Rust boundary, so the two ratio
aggregates must not be merged.

## 8. Fairness assessment

The article's comparison is useful as a migration story for one static blog, but not as a renderer
benchmark:

1. It publishes no benchmark source, machine, runtime version, corpus, warmup, repetitions,
   distribution, or raw samples.
2. It compares browser user-perceived Mermaid latency with Node build-time render calls.
3. It reports WASM cold initialization separately, then appears to use a first-render-scale number
   as the per-diagram cost.
4. It compares different default themes, layout implementations, text measurement, SVG structure,
   and output sizes.
5. It compares Satteri's narrow Merman option wrapper with its much wider native wrapper, then
   attributes that wrapper difference to the rendering engines.
6. It treats the existence of a WASM bridge as proof of steady-state slowness. The same-host probe
   demonstrates that this architectural inference is unsafe.

The exact-version part is comparatively sound: Satteri's lockfile identifies Merman 0.7.0, and
Satteri 0.7.0 identifies mmdr 0.3.1. The Mermaid.js comparator is not exact because `mermaid@11` is a
moving CDN major tag.

A publishable comparison would need at least:

- immutable versions for all three renderers;
- one runtime category per table, rather than browser and Node in the same latency row;
- separate import/load, compile/instantiate, first-render, and warm-render measurements;
- identical source fixtures, theme intent, viewport, and text-measurement policy;
- structural or visual acceptance criteria for each output;
- warmups, repetitions, p50/p95/p99, failures, SVG bytes, and peak memory; and
- a representative multi-family corpus, not one blog diagram.

## Reproduction record

Registry and source identity were checked with:

```sh
curl -fsSL 'https://registry.npmjs.org/%40mermanjs%2Fweb' |
  jq '{dist_tags: .["dist-tags"], v070: .versions["0.7.0"], time: .time["0.7.0"]}'

curl -fsSL 'https://registry.npmjs.org/%40xingwangzhe%2Fsatteri-mermaid' |
  jq '{dist_tags: .["dist-tags"], v050: .versions["0.5.0"], v070: .versions["0.7.0"]}'

git ls-remote --tags https://github.com/xingwangzhe/satteri-mermaid.git
git ls-remote --tags https://github.com/1jehuang/mermaid-rs-renderer.git refs/tags/v0.3.1
```

The artifact and benchmark setup used the registry tarballs directly:

```sh
AUDIT_TMP="$(mktemp -d)"

curl -fsSL -o "$AUDIT_TMP/web.tgz" \
  'https://registry.npmjs.org/@mermanjs/web/-/web-0.7.0.tgz'
mkdir "$AUDIT_TMP/web"
tar -xf "$AUDIT_TMP/web.tgz" -C "$AUDIT_TMP/web" --strip-components=1
wc -c "$AUDIT_TMP/web/pkg/merman_wasm_bg.wasm"

curl -fsSL -o "$AUDIT_TMP/native.tgz" \
  'https://registry.npmjs.org/@xingwangzhe/satteri-mermaid/-/satteri-mermaid-0.7.0.tgz'
mkdir "$AUDIT_TMP/native"
tar -xf "$AUDIT_TMP/native.tgz" -C "$AUDIT_TMP/native" --strip-components=1
```

For historical Merman, the timed initialization was the Satteri 0.5 sequence: import the package
API, `readFileSync` the WASM, `WebAssembly.compile`, and call `initMerman({ loader, wasm })`.
Fifteen fresh processes supplied the cold samples; warm timing used 12 batches of 2,048 renders.
For current Merman, a no-store local HTTP server exposed the generated package entry points to
seven fresh Edge realms per package; warm timing used 12 batches of 1,024 renders. Each candidate's
repeated SVG hash was stable. The resulting JSON receipt has SHA-256
`b338fc1a582eab0b91d6a154f135eed5ef3e4f9aa8297d3906eb2e0c331af355`.

Border probes rendered the exact `themeVariables` overrides described above and asserted the
requested primary color, border color, line color, and 7 px stroke in the SVG. Native sizes came
from the exact npm tarball and current private candidate. `size -m`, `nm -gU`, `otool -L`, and a
copy subjected to the platform strip command supplied the Mach-O evidence.

The ignored experiment artifacts bind the later Node conclusions:

| Evidence | SHA-256 |
| --- | --- |
| Direct Merman/Satteri timing JSON | `89e1eaeeaba8a45f9e6fa84989816efc5bdef8d828ff208f1b80eacd4493b00f` |
| Direct Merman/Satteri correctness JSON | `554d3764a66754e2503e1f0ae3dd11a2e4295f32aac07b513dca7455b4b811f0` |
| Merman N-API/Node-WASM schema-2 report | `a5ef6ad899033010209de2ae9cb25ffadad13c3f57fc56cbd351f777e8119226` |
| N-API capability/profile size report | `85293fafb973285aa347f05216ca3a639031b5d1d915e318c51f10ef5691577c` |

The direct timing report binds Merman source `4264f2aad`, Merman artifact
`sha256:dcee2097a278aa8f68e859595050efb760e14ce8a801fa2973b469f5dbe9d974`,
Satteri 0.7.1, and Satteri artifact
`sha256:033d35d852ba5b2b712f72f64ad5900fa975a9a60806ba5fd3a2127cec4e41fa`.
The transport report and its reproduction command are documented in
[`NODE_TRANSPORT_ADMISSION.md`](../performance/NODE_TRANSPORT_ADMISSION.md).

## Limitations

- The browser audit used working-tree Web artifacts. The Node experiments used source-bound,
  hashed artifacts in isolated worktrees. None is a published release promise.
- Startup probes used fresh Node processes but a warm operating-system filesystem cache, not a
  rebooted host or evicted page cache.
- Current browser probes used a local no-store origin. They exclude real network transfer and
  browser process startup, both of which can dominate a first visit.
- Browser phase timing used one small flowchart. The direct native comparison used 30 cross-family
  fixtures, but both experiments ran on one machine and neither is a universal throughput ranking.
- Outputs were checked for successful rendering, well-formed SVG, byte size, and requested
  CSS/style signals. Independent renderer outputs were not judged visually or geometrically
  equivalent.
- The current `0.8.0-alpha.4` Web package observations come from an uncommitted working-tree
  candidate and must not be represented as an npm release promise.

## Primary sources

- [Satteri article Markdown](https://xingwangzhe.fun/posts/satteri-mermaid-npm-package.md) -- claims
  and published code excerpts.
- [Satteri 0.5.0 source](https://github.com/xingwangzhe/satteri-mermaid/tree/73ef075f8a0a52c8683c582cf787ae2fac3b8502)
  -- exact WASM wrapper, plugin options, dependency, and lock.
- [Satteri 0.7.0 source](https://github.com/xingwangzhe/satteri-mermaid/tree/7dfe1beccd57c88d38fc1502593c911943aa928a)
  -- exact native wrapper and theme/color options used by the article.
- [`@mermanjs/web@0.7.0` registry record](https://registry.npmjs.org/@mermanjs%2Fweb/0.7.0) and
  [source](https://github.com/Latias94/merman/tree/00a2f291025913d612b6ca1ce9b6df6daf5bef94)
  -- published package identity and then-current binding contract.
- [mmdr 0.3.1 source](https://github.com/1jehuang/mermaid-rs-renderer/tree/2f993bd79a55235eb59a34d807852276ba25bea7)
  -- native renderer theme model and preset names.
- [Mermaid 11.16 source](https://github.com/mermaid-js/mermaid/tree/7c0cafcf42e76bfaf79d0cbbd12edb986612f014)
  -- authoritative current theme union, registry, and documentation.
- [Cargo profiles](https://doc.rust-lang.org/cargo/reference/profiles.html) and
  [rustc codegen options](https://doc.rust-lang.org/rustc/codegen-options/index.html) -- release
  LTO, codegen-unit, and stripping semantics.
- Current local Merman
  [`Web contract`](../../platforms/web/README.md),
  [`binding options`](../bindings/OPTIONS_JSON.md), and
  [`theme tests`](../../crates/merman/tests/theme_renderability_smoke.rs) -- unpublished current
  repository evidence.
- [Merman/mmdr native checkpoint](../performance/renderer_comparison_2026-07-28_75c9fd156_vs_mmdr.md)
  and [Node transport admission](../performance/NODE_TRANSPORT_ADMISSION.md) -- reproducible local
  performance and correctness boundaries.
