# Workflow Recommendations

Use this table as a starting point, then verify the exact current manifest and package surface before publishing it. Cargo features are additive; artifact profiles and npm package identities are the distribution contract.

| User need | Recommended surface | Typical selection | User-visible trade-off |
| --- | --- | --- | --- |
| Static blog or documentation generator in Rust | `merman` | Default `complete-svg`, or `default-features = false` with `svg,layout-cytoscape,layout-elk,math` | Complete SVG and Mermaid-compatible layouts; math increases binary/WASM size. |
| Static blog using a command line pipeline | `merman-cli` | Release artifact profile `cli-release` | Broad export and CLI tooling; use the analysis profile instead for lint-only jobs. |
| Dynamic browser diagram viewer | `@mermanjs/web-render` | Standalone render package | SVG + Cytoscape + ELK + math without analysis/editor/ASCII APIs. |
| Browser diagnostics or lint | `@mermanjs/web-analysis` | Standalone analysis package | Detection, validation, facts, and diagnostics without SVG or editor APIs. |
| Browser app with editing | `@mermanjs/web`, or `@mermanjs/web-render` plus `@mermanjs/web-editor` across isolated realms | Full package alone for one-realm convenience; render on the main thread and use analysis-plus-editor in the worker only when that split is measured | Do not call `web-editor` editor-only, pair full with slim duplicates, or infer capabilities from package names. |
| Embedded editor integration | `merman-editor-core`, `merman-analysis`, or `merman` | `merman` with `default-features = false, features = ["analysis", "editor"]`, or depend on the lower crate directly | No SVG backend is needed for diagnostics; do not apply facade feature syntax to a lower crate that lacks that feature. |
| LSP server process | `merman-lsp` | Published binary, or `--no-default-features --features stdio`; use `lsp-stdio-release` for exact release evidence | The binary requires the `stdio` leaf. |
| CLI lint/CI gate | `merman-cli` | `--no-default-features --features analysis` | Smallest CLI executable path; no SVG, raster, layout, icon, network, or Markdown conversion. |
| Embedded lint library | `merman-analysis` | Depend on the crate directly | The crate is already the analysis surface; it does not expose a facade-style `analysis` feature. |
| Markdown/MDX renderer | `merman-cli` | `--no-default-features --features svg,markdown`, add `parallel-markdown` only for measured batch throughput | Markdown conversion and SVG are explicit; Rayon adds worker-pool dependencies. |
| Terminal or text-only preview | `merman`/`merman-cli` | Disable defaults, select `ascii`, and query `ascii_capabilities` | No SVG renderer required; the current 14 admitted families are graded Full, Partial, or Summary, and SVG support does not imply ASCII support. |
| Node or static-site generation | Current CLI subprocess; `@mermanjs/node` only after admission | Prefer an admitted N-API/native candidate in the future; compare Node-targeted WASM separately | No in-process Node package is currently admitted; parity and all-target evidence gate publication. |
| Typst document rendering | `@preview/merman:<published-version>` | Verify the package-to-Merman version mapping and `typst-wasm` profile | Typst has an independent release track and constrained ABI; do not infer the target revision or browser/math closure from the package name. |
| Python or Flutter embedding | Declared `merman` release channel on PyPI or pub.dev | Verify that the compared target version is live before using present tense | The target may define a full ABI 3 SKU before registries contain it; no slim prebuilt SKU is implied. |
| Android or Apple embedding | Versioned GitHub Release AAR or XCFramework | Artifact-only channel after the selected release publishes | Do not describe credential-blocked Maven Central or registry-blocked SwiftPM channels as published. |
| C ABI embedding | Versioned `merman-ffi` source crate from crates.io | Build the source crate after verifying the target version is published | The artifact profile is a reproducible build contract, not a downloadable binary SDK. |

## Migration vocabulary

Use concrete replacements only when the compared range includes the breaking change:

| Old name | Current selection |
| --- | --- |
| `core-full` | No direct replacement: core language semantics are invariant; select the required output leaves. |
| `core-host` | Select the required `system-clock`, `system-timezone`, `system-random`, and/or `system-timing` adapters. |
| Historical `full`/`tiny` registry profiles | Select observable capability leaves or an exact artifact profile. |
| `render` | `svg` |
| `raster` | `png`, `jpeg`, and/or `pdf` |
| `cytoscape-layout` | `layout-cytoscape` |
| `elk-layout` | `layout-elk` |
| `ratex-math` | `math` |
| Web subpaths and raw WASM fallback | Standalone package identities with a declared runtime contract. |

These old names and subpaths have no compatibility aliases. Match replacements by declared
capabilities: an older basic render subpath is not automatically equivalent to the current
complete `@mermanjs/web-render` package.
