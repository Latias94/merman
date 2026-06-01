# @merman/web

Browser integration for merman. This package wraps the `merman-wasm` wasm-bindgen output with a
small TypeScript API.

## Build

```sh
npm install --prefix platforms/web
npm run build --prefix platforms/web
```

## Usage

```ts
import { initMerman, renderSvg } from "@merman/web";

await initMerman();

const svg = renderSvg("flowchart TD\nA[Hello] --> B[World]", {
  svg: { pipeline: "readable" },
});
```

The options object is serialized to the shared merman binding options JSON contract documented in
`docs/bindings/OPTIONS_JSON.md`.
