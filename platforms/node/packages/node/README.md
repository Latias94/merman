# `@mermanjs/node` private candidate

This package is private until U14 comparison evidence admits a Node transport. Its intended shape is a small Promise-first loader and type surface with exact-version optional dependencies on one target package. It contains no native binary, WASM binary, postinstall downloader, or browser-WASM fallback.

For a supported Node or SSG integration today, invoke `merman-cli` as a child process. Do not install this candidate from its workspace.

Queued operations are bounded. `AbortSignal` can cancel queued work but does not preempt executing Rust work. `dispose()` drains executing work and rejects pending work. `renderSvgSync()` is only for an explicit SSG build path.
