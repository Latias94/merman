# Satteri Mermaid / Node SSG Integration Research

**Date:** 2026-07-22
**Status:** research and design input; no product behavior changed by this document.

## Scope and method

This investigates the migration described in [Satteri Mermaid NPM package](https://xingwangzhe.fun/posts/satteri-mermaid-npm-package/). The blog is useful context, but its claims were checked against the linked [patch](https://raw.githubusercontent.com/msfjarvis/msfjarvis.dev/10ff9777049434698de2374d051168cfd746aa84/patches/%40mermanjs__web%400.7.0.patch), published npm tarballs, the pinned historical Merman source, current Merman source, and primary Node, wasm-bindgen, and napi-rs documentation.

The question is deliberately broader than whether one patch still applies. It asks whether a browser-oriented WASM package is a sound Node/SSG integration surface, and whether Merman should add a native Node-API transport without confusing transport with Mermaid semantic capability.

## Executive conclusion

1. The Satteri patch fixed a real wasm-bindgen initializer **contract drift**, but it was not the complete Node solution. `@mermanjs/web@0.7.0` accepted a direct `WebAssembly.Module` only through a deprecated compatibility path. Passing `{ module_or_path: wasm }` used the generated API's preferred shape and removed the warning.
2. The operational Node problem was deeper: Merman 0.7.0 was built with wasm-bindgen's `--target web`. Its default initializer resolves a `file:` URL relative to `import.meta.url`; Node's `fetch()` does not load that URL. A Node/SSG caller must resolve, read, and compile the asset itself, or consume a Node-targeted artifact. This was reproduced from the published 0.7.0 tarball.
3. The current `@mermanjs/web` initializer correctly passes custom WASM values in the object form, so the exact patch is no longer needed. It remains a browser package, however, and does not make default Node/SSG initialization work. Its custom-asset README example is appropriate to a browser/bundler URL, not Node package resolution.
4. A Node integration is justified as a separate **transport and distribution** after a small, measured admission prototype. It must not be a Mermaid diagram feature, a Cargo semantic capability, or a substitute for the browser package. The likely product is `@mermanjs/node`, backed by a private napi-rs crate and generated platform packages.
5. Do not copy Satteri's distribution topology. Its published root tarball contains multiple native binaries, reports an unpacked size of 19,931,056 bytes, and has incomplete target coverage despite broader loader claims. The napi-rs recommended root-loader plus exact optional platform packages is the appropriate model.

## What the patch actually fixed

The linked patch changes both source and generated distribution code from:

```ts
await module.default(wasm);
```

to:

```ts
await module.default({ module_or_path: wasm });
```

The generated `merman_wasm.js` in the published `@mermanjs/web@0.7.0` package accepts both forms. A direct `InitInput`, including a compiled `WebAssembly.Module`, takes a deprecated compatibility branch; the object form is the documented preferred call shape. Therefore the patch is a correct forward-compatibility and warning-removal fix, but not evidence that direct input could never initialize.

The historical wrapper did not otherwise know how to locate a Node-installed WASM file. It was built with `wasm-pack build --target web`, and its default generated initializer derives `new URL("merman_wasm_bg.wasm", import.meta.url)`. On Node 26, importing the published package and calling `initMerman()` produced `TypeError: fetch failed` for that `file:` URL. Reading the exported `.wasm`, compiling it with `WebAssembly.compile`, and supplying both a custom JS glue loader and the compiled module initialized successfully. The direct-form experiment also emitted the generated deprecation warning; the object form did not.

This matches wasm-bindgen's own deployment guidance: [`--target web`](https://rustwasm.github.io/docs/wasm-bindgen/reference/deployment.html) is for browser delivery, while Node should use a Node target such as `--target nodejs` or the documented ESM option. The [no-bundler example](https://rustwasm.github.io/docs/wasm-bindgen/examples/without-a-bundler.html) also explains that web initialization derives the asset URL from the module URL.

| Layer | 0.7.0 finding | Current Merman status | Correct ownership |
| --- | --- | --- | --- |
| wasm-bindgen call shape | Direct custom input used deprecated compatibility behavior. | Fixed in [`platforms/web/src/index.ts`](../../platforms/web/src/index.ts): custom input is wrapped as `{ module_or_path: wasm }`. | Browser WASM wrapper. |
| Default asset loading | Browser `file:` URL path fails under Node's `fetch`. | Still browser-oriented by design. | A Node artifact or a documented Node-specific adapter, never filesystem crawling in an app. |
| npm exports | The WASM subpath was exportable but callers still had to turn it into bytes/module. | Browser package exports remain a browser surface. | Node package loader should resolve its own native binary. |
| TypeScript contract | Generated wasm-bindgen input shape was not reflected by the wrapper's invocation. | Wrapper call is aligned; custom inputs remain explicit. | Keep wrapper types synchronized with generated glue. |
| Cache and engine reuse | The blog's manual initialization was a singleton workaround. | Browser runtime owns its cache and initialization state. | A Node renderer must own a bounded, explicit engine lifecycle. |
| Mermaid capability | No diagram behavior was missing. | No semantic gap. | Do not introduce a Node-specific Mermaid feature. |

`@mermanjs/web@0.7.0` was approximately 8.77 MB unpacked according to its [npm registry metadata](https://registry.npmjs.org/@mermanjs%2Fweb/0.7.0), including an 8,685,215-byte WASM file. That is evidence of the browser artifact's size, not evidence that a native addon is inherently smaller or faster.

## Why the current web package should not become a hidden Node product

The current package describes itself as browser rendering, and [`crates/merman-wasm/Cargo.toml`](../../crates/merman-wasm/Cargo.toml) likewise declares browser WASM bindings. That boundary is healthy. Extending it with Node-only `fs`, `createRequire`, or platform probing would make browser and SSR bundling behavior less predictable.

There is one documentation issue to correct if no Node package exists yet: the current custom-input example uses:

```ts
new URL("@mermanjs/web/pkg/merman_wasm_bg.wasm", import.meta.url)
```

In Node, that is a URL relative to the caller's source file, not package-resolution syntax. Modern Node offers [`import.meta.resolve()`](https://nodejs.org/api/esm.html#importmetaresolvespecifier) for module-relative resolution, but a consumer still has to read/compile a browser-targeted WASM asset and choose the correct glue module. The web README should state plainly that the package is browser-only and should not prescribe this code as a Node recipe.

The product choices are consequently:

1. Keep `@mermanjs/web` browser-only and publish a Node-targeted WASM package.
2. Keep `@mermanjs/web` browser-only and publish a native Node-API package.
3. Publish both only if measured workload, support, and release cost justify two Node transports.

The recommended order is an evidence-gated prototype of (1) and (2), then one deliberate public product. A browser package must not silently select a Node implementation, and a native package must never silently fall back to Mermaid.js or browser WASM because that changes output, resource, and environment behavior.

## Assessment of Satteri's native package

Satteri's source currently uses napi-rs target configuration and a JavaScript platform loader. Its [published 0.7.1 registry record](https://registry.npmjs.org/@xingwangzhe%2Fsatteri-mermaid/0.7.1) reports 19,931,056 bytes unpacked. The corresponding release workflow puts four compiled `.node` files in the root package before publishing. Their observed sizes range from about 4.2 MB to 5.8 MB.

This disproves two broad conclusions that could otherwise be drawn from the blog:

- The published package is not a sub-1 MB native module.
- It does not demonstrate universal desktop support. Its target configuration includes Linux x64 GNU, Linux arm64 GNU, macOS arm64, and Windows x64; the loader has branches that exceed that actual artifact set.

The blog's move to Node-API can still be a good product decision for an SSG. Its relevant value is predictable native initialization and a direct Node API, not a generally valid performance or package-size claim. It also used a different renderer and coverage set, so its timing data is not a Merman-vs-N-API benchmark.

## Transport comparison for Merman

| Surface | Appropriate caller | Initialization and ownership | Threading and large output | Fonts, time, and capabilities | Distribution trade-off |
| --- | --- | --- | --- | --- | --- |
| Rust facade | Rust applications, build tools | Direct `Engine` / renderer construction. | Native caller controls scheduling and buffers. | Canonical environment/resource policies. | No Node integration. |
| C ABI | C/C++ or custom FFI hosts | Explicit ABI handles and buffers. The planned ABI 3 request/result/sink model is the right shared lower boundary. | Caller must respect ownership and callback rules. | Host callbacks are explicit, not automatic. | Node FFI wrappers would add unsafe dynamic-library packaging with no ergonomic gain. |
| UniFFI | Swift, Kotlin, Python-style SDKs | Generated bindings around the binding core. | Language runtime controls call boundaries. | Good for declared SDK hosts, not Node. | Does not supply an npm-native runtime. |
| Browser WASM | Browsers and browser bundlers | `wasm-bindgen --target web`; browser asset loading and browser runtime state. | JS/WASM boundary and browser lifecycle apply. | Browser text measurement integration where available. | Portable browser delivery, but not default Node asset loading. |
| Node-API via napi-rs | Node SSG, CLIs, server-side Node workers | Native addon loader initializes a canonical binding engine. | Sync calls block the event loop; async work needs a bounded native task model. | Select an explicit Merman environment profile; N-API does not grant automatic font or timezone parity. | Requires per-platform binaries, support policy, and native-addon security documentation. |

Node's [N-API documentation](https://nodejs.org/api/n-api.html) guarantees ABI stability for the Node-API interface across compatible Node versions. It does **not** make a compiled Rust binary portable across OS, CPU, libc, or alternative runtimes. napi-rs makes the same distinction in its [compatibility guidance](https://napi.rs/docs/more/support-compatibility). Merman must publish only an actually tested target matrix.

## Recommended Node package architecture

### Product boundary

Add the following only after the admission prototype passes:

- A private `crates/merman-node` transport crate using napi-rs.
- A public `@mermanjs/node` JavaScript root package.
- Private-or-public-but-not-user-installed `@mermanjs/node-<target>` platform packages, each containing exactly one native artifact and declaring exact `os`, `cpu`, and, where needed, `libc` constraints.

`@mermanjs/node` is a runtime transport. It does not add a Mermaid family, parser mode, render capability, or configuration syntax. It must consume the same generated capability descriptor used by Rust, CLI, FFI, WASM, and documentation. Its build should select a named descriptor/preset, initially the complete native SDK closure, rather than inventing Node-only semantic flags.

The addon should call [`merman-bindings-core`](../../crates/merman-bindings-core) and the planned typed render request/result model directly. Do not bridge Node through the C ABI, and do not duplicate parsing, configuration, capability detection, or error classification in JavaScript. The Node transport needs its own version/schema field, while Merman's native C ABI version and Node-API version remain separate compatibility contracts.

### Public API and lifecycle

The initial API should be intentionally small:

```ts
const renderer = createRenderer({
  environment: "deterministic" | "native",
  resourceProfile: "default" | "constrained",
  maxConcurrentRenders: number,
});

const svg = await renderer.renderSvg(source, options);
await renderer.dispose();
```

- `renderSvgSync()` may exist only as an explicit, documented event-loop-blocking convenience for one-off build tools.
- The default should return a Promise backed by napi-rs `AsyncTask` or an equivalent bounded native scheduler. Node says asynchronous native work must not block the event loop; see [N-API async work](https://nodejs.org/api/n-api.html#asynchronous-work).
- Accepting an `AbortSignal` must document the actual limit: napi-rs [cancellation](https://napi.rs/docs/concepts/async-task) can cancel queued work, not arbitrary in-progress native rendering. Full cancellation requires a future cooperative Merman operation token.
- Cache engines per explicit renderer instance and compatible environment, with bounded concurrency and disposal. Do not make a process-global, unbounded cache keyed by arbitrary options.
- Return a string or `Buffer` for ordinary SSG output. The future ABI sink is useful for cross-language ownership, but it does not by itself make an async JavaScript callback a safe streaming interface. A `renderToFile` API should be added only after a native worker can write atomically with verified memory and backpressure behavior.

For text measurement, native Node should initially use Merman's deterministic vendored metrics. A JavaScript callback cannot transparently provide a synchronous, safe native font measurement service across async worker threads. System-font exactness needs a separately admitted native font backend and source-backed behavior evidence. Similarly, `environment: "native"` may opt into system clock/timezone/random adapters; `"deterministic"` must not read them. Neither decision belongs to N-API itself.

### Packaging, installation, and security

napi-rs recommends a thin root package with exact `optionalDependencies` on per-platform packages, avoiding postinstall downloads and a root tarball containing every binary; see its [release guide](https://napi.rs/docs/deep-dive/release). Merman should follow that model.

Required distribution rules:

- Publish a small loader package plus one binary per supported target, never all target binaries in the root package.
- Pin root and platform package versions exactly, verify every expected package before moving a dist-tag, and recover safely from npm's non-atomic multi-package publication.
- Start with only CI-built, clean-install-tested targets. A reasonable first matrix is macOS x64/arm64, Windows x64, Linux x64 GNU/musl, and Linux arm64 GNU/musl, but no target is public until it is actually built and tested.
- Return a typed unsupported-platform error with OS, CPU, libc, and runtime evidence. Do not try an undocumented browser-WASM fallback.
- Mark the package Node-only, ensure bundlers externalize it, and test that browser/client imports fail clearly rather than leaking a binary into a client build.
- Document Node's [permission model](https://nodejs.org/api/permissions.html): with that model enabled, native addons require `--allow-addons`.
- Produce SPDX/license notices, dependency provenance, artifact hashes, and an SBOM per platform release. Native packaging widens the release and supply-chain surface even when application logic is shared.

## Admission prototype and release gates

Before adding a permanent package, implement a private prototype that runs the same `merman-bindings-core` request/options/error contract through two transports:

1. Node-targeted wasm-bindgen output, not the browser `--target web` artifact.
2. napi-rs Node-API output.

Measure on a clean Node process for the pinned 35-family corpus and representative large diagrams:

- import plus first render, then warm render and batch render;
- RSS/peak allocation, output size, and error behavior;
- deterministic SVG semantic parity with direct Rust rendering;
- package tarball and installed aggregate size, both compressed and unpacked;
- supported Node versions and each proposed OS/CPU/libc target;
- concurrent rendering, `dispose`, queued cancellation, and no event-loop starvation;
- large SVG behavior and any proposed file-output path;
- clean installation from npm-like packages, unsupported-target diagnostics, permission-model behavior, ESM/CJS loader behavior, and SSR/bundler externalization.

Use these gates to choose one public Node transport. Do not use the blog's timing as an admission criterion, because it measures another renderer and a different coverage set. If Node-targeted WASM is adequate, it has lower native release burden; if N-API materially improves complete SSG workflow behavior and the platform matrix remains supportable, publish the native package. A decision must be based on measured end-to-end user value, not on a generic claim that native code is faster.

## Required plan and documentation changes

The capability-driven architecture plan currently covers browser WASM packages and native SDKs, but not Node/SSG as a first-class transport. Add a dedicated, gated work unit with these requirements:

1. Record the prototype decision and exact supported Node/platform matrix in the capability/release descriptor without adding a Mermaid semantic capability.
2. Add a `merman-node` transport crate and package generator only if the prototype admits N-API.
3. Make every Node request use the canonical binding-core typed request, resource profile, environment policy, error schema, and capability report.
4. Add artifact-level release verification for optional platform packages, target metadata, dist-tags, hashes, notices, SBOMs, and clean installs.
5. Add fixture parity, lifecycle, concurrency, cancellation, large-output, native-environment, unsupported-platform, permission-model, and SSR externalization tests.
6. Update the user-facing feature guide with a workflow matrix: browser applications use `@mermanjs/web`; Node SSG/build systems use `@mermanjs/node` only if published; Rust/CLI/native SDK users select their documented presets. State that transport selection does not alter Mermaid semantics.
7. Correct the browser package README so its custom WASM example is not presented as a Node solution. Until a Node product exists, document that Node use is unsupported rather than asking users to search `node_modules` for an asset.

This complements, rather than replaces, the planned capability and package architecture work in [`docs/plans/2026-07-22-001-refactor-capability-driven-feature-and-distribution-architecture-plan.md`](../plans/2026-07-22-001-refactor-capability-driven-feature-and-distribution-architecture-plan.md). The plan's browser package split, typed ABI work, and generated capability catalog are prerequisites for a correct Node transport; they are not themselves a Node/SSG integration contract.

## Source inventory

- [Satteri blog post](https://xingwangzhe.fun/posts/satteri-mermaid-npm-package/)
- [Satteri patch for `@mermanjs/web@0.7.0`](https://raw.githubusercontent.com/msfjarvis/msfjarvis.dev/10ff9777049434698de2374d051168cfd746aa84/patches/%40mermanjs__web%400.7.0.patch)
- [`@mermanjs/web@0.7.0` npm registry metadata](https://registry.npmjs.org/@mermanjs%2Fweb/0.7.0)
- [Current Satteri native package registry metadata](https://registry.npmjs.org/@xingwangzhe%2Fsatteri-mermaid/0.7.1)
- [Satteri package manifest](https://raw.githubusercontent.com/xingwangzhe/satteri-mermaid/53d6be9609ac2fb666dd73f93129e821fd3f95ea/package.json) and [release workflow](https://raw.githubusercontent.com/xingwangzhe/satteri-mermaid/53d6be9609ac2fb666dd73f93129e821fd3f95ea/.github/workflows/release.yml)
- [wasm-bindgen deployment guidance](https://rustwasm.github.io/docs/wasm-bindgen/reference/deployment.html)
- [Node ESM `import.meta.resolve`](https://nodejs.org/api/esm.html#importmetaresolvespecifier) and [package exports](https://nodejs.org/api/packages.html)
- [Node-API documentation](https://nodejs.org/api/n-api.html), including [asynchronous work](https://nodejs.org/api/n-api.html#asynchronous-work)
- [napi-rs release packaging](https://napi.rs/docs/deep-dive/release), [support compatibility](https://napi.rs/docs/more/support-compatibility), and [async task cancellation](https://napi.rs/docs/concepts/async-task)
- Current [`platforms/web/src/index.ts`](../../platforms/web/src/index.ts), [`platforms/web/README.md`](../../platforms/web/README.md), [`crates/merman-wasm/Cargo.toml`](../../crates/merman-wasm/Cargo.toml), and [`crates/merman-bindings-core`](../../crates/merman-bindings-core)
