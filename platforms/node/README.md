# Merman Node.js / SSG package group

`@mermanjs/node` is Merman's experimental native Node.js package for static SVG rendering. It is
published as an alpha package group: a small loader selects one exact-version platform package at
install time. The root package contains no native binary, no browser-WASM fallback, and no
postinstall downloader.

Install the loader, not a platform package:

```console
npm install @mermanjs/node@alpha
```

The alpha group supports macOS arm64/x64, Linux x64 glibc/musl, and Windows x64 MSVC on Node
`>=22`. Each release builds, packs, installs, and renders through the public loader on its actual
target. It uses the deterministic static-SVG recipe: SVG plus Cytoscape and ELK layouts. Math,
binary export, analysis, ASCII, text-measurement callbacks, browser fallback, and runtime download
are intentionally outside this package surface.

```js
import { createNodeEngine } from "@mermanjs/node";

const engine = await createNodeEngine();
try {
  const svg = await engine.renderSvg("flowchart TD\\nA --> B");
  console.log(svg);
} finally {
  await engine.dispose();
}
```

The package remains prerelease software. Its API is Promise-first, uses a bounded queue, and treats
`AbortSignal` as queued-work cancellation only; `dispose()` waits for active Rust work to settle.
`renderSvgSync()` is reserved for explicit static-site build paths.

The Node-targeted WASM implementation remains an internal comparison transport. It is not shipped
or selected by the loader.
