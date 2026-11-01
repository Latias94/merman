# Complete SVG binding budget, 2026-11-01

The main binding-delivery change explicitly includes math in `web-full` and `web-render`.
This restores the complete SVG product expectation of published Web 0.9.2. It is a capability
change relative to the intervening math-free main recipes, not an equal-capability size saving.
Analysis, editor and ASCII retain their separate task scopes.

Measurements use the final assembled npm packages from the implementation based on main
`939ccec9f8d1391ae672a5397d5b57d10a1307b8`, Rust 1.95.0, wasm-pack 0.15.0 and
wasm-tools 1.253.0 on macOS ARM64. The existing package producer selects the descriptor-owned
`wasm-size` Cargo profile. The size owner strips final WASM with `wasm-tools strip --all`
and applies its existing gzip and Brotli settings; no alternate Cargo-only artifact is measured.

## Final artifact sizes

| Artifact | Raw bytes | Stripped bytes | Gzip bytes | Brotli bytes |
| --- | ---: | ---: | ---: | ---: |
| `web-analysis` | 4,058,595 | 4,058,330 | 1,550,929 | 1,180,298 |
| `web-ascii` | 5,750,323 | 5,750,058 | 2,120,693 | 1,600,529 |
| `web-editor` | 4,217,499 | 4,217,234 | 1,613,238 | 1,225,700 |
| `web-full` | 17,200,560 | 17,200,295 | 6,359,194 | 4,678,479 |
| `web-render` | 15,173,229 | 15,172,964 | 5,663,850 | 4,172,049 |

The full/render raw WASM SHA-256 values are respectively
`7482040107513ab84e26d57a97daea01c1af2e9d76e01ed8bceb46eb9e139c1b` and
`0cda808d218ebee0e60bc497ec5edcf22c541a871242598963be11fdd858955c`.

Only the full/render ceilings in `docs/release/WASM_SIZE_BUDGETS.json` change. Each replacement
uses its measured value plus 3%, rounded upward to 1,000 bytes, following the existing policy.
The other three Web budgets and the Typst budget remain unchanged. Render saves approximately
11.8% stripped and 10.8% Brotli against full on this same build; it remains the dedicated
complete SVG product rather than a new basic-SVG SKU. These values do not isolate the cost of
math from every change since the historical budget baseline.

## Installed browser observations

The installed-consumer owner packs and installs one task package at a time outside the checkout,
builds a Vite production application under a non-root base, and runs Chromium 156.0.8078.4.
Each package executes in an actual browser main thread and a module Worker. Full/render exercise
ELK planning, ordinary SVG and formulas with positive nested viewports and nonempty vector glyph
paths. The remaining packages execute their own analysis, editing and ASCII operations.

All five packages passed CSP checks and loaded exactly one explicitly selected WASM URL in each
realm, without an extra default fetch. Repeated public initialization retained the same runtime.

| Artifact | Main initialization, ms | Worker initialization, ms | Initial linear memory, bytes | Linear memory after operations, bytes |
| --- | ---: | ---: | ---: | ---: |
| `web-full` | 33.2 | 21.1 | 4,390,912 | 6,946,816 |
| `web-render` | 27.9 | 16.8 | 4,325,376 | 6,684,672 |
| `web-analysis` | 8.4 | 7.3 | 2,228,224 | 2,555,904 |
| `web-editor` | 8.6 | 6.5 | 2,228,224 | 2,555,904 |
| `web-ascii` | 10.4 | 8.5 | 2,228,224 | 2,555,904 |

Initialization timing starts after the JavaScript imports and includes the local fetch and WASM
initialization. Linear memory is the actual exported `WebAssembly.Memory.buffer.byteLength`,
not process RSS, JavaScript heap usage or a peak-memory estimate. Both realms reached the same
linear-memory sizes in this run. First math renders took 61.2/58.7 ms on the main thread and
4.1/3.4 ms in the later full/render Workers. The Workers may benefit from browser compilation
caches. These are single-run observations on a shared development machine with other work in
progress; they establish neither network download latency nor a performance regression verdict.

## Reproduce

```text
node platforms/web/scripts/build-wasm.mjs --all-packages
npm --prefix platforms/web run build:ts
npm --prefix platforms/web run build:packages
npm --prefix platforms/web run smoke
cargo run --locked -p xtask -- wasm-size-matrix --surface web \
  --web-package-root platforms/web/packages \
  --budget-file docs/release/WASM_SIZE_BUDGETS.json
npm ci --prefix playground
npm ci --prefix playground/tests
npm run install:browsers --prefix playground/tests
node platforms/web/scripts/installed-browser-consumer.mjs
```

The consumer prints tarball digests, browser requests, operations, timing and linear-memory
observations. Pages and immutable release preflight own the same installed-consumer command.
New published versions still require their normal package-group and registry reconciliation.
