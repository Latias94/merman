# Feature Surfaces

Merman uses Cargo features for three separate concerns:

- core profile features, such as `full` and `host`;
- output capability features, such as `render`, `ascii`, `raster`, and `ratex-math`;
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
| `merman` | `core-full` | Forwards to `merman-core/full`; enabled by default. |
| `merman` | `core-host` | Forwards to `merman-core/host`; enabled by default. |
| `merman-wasm` | `render` | Browser wasm-bindgen rendering surface for `@mermanjs/web`. |
| `merman-wasm` | `ascii` | Browser wasm-bindgen ASCII/Unicode surface for `@mermanjs/web`. |

The current `merman-wasm` crate is a browser/JavaScript WebAssembly package. It is not the
pure-WASM or Typst plugin surface.

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
