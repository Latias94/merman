# `@mermanjs/node` private candidate

This package remains private until the complete Node admission matrix passes. Its shape is a small
Promise-first loader and type surface with exact-version optional dependencies on one target
package. It contains no native binary, WASM binary, postinstall downloader, or browser-WASM
fallback. It requires Node `>=22.0.0`.

For a supported Node or SSG integration today, invoke `merman-cli` as a child process. Do not install this candidate from its workspace.

`createNodeEngine()` returns `MermanEngine`. The candidate is deterministic-only and text-only;
`renderSvg`, `svgPlanJson`, `metadataJson`, and generic string-result execution share the same strict
native/WASM transport contract. Its private recipe includes SVG and both layout backends, but not
math or binary export; unsupported diagrams keep the shared typed capability error. Transport
identity/version checks establish compatibility, not authentication.

Queued operations are bounded. `AbortSignal` can cancel queued work but does not preempt executing
Rust work. `dispose()` drains executing work, rejects pending work, and is idempotent.
`renderSvgSync()` is only for an explicit SSG build path.
