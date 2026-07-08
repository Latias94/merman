# Feature Surfaces

Merman uses Cargo features for three separate concerns:

- core profile features, such as `full` and `host`;
- output capability features, such as `render`, `ascii`, `raster`, and `ratex-math`;
- analysis capability features, such as `analysis` and `editor-language`;
- host capability features, such as `host-clock`, `host-random`, and `host-timing`.

Keep these concerns separate. Output features decide what Merman can produce. Host capability
features decide whether core parsing may call the ambient host for time, randomness, browser APIs,
or similar capabilities.

## Core Features

| Crate | Feature | Default | Meaning |
| --- | --- | ---: | --- |
| `merman-core` | `full` | yes | Compatibility profile for full Mermaid behavior. Enables `full-config` and `full-sanitization`. |
| `merman-core` | `full-config` | via `full` | Enables full YAML frontmatter parsing and JSON5 directive parsing through `serde_yaml` and `json5`. |
| `merman-core` | `full-sanitization` | via `full` | Enables DOMPurify-like HTML sanitization and URL canonicalization through `lol_html` and `url`. |
| `merman-core` | `host` | yes | Host capability profile. Enables `host-clock`, `host-random`, and `host-timing`. |
| `merman-core` | `host-clock` | yes | Enables `chrono/clock` and system local-time behavior. Disable for pure-WASM and Typst-style hosts. |
| `merman-core` | `host-random` | yes | Enables UUID v4 generated IDs. Disable for pure-WASM and Typst-style hosts; generated IDs become deterministic. |
| `merman-core` | `host-timing` | yes | Enables parse timing instrumentation through `web-time`. Disable for pure-WASM and Typst-style hosts. |

`merman-core --no-default-features` is the current starting point for pure-WASM and Typst work. It
is intentionally smaller and more deterministic than the default full profile. In this profile,
implicit local time falls back to UTC, generated IDs are deterministic, and parse timing
instrumentation is disabled.

Without `full-config`, closed YAML frontmatter is stripped before diagram detection, but title/config
fields from that frontmatter are not applied. Common Mermaid inline metadata remains supported by a
small built-in parser, including flowchart `@{ shape: rounded }`, sequence participant metadata,
kanban item metadata, and common JSON-like directive objects. Directives use that same built-in
parser and do not claim full JSON5 compatibility.

Without `full-sanitization`, core still filters dangerous URL protocols and conservatively escapes
HTML while preserving Mermaid line break tags such as `<br/>`. It does not claim DOMPurify parity,
does not apply caller-provided `dompurifyConfig`, and does not canonicalize URLs through the `url`
crate.

## Public Facade Features

| Crate | Feature | Meaning |
| --- | --- | --- |
| `merman` | `render` | Enables layout and SVG rendering through `merman-render`. |
| `merman` | `ascii` | Enables terminal-oriented ASCII/Unicode rendering through `merman-ascii`. |
| `merman` | `raster` | Enables PNG/JPG/PDF conversion support. |
| `merman` | `ratex-math` | Enables the pure-Rust RaTeX math backend for supported labels. |
| `merman` | `cytoscape-layout` | Enables the Cytoscape-style Architecture and Mindmap layout seam through `merman-render`. Enabled by `core-full`. |
| `merman` | `core-full` | Forwards to `merman-core/full`; enabled by default. |
| `merman` | `core-host` | Forwards to `merman-core/host`; enabled by default. |
| `merman-ascii` | `core-full` | Forwards to `merman-core/full`; enabled by default for direct `merman-ascii` users. |
| `merman-ascii` | `core-host` | Forwards to `merman-core/host`; enabled by default for direct `merman-ascii` users. |
| `merman-bindings-core` | `analysis` | Enables diagnostics analysis, validation projection, document facts, and lint rule catalog helpers; enabled by default for native binding users. |
| `merman-wasm` | `analysis` | Browser wasm-bindgen diagnostics, validation, document facts, and lint rule catalog surface for `@mermanjs/web/core` and render/full presets. |
| `merman-wasm` | `render` | Browser wasm-bindgen rendering surface for `@mermanjs/web`. |
| `merman-wasm` | `ascii` | Browser wasm-bindgen ASCII/Unicode surface for `@mermanjs/web`; pair with `core-full`/`core-host` only when the artifact needs those core profiles. |
| `merman-wasm` | `core-full` | Browser package full core profile; enabled by default. |
| `merman-wasm` | `core-host` | Browser package host capability profile; enabled by default. |
| `merman-wasm` | `cytoscape-layout` | Browser opt-in for Architecture and Mindmap Cytoscape-style layout when building non-full presets. Enabled by `core-full`. |
| `merman-wasm` | `editor-language` | Browser editor-language APIs; implies `analysis` and adds `merman-editor-core`. |
| `merman-wasm` | `ratex-math` | Browser package RaTeX math rendering support; implies `render`. |
| `merman-typst-plugin` | `render` | Typst wasm-minimal-protocol SVG render surface; enabled by default. |
| `merman-typst-plugin` | `analysis` | Typst validation surface; enabled by default so the package `validate-mermaid` API keeps existing behavior. |
| `merman-typst-plugin` | `core-full` | Typst no-host artifact with full config and sanitization support. |
| `merman-typst-plugin` | `core-host` | Opt-in host capability profile; do not enable for Typst package builds. |
| `merman-typst-plugin` | `cytoscape-layout` | Typst opt-in for Architecture and Mindmap Cytoscape-style layout. Enabled by `core-full`. |
| `merman-typst-plugin` | `ratex-math` | Typst plugin artifact plus RaTeX math rendering support; implies `render`. |

The current `merman-wasm` crate is a browser/JavaScript WebAssembly package. It is not the
pure-WASM or Typst plugin surface. The Typst surface is `merman-typst-plugin`, which uses
wasm-minimal-protocol and must keep browser/wasm-bindgen imports out of package builds.

Bindings expose the selected registry profile and per-family parser/render capability metadata so
hosts can inspect the actual full/tiny diagram surface in slim artifacts instead of inferring it
from package names.

The public `merman` facade disables `merman-ascii` default features internally and forwards
`core-full`/`core-host` with weak optional dependency features. This keeps direct `merman-ascii`
usage backwards-compatible while allowing `merman --no-default-features --features ascii` and
browser ASCII presets to stay on the slim core profile.

The binding crates keep `analysis` separate from `render` and `ascii`. Defaults preserve the
diagnostics and validation surface for existing native, browser, and Typst users, while slim builds
such as `merman-wasm --no-default-features --features ascii` can omit `merman-analysis`, JSON5/YAML
lint support, and editor-language dependencies.

## Host Profiles

| Profile | Intended host | Feature posture |
| --- | --- | --- |
| Full/native | Rust applications, CLI, native bindings | Keep defaults unless the caller explicitly wants deterministic host behavior. |
| Browser WASM | `@mermanjs/web` and wasm-bindgen consumers | Browser APIs are allowed and must be documented as browser-only. |
| Pure WASM | `wasm32-unknown-unknown` without JS/WASI imports | Start from `merman-core --no-default-features` or `merman --no-default-features`; no `full`, no `host`. |
| Typst WASM | Typst plugin / `wasmi` host | Same as pure WASM, plus only the wasm-minimal-protocol imports are allowed. |

## Rules For New Features

- Document every public Cargo feature here and near the defining `[features]` table.
- Do not hide host access behind parser or render features; name it as a host capability.
- Pure-WASM and Typst profiles must not depend on `wasm-bindgen`, `js-sys`, browser randomness,
  JavaScript date/time, WASI, browser panic hooks, full YAML/JSON5 parsing, DOMPurify-like HTML
  rewriting, or URL canonicalization dependencies.
- Use `xtask profile-budget` gates when changing dependencies or host capability features.
