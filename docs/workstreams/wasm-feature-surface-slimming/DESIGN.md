# WASM Feature Surface Slimming

Status: Closed
Last updated: 2026-10-29

This is the historical design for WFS-010 through WFS-120, completed on 2026-06-10.
[Evidence And Gates](EVIDENCE_AND_GATES.md#wfs-120-final-verification-and-closeout) retains the
measurements and final verification. Profile names and dependency observations below describe
that work, not current package admission. Use the [capability guide](../../FEATURES.md),
[package surface guide](../../release/PACKAGE_SURFACES.md), and
[WASM surface ADR](../../adr/0069-wasm-package-surface-semantics.md) for their current owners.

## Why This Lane Exists

Merman has grown from a Rust parser/render library into a multi-surface product: native Rust, CLI,
FFI, UniFFI, browser WASM, and platform packages. The current feature graph is still mostly shaped
around "full Mermaid parity by default" and browser WASM. That is the right default for many hosts,
but it makes non-JS WebAssembly targets pay for capabilities they cannot provide.

The immediate forcing function is the Typst plugin environment. Typst loads plugins through
`wasmi` and the wasm-minimal-protocol. It does not run wasm-bindgen glue, does not provide
browser/Node JS imports, and requires plugin functions to be pure and deterministic. A direct
`merman` dependency currently does not satisfy that environment.

## Evidence Snapshot

- `repo-ref/typst/crates/typst-library/src/foundations/plugin.rs` shows that Typst provides only
  the wasm-minimal-protocol imports under `typst_env`, checks for exported `memory`, and calls
  exported functions with `i32` buffer lengths.
- A parser-only local probe depending on `merman` with `default-features = false` built a
  `wasm32-unknown-unknown` module of about 4.3 MB. The import table still contained
  `__wbindgen_placeholder__` imports for WebCrypto randomness and `js_sys::Date`.
- `cargo build --profile wasm-size -p merman-wasm --target wasm32-unknown-unknown` produced about
  8.5 MB with default features and 49 imports. With `--no-default-features`, the same transport was
  about 2.7 MB with 25 imports, but still contained JS Date and WebCrypto imports.
- `cargo tree -p merman-core --target wasm32-unknown-unknown -e normal` shows direct core ownership
  of `chrono`, `uuid`, `web-time`, `lol_html`, `url`, `serde_yaml`, `json5`, and related transitive
  dependencies.
- After the host/config/sanitization splits, the Typst bridge-only profile strips to 33,412 bytes,
  while the default Typst render profile strips to 5,414,863 bytes. The remaining render weight is
  real code/data, not only custom metadata.
- The default Typst render path does not include browser, raster, host-random, math, ASCII, YAML,
  JSON5, DOM sanitization, or URL canonicalization dependencies. The next high-impact candidates
  are layout/render dependencies (`manatee`/`nalgebra`, `roughr-merman`, `pulldown-cmark`) and
  generated static data such as font metrics.
- `repo-ref/mermaid-rs-renderer` is a useful reference for package boundaries and feature slicing,
  but it is not a byte-size target because it has a smaller product/parity scope and does not carry
  this repository's `dugong`/`manatee`/`roughr` parity chain.

## Problem

There are three different concerns currently mixed together:

- Mermaid parity scope: which diagram families and syntax features are compiled and registered.
- Host capability scope: whether the target can provide current time, timezone, randomness, JS
  objects, panic hooks, console, WebCrypto, or browser glue.
- Output scope: semantic JSON, typed render model, SVG, ASCII, raster/PDF, browser TypeScript
  wrappers, and future Typst plugin bytes.

Feature flags only partially expressed these concerns at the start of this lane. `merman` defaults
were empty, while `merman-core` still pulled broad parser/sanitizer/time/random dependencies.
`merman-wasm` correctly targets browsers, but its name can make downstream users assume it is
generic WebAssembly. The old full/tiny flag also needed to become a real dependency slimming
boundary, not only a detector registration switch.

## Target State

When this lane closes:

- `merman-core` has a small, host-neutral profile that can build for `wasm32-unknown-unknown`
  without `wasm-bindgen`, `js-sys`, WebCrypto, JS Date, or current-time imports.
- Mermaid full parity remains the default release posture for normal Rust/native/browser users.
- Browser WASM and Typst/plugin WASM are separate package surfaces with separate dependency and
  import allowlists.
- Time and randomness are explicit host capabilities. Deterministic/no-host profiles cannot
  accidentally call system local time or entropy.
- Heavy core dependencies such as YAML, JSON5 directives, DOM-like sanitization, URL canonicalizing,
  and broad family registration are either justified in the relevant profile or feature-gated.
- The existing diagram family facts module projects feature-profile-specific detector/parser/render
  registration instead of scattering `cfg` decisions across call sites.
- CI has a small set of commands that fail if the Typst/pure-wasm profile regresses into JS imports.

## In Scope

- Feature flag redesign across `merman-core`, `merman-render`, `merman`, `merman-bindings-core`,
  and `merman-wasm`.
- Optional new crate for Typst/wasm-minimal-protocol packaging.
- Host capability boundaries for time, timezone, randomness, timing instrumentation, and JS-only
  panic hooks.
- Dependency slimming for parser preprocessing, sanitization, URL handling, and diagram-family
  registration.
- Size/import measurement scripts or xtask commands.
- ADR updates if public defaults or package semantics change.

## Out Of Scope

- Reducing Mermaid parity quality to chase size.
- Replacing `dugong` or `manatee` algorithms as part of this lane.
- Raster/PDF slimming; those remain behind the existing `raster` feature.
- Shipping a full Typst package wrapper before the pure wasm module is proven loadable.
- Changing the browser package to stop using wasm-bindgen.

## Architecture Direction

Treat host capability as a first-class boundary. The browser package may depend on wasm-bindgen and
JS capabilities; the Typst plugin may not. The Rust/native full profile may use local time and
system entropy; deterministic/pure profiles must receive fixed time and deterministic identifiers
or disable families that require unavailable host capabilities.

Use existing domain structure where possible. The diagram family facts module is the right owner for
profile-specific registration because it already projects detector, parser, typed render parser,
metadata, and fallback policy. It should learn profiles rather than forcing each adapter to know
which families are safe.

WFS-080 made this boundary concrete: detector, semantic parser, typed render parser, supported
diagram facts, and supported metadata now project from the same `BaselineRegistryProfile`. The
tiny/no-default profile excludes Mermaid's current full-only large-feature registrations
(`mindmap`, `architecture`, `flowchart-elk`) from parser/render registries and metadata, while
preserving normal flowchart aliases.

Do not create a generic trait forest before the second adapter exists. Time and randomness already
have multiple real adapters: native system, browser JS, deterministic test/probe, and Typst/pure
wasm. Those are real seams. Build the smallest interfaces that keep the capability decision out of
family parsers.

### Contract Decisions

- Full/native/browser compatibility remains the default. The facade disables implicit core
  defaults on its dependency edge and selects the full core profile explicitly, so opting out of
  facade defaults also opts out of broad core dependencies. See
  [ADR 0006](../../adr/0006-feature-flags-tiny-vs-full.md) and
  [ADR 0012](../../adr/0012-tiny-scope.md) for the original feature decision.
- Full preprocessing, DOM sanitization, and URL canonicalization preserve
  [ADR 0020](../../adr/0020-sanitization-and-security-level.md) and
  [ADR 0023](../../adr/0023-url-sanitization-braintree-port.md). Minimal profiles must escape
  conservatively and reject unsafe links; reducing dependencies must not weaken full defaults.
- Host-neutral parsing receives fixed dates, UTC offsets, and deterministic IDs, or rejects a
  family that needs an unavailable capability. An admitted Gantt input with the same explicit date
  and offset must retain the same date semantics across native and pure-WASM paths.
- Browser and Typst transports remain separate adapters over the host-neutral facade. The original
  research also cites this [Typst integration report](https://github.com/HSGamer/typst-mmdr/issues/4#issuecomment-4624101901).
  Browser code keeps wasm-bindgen; Typst requires exported memory and only its two protocol imports.
  The [binding strategy](../../adr/0066-ffi-binding-strategy.md) owns the shared facade boundary.
- Dependency, import, export, and size measurements precede broad feature changes. Full behavior
  tests remain independent from pure-WASM admission, and the
  [runtime policy](../../adr/0008-async-and-runtime.md) still governs host integration.

## Acceptance Boundary

The completed lane used these acceptance conditions:

- a parser-only pure wasm probe has no JS/wasm-bindgen imports;
- a Typst minimal-protocol probe can be loaded by Typst or a compatible wasmi runner;
- browser WASM has documented default and slim package variants;
- full native/browser parity gates still pass;
- dependency and import budgets are documented in release notes or package-surface docs.

Transport validation did not establish Typst registry publication or full-family SVG coverage.
Rough/hand-drawn admission also required a separate deterministic-randomness decision. These were
scope boundaries, not reasons to keep this implementation lane active.
