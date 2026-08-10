# `@mermanjs/node`

Experimental native Node.js and static-site SVG rendering bindings for Merman.

Install this loader package with `npm install @mermanjs/node@alpha`. It resolves exactly one
matching `@mermanjs/node-<platform>` optional dependency; do not install those platform packages
directly. The loader has no bundled native/WASM binary, postinstall downloader, or browser fallback.

It requires Node `>=22` and currently exposes deterministic SVG rendering and metadata/layout
operations backed by SVG, Cytoscape, and ELK. Optional capabilities outside that shipped recipe
return the shared typed missing-capability error.
