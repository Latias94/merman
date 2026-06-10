# WASM Feature Surface Slimming -- Evidence And Gates

Status: Open
Last updated: 2026-06-10

## Current Evidence

### Typst Plugin Protocol

Source: `repo-ref/typst/crates/typst-library/src/foundations/plugin.rs`

Observed contract:

- Typst loads raw WebAssembly bytes through `wasmi`.
- The module must export `memory`.
- Callable plugin exports accept only `i32` parameters and return one `i32`.
- The host links only:
  - `typst_env::wasm_minimal_protocol_write_args_to_buffer`
  - `typst_env::wasm_minimal_protocol_send_result_to_host`
- The documentation explicitly says WASI does not directly work and plugins cannot print, read
  files, or access similar system capabilities.

### Parser-Only Probe

Probe location: `/tmp/merman-typst-probe`

Probe shape:

- `cdylib`
- `wasm32-unknown-unknown`
- `merman = { path = ".../crates/merman", default-features = false }`
- one exported function calling `Engine::parse_metadata_with_type_sync` for a flowchart with fixed
  date.

Command:

```bash
cargo build --release --target wasm32-unknown-unknown
wasm-tools print /tmp/merman-typst-probe/target/wasm32-unknown-unknown/release/merman_typst_probe.wasm
```

Observed result:

- Unstripped wasm: about 4.3 MB.
- Import count: 12.
- Import table included `__wbindgen_placeholder__` and `__wbindgen_externref_xform__`.
- JS capability imports included WebCrypto randomness and `js_sys::Date` timezone/time functions.

Conclusion: parser-only `merman` is not currently Typst-plugin-compatible.

### Browser WASM Package

Default command:

```bash
cargo build --profile wasm-size -p merman-wasm --target wasm32-unknown-unknown
```

Observed result:

- `target/wasm32-unknown-unknown/wasm-size/merman_wasm.wasm`: about 8.5 MB.
- Import count: 49.
- Expected browser imports include wasm-bindgen glue, console panic hook, WebCrypto, JS Date, JS
  performance, and serde-wasm-bindgen helpers.

No-default command:

```bash
cargo build --profile wasm-size -p merman-wasm --target wasm32-unknown-unknown --no-default-features
```

Observed result:

- `target/wasm32-unknown-unknown/wasm-size/merman_wasm.wasm`: about 2.7 MB.
- Import count: 25.
- Still includes wasm-bindgen glue, WebCrypto, and JS Date imports.

Conclusion: disabling render/ascii lowers size but does not produce a pure wasm surface.

### Dependency Roots

Commands:

```bash
cargo tree -p merman-core --target wasm32-unknown-unknown -e normal --depth 2
cargo tree -p merman-core --target wasm32-unknown-unknown -e normal -i wasm-bindgen --depth 6
cargo tree -p merman-render --target wasm32-unknown-unknown -e normal --depth 2
```

Important roots:

- `chrono` pulls `js-sys`/`wasm-bindgen` on `wasm32-unknown-unknown` through default clock/timezone
  behavior.
- `uuid` pulls `wasm-bindgen` through the `js` feature and WebCrypto-backed UUID generation.
- `web-time` pulls `js-sys`/`wasm-bindgen` on wasm targets.
- `lol_html`, `url`, `serde_yaml`, and `json5` are direct `merman-core` dependencies and are major
  core slimming candidates.
- `roughr` pulls `rand`/`getrandom` in render paths and needs deterministic/pure-wasm review before
  any Typst SVG subset claims hand-drawn parity.

### WFS-030 Host Clock Split

Change:

- workspace `chrono` now disables default features and enables only `std`;
- `merman-core` default features include `host-clock`, which owns `chrono/clock`;
- `merman-core --no-default-features` uses deterministic UTC fallback behavior for implicit local
  time instead of `chrono::Local`;
- native/render/xtask crates that need system time opt into `chrono/clock` explicitly.

Validation:

```bash
cargo fmt --check -p merman-core -p merman-render -p xtask
cargo check -p merman-core --no-default-features --target wasm32-unknown-unknown
cargo nextest run -p merman-core gantt
cargo nextest run -p merman-core --no-default-features gantt
cargo nextest run -p merman-core --no-default-features runtime
cargo nextest run -p xtask profile_budget
```

Post-change dependency evidence:

```bash
cargo tree -p merman-core --no-default-features --target wasm32-unknown-unknown -e normal -i js-sys --depth 8
cargo tree -p merman-core --no-default-features --target wasm32-unknown-unknown -e normal -i wasm-bindgen --depth 8
cargo tree -p merman-core --no-default-features --target wasm32-unknown-unknown -e features -i chrono --depth 8
```

Observed result:

- `chrono` now appears only through its `std`/`alloc` feature path for `merman-core
  --no-default-features`;
- remaining `js-sys` root is `web-time`;
- remaining `wasm-bindgen` roots are `uuid` and `web-time`.

### WFS-040 Host Random Split

Change:

- workspace `uuid` now disables default features and enables only `std`;
- `merman-core` default features include `host-random`, which owns `uuid/js` and `uuid/v4`;
- block and gitGraph generated IDs use a core runtime helper rather than direct UUID calls;
- `merman-core --no-default-features` uses deterministic generated hex IDs.

Validation:

```bash
cargo fmt --check -p merman-core -p merman-render -p xtask
cargo check -p merman-core
cargo check -p merman-core --no-default-features --target wasm32-unknown-unknown
cargo check -p merman-wasm --target wasm32-unknown-unknown
cargo nextest run -p merman-core block
cargo nextest run -p merman-core git_graph
cargo nextest run -p merman-core --no-default-features block
cargo nextest run -p merman-core --no-default-features git_graph
cargo nextest run -p merman-core --no-default-features runtime
```

Post-change dependency evidence:

```bash
cargo tree -p merman-core --no-default-features --target wasm32-unknown-unknown -e normal --depth 8 | rg "uuid|getrandom|wasm-bindgen|js-sys|web-time"
cargo tree -p merman-core --no-default-features --target wasm32-unknown-unknown -e normal -i wasm-bindgen --depth 8
cargo run -p xtask -- profile-budget check-deps --profile pure-wasm --package merman-core --target wasm32-unknown-unknown --no-default-features --depth 3
```

Observed result:

- `uuid` and `getrandom` no longer appear in the `merman-core --no-default-features` wasm tree;
- `profile-budget` failures dropped to `web-time`, `js-sys`, and `wasm-bindgen`;
- remaining `wasm-bindgen`/`js-sys` root is `web-time`, which is WFS-050.

### WFS-050 Host Timing Split

Change:

- `merman-core` default features include `host-timing`, which owns `web-time`;
- core parse timing now goes through runtime timing helpers;
- `merman-core --no-default-features` disables parse timing instrumentation and does not link
  `web-time`.

Validation:

```bash
cargo fmt --check -p merman-core -p merman-render -p xtask
cargo check -p merman-core
cargo check -p merman-core --no-default-features --target wasm32-unknown-unknown
cargo check -p merman-wasm --target wasm32-unknown-unknown
cargo nextest run -p merman-core --no-default-features runtime
cargo nextest run -p merman-core --no-default-features gantt
cargo run -p xtask -- profile-budget check-deps --profile pure-wasm --package merman-core --target wasm32-unknown-unknown --no-default-features --depth 3
cargo run -p xtask -- profile-budget check-deps --profile typst-wasm --package merman-core --target wasm32-unknown-unknown --no-default-features --depth 3
```

Post-change dependency evidence:

```bash
cargo tree -p merman-core --no-default-features --target wasm32-unknown-unknown -e normal --depth 8 | rg "uuid|getrandom|wasm-bindgen|js-sys|web-time"
```

Observed result:

- pure-wasm and typst-wasm dependency gates pass for `merman-core --no-default-features`;
- the dependency tree filter has no matches for `uuid`, `getrandom`, `wasm-bindgen`, `js-sys`, or
  `web-time`;
- render/layout crates still use timing internally and remain browser/full-surface work until later
  profile or crate-splitting tasks.

### WFS-060 Full Config Split

Change:

- `merman-core` default `full` now enables `full-config`;
- `full-config` owns `serde_yaml`, full YAML frontmatter parsing, and the `json5` directive parser;
- `merman-core --no-default-features` strips closed YAML frontmatter before detection but does not
  apply frontmatter title/config;
- no-default core uses a small built-in inline config parser for common Mermaid metadata and
  directive objects, including flowchart `@{ shape: rounded }`, sequence participant metadata, and
  kanban item metadata;
- the built-in inline parser also covers existing flowchart metadata tests for multiline double
  quoted strings and YAML-style `|` literal block labels.

Validation:

```bash
cargo check -p merman-core
cargo check -p merman-core --no-default-features --target wasm32-unknown-unknown
cargo nextest run -p merman-core inline_config
cargo nextest run -p merman-core --no-default-features inline_config
cargo nextest run -p merman-core parse_merges_frontmatter_and_directive_config parse_metadata_with_type_sync_moves_init_config_without_detection parse_diagram_sequence_extended_participant_syntax_parses_type_override parse_diagram_flowchart_node_data_multiple_properties_same_line knbn_37_ticket_metadata
cargo nextest run -p merman-core --no-default-features frontmatter_is_stripped_without_full_config_but_config_is_not_applied parse_returns_malformed_frontmatter_error_for_unclosed_frontmatter parse_metadata_with_type_sync_moves_init_config_without_detection parse_merges_init_directive_numeric_values_like_upstream parse_diagram_sequence_extended_participant_syntax_parses_type_override parse_diagram_flowchart_node_data_multiple_properties_same_line knbn_37_ticket_metadata
cargo nextest run -p merman-core --no-default-features parse_diagram_flowchart_node_data
cargo nextest run -p merman-core --no-default-features parse_diagram_sequence_extended_participant_syntax
cargo nextest run -p merman-core --no-default-features metadata
cargo run -p xtask -- profile-budget check-deps --profile pure-wasm --package merman-core --target wasm32-unknown-unknown --no-default-features --depth 3
cargo run -p xtask -- profile-budget check-deps --profile typst-wasm --package merman-core --target wasm32-unknown-unknown --no-default-features --depth 3
```

Post-change dependency evidence:

```bash
cargo tree -p merman-core --no-default-features --target wasm32-unknown-unknown -e normal --depth 8 | rg "serde_yaml|unsafe-libyaml|json5|pest"
cargo tree -p merman-core --target wasm32-unknown-unknown -e features -i serde_yaml --depth 6
cargo tree -p merman-core --target wasm32-unknown-unknown -e features -i json5 --depth 6
```

Observed result:

- the no-default wasm dependency tree filter has no matches for `serde_yaml`, `unsafe-libyaml`,
  `json5`, or `pest`;
- default/full dependency roots remain explicit through `full -> full-config`;
- pure-wasm and typst-wasm dependency budget gates still pass for
  `merman-core --no-default-features`.

### WFS-070 Full Sanitization Split

Change:

- `merman-core` default `full` now enables `full-sanitization`;
- `full-sanitization` owns DOMPurify-like HTML rewriting through `lol_html`;
- `full-sanitization` owns URL canonicalization through the `url` crate;
- `merman-core --no-default-features` keeps dangerous URL protocol filtering but returns the
  cleaned URL without `url` crate canonicalization;
- no-default core uses conservative HTML escaping while preserving Mermaid line break tags such as
  `<br/>`;
- public facade defaults remain full/host, while `merman --no-default-features` now forwards to a
  minimal core dependency graph.

Validation:

```bash
cargo fmt --check
cargo check -p merman-core
cargo check -p merman-core --no-default-features --target wasm32-unknown-unknown
cargo check -p merman --no-default-features --target wasm32-unknown-unknown
cargo check -p merman-wasm --target wasm32-unknown-unknown
cargo nextest run -p xtask profile_budget
cargo nextest run -p merman-core sanitize_text sanitize_url remove_script format_url
cargo nextest run -p merman-core --no-default-features sanitize_text sanitize_url remove_script format_url inline_config parse_diagram_flowchart_node_data parse_diagram_sequence_extended_participant_syntax_parses_type_override knbn_37_ticket_metadata
cargo run -p xtask -- profile-budget check-deps --profile pure-wasm --package merman-core --target wasm32-unknown-unknown --no-default-features --depth 8
cargo run -p xtask -- profile-budget check-deps --profile typst-wasm --package merman-core --target wasm32-unknown-unknown --no-default-features --depth 8
```

Post-change dependency evidence:

```bash
cargo tree -p merman --no-default-features --target wasm32-unknown-unknown -e normal --depth 4
cargo tree -p merman-core --no-default-features --target wasm32-unknown-unknown -e normal --depth 8 | rg "serde_yaml|unsafe-libyaml|json5|pest|lol_html|url|idna|icu_|cssparser|selectors|encoding_rs|uuid|getrandom|wasm-bindgen|js-sys|web-time"
```

Observed result:

- `profile-budget` reports zero failures for pure-wasm and typst-wasm core profiles;
- the no-default wasm dependency tree filter has no matches for full config, full sanitizer,
  host-random, host-timing, or wasm-bindgen/browser crates;
- default `merman-wasm` still builds for `wasm32-unknown-unknown`, preserving the browser
  wasm-bindgen package surface.

### Typst Render WASM Size Baseline

The Typst package size is dominated by `merman_typst_plugin.wasm`; non-wasm package files are only
on the order of tens of kilobytes. Treat stripped wasm size and real code/data size as separate
signals because custom metadata can dominate unstripped deltas.

Observed stripped wasm sizes:

| Typst feature profile | Stripped wasm bytes | Notes |
| --- | ---: | --- |
| `--no-default-features` | 33,412 | Bridge-only surface; proves wasm-minimal-protocol glue is small. |
| default `render` | 5,414,863 | Current package default. |
| `core-full` | 6,262,880 | About 848 KB above default render. |
| `ratex-math` | 8,238,005 | About 2.82 MB above default render. |

Default render section ownership snapshot:

- `code`: about 4.50 MB;
- `data`: about 0.89 MB;
- custom `name`: about 1.61 MB before strip.

Conclusion:

- previous `wasm-tools strip --all` work mainly removed about 1.6 MB of metadata;
- further meaningful reductions must target real `code`/`data` ownership, not only custom sections.

### Typst Default Render Dependency Audit

Confirmed outside the default Typst render wasm path:

- `core-full`: `serde_yaml`, `json5`, `lol_html`, `url`;
- `core-host`: `uuid`, `web-time`, `getrandom/js`, `chrono/clock`;
- browser wrapper crates: `wasm-bindgen`, `serde-wasm-bindgen`, `console_error_panic_hook`,
  `js-sys`;
- raster/CLI crates: `image`, `resvg`, `usvg`, `tiny-skia`, `svg2pdf`, `png`, `clap`,
  `reqwest`, `rayon`;
- optional math and ASCII surfaces: `ratex-*`, `merman-ascii`;
- `roughr-merman/host-random`.

The real default render weight is concentrated in the render path:

- `merman-core`;
- `merman-render`;
- `dugong`;
- `manatee` and `nalgebra`;
- `roughr-merman`;
- `pulldown-cmark`;
- `logos`, `lalrpop-util`, `regex`;
- `chrono`, `serde_json`, `htmlize`, `base64`, `unicode-width`.

Priority slimming candidates:

1. `manatee -> nalgebra`: likely high reward, high risk. It backs architecture/mindmap COSE/FCoSE
   layout and needs family/profile strategy, not a blind dependency deletion.
2. `roughr-merman`: medium-risk candidate for making hand-drawn rendering opt-in.
3. `pulldown-cmark`: potentially meaningful, but affects markdown/html labels and needs family
   evidence before gating.
4. generated static data: inspect generated font metric tables such as
   `crates/merman-render/src/generated/font_metrics_flowchart_11_12_2.rs` because the default
   stripped wasm still carries nearly 0.9 MB of data.

`repo-ref/mermaid-rs-renderer` is useful as a module-boundary reference, not as a byte-size target.
It has a different product scope and does not carry the same `dugong`/`manatee`/`roughr` parity
chain.

### WFS-080 Family Profile Projection

Change:

- `family.rs` now projects semantic parser facts, typed render parser facts, supported diagram
  facts, and supported diagram metadata from `BaselineRegistryProfile`;
- `DiagramRegistry` and `RenderDiagramRegistry` now expose full and tiny pinned-baseline
  constructors and their feature-selected constructor follows the crate's `full` feature;
- public `supported_diagrams()` now reports the current feature-selected profile, and
  `supported_diagrams_for_profile(profile)` exposes explicit profile queries for tests and future
  bindings;
- tiny/no-default excludes the full-only large-feature registrations `mindmap`, `architecture`,
  and `flowchart-elk`, while keeping ordinary flowchart aliases such as `flowchart-v2` and
  `flowchart`;
- tests that exercise full-only known-type parsing are now gated to the full profile, and
  no-default tests assert those known-type parsers are unsupported.

Validation:

```bash
cargo fmt --check -p merman-core
cargo check -p merman-core
cargo check -p merman-core --no-default-features --target wasm32-unknown-unknown
cargo nextest run -p merman-core registry
cargo nextest run -p merman-core --no-default-features registry
cargo nextest run -p merman-bindings-core metadata
```

### WFS-090 WASM Size Matrix

Change:

- added `cargo run -p xtask -- wasm-size-matrix` to build and measure named browser and Typst
  feature presets;
- browser presets are `browser-core`, `browser-render`, `browser-ascii`, `browser-full`, and
  `browser-ratex-math`;
- Typst presets are `typst-bridge`, `typst-render`, `typst-core-full`, and `typst-ratex-math`;
- each row reports raw artifact bytes and stripped bytes from a stripped copy under
  `target/wasm-size-matrix/`, leaving the build artifact in place;
- `docs/release/PACKAGE_SURFACES.md` and `crates/merman-wasm/README.md` now explicitly label
  `merman-wasm` as the browser/wasm-bindgen surface, separate from Typst/pure wasm.

Validation:

```bash
cargo nextest run -p xtask wasm_size_matrix
cargo run -p xtask -- wasm-size-matrix --surface typst --preset typst-bridge --no-strip
cargo run -p xtask -- wasm-size-matrix --surface typst --preset typst-bridge
```

Observed browser matrix:

| Preset | Default features | Extra features | Raw bytes | Stripped bytes |
| --- | --- | --- | ---: | ---: |
| `browser-core` | no | none | 1,862,617 | 1,345,196 |
| `browser-render` | no | `render` | 7,412,346 | 5,610,023 |
| `browser-ascii` | no | `ascii` | 3,874,343 | 2,929,536 |
| `browser-full` | yes | none | 8,866,039 | 6,718,352 |
| `browser-ratex-math` | yes | `ratex-math` | 12,145,965 | 9,446,738 |

Observed Typst matrix:

| Preset | Default features | Extra features | Raw bytes | Stripped bytes |
| --- | --- | --- | ---: | ---: |
| `typst-bridge` | no | none | 47,287 | 33,412 |
| `typst-render` | yes | none | 7,025,842 | 5,417,005 |
| `typst-core-full` | yes | `core-full` | 8,090,998 | 6,263,908 |
| `typst-ratex-math` | yes | `ratex-math` | 10,544,347 | 8,240,147 |

Immediate reading:

- browser transport-only still costs about 1.35 MB stripped because it includes wasm-bindgen,
  serde-wasm-bindgen, panic hook, and metadata helpers;
- browser `render` adds about 4.26 MB stripped over browser core;
- browser `ascii` adds about 1.58 MB stripped over browser core;
- Typst bridge-only remains tiny at 33,412 bytes stripped;
- Typst render and browser render are close in render-code size once transport overhead is
  separated.

### WFS-100 Typst Plugin Transport Smoke

Change:

- confirmed that the existing `merman-typst-plugin` crate is the experimental Typst transport;
- added `cargo run -p xtask -- typst-plugin-smoke --wasm <plugin.wasm>`, which loads the plugin
  with `wasmi`, links the two wasm-minimal-protocol `typst_env` functions, calls
  `render_svg_json`, and verifies the returned JSON contains a successful SVG payload;
- corrected Typst plugin docs so the default artifact is described as the render artifact and the
  bridge-only artifact is explicitly `--no-default-features`.

Validation:

```bash
cargo check -p xtask
cargo build -p xtask
cargo build -p merman-typst-plugin --profile wasm-size --target wasm32-unknown-unknown
target/debug/xtask profile-budget check-wasm --profile typst-wasm --wasm target/wasm32-unknown-unknown/wasm-size/merman_typst_plugin.wasm
target/debug/xtask typst-plugin-smoke --wasm target/wasm32-unknown-unknown/wasm-size/merman_typst_plugin.wasm
```

Observed default render artifact:

- imports: exactly
  `typst_env::wasm_minimal_protocol_send_result_to_host` and
  `typst_env::wasm_minimal_protocol_write_args_to_buffer`;
- exports: `memory`, `abi_version`, `package_version`, `render_svg_json`, `validate_json`,
  `__data_end`, and `__heap_base`;
- size: 7,026,184 raw bytes;
- smoke: `render_svg_json` returned 10,442 JSON bytes with a 9,875-byte SVG payload for a flowchart
  fixture.

### WFS-110 Release And Compatibility Semantics

Change:

- added ADR-0069 to freeze WASM package surface semantics and alternatives;
- updated package-surface release notes with compatibility/migration rules and surface-specific
  gates;
- updated the release operator guide with browser preset and Typst transport checks;
- updated README entry points, feature-surface summary, workspace crate table, and links.

Decision summary:

- `@mermanjs/web` stays one npm package and publishes `browser-full` by default.
- Browser slim presets remain source-build presets, not public npm package variants.
- `merman-wasm` is browser/wasm-bindgen only.
- `merman-typst-plugin` owns Typst-compatible wasm-minimal-protocol transport.
- Rust/native defaults remain compatibility-oriented; constrained hosts opt into no-default
  profiles intentionally.

Validation:

```bash
jq . docs/workstreams/wasm-feature-surface-slimming/WORKSTREAM.json
cargo fmt --all --check
git diff --check
```

## Gates

### Always-Preserve Gates

These protect existing full/native/browser behavior:

```bash
cargo fmt --all --check
cargo nextest run -p merman-core
cargo nextest run -p merman-render
cargo nextest run -p merman-bindings-core
cargo run -p xtask -- check-alignment
```

Use narrower task-local gates first, but closeout must explain any skipped full gate.

### Dependency Gates

Before and after each dependency-slimming task, capture:

```bash
cargo tree -p merman-core --target wasm32-unknown-unknown -e normal --depth 3
cargo tree -p merman --no-default-features --target wasm32-unknown-unknown -e normal --depth 3
cargo tree -p merman-wasm --target wasm32-unknown-unknown -e features --depth 4
```

Pure-wasm target acceptance:

- no `wasm-bindgen`;
- no `js-sys`;
- no WebCrypto/random JS imports;
- no JS Date/timezone imports;
- no browser panic hook imports.

The repository now has an initial dependency allowlist gate:

```bash
cargo run -p xtask -- profile-budget check-deps --profile pure-wasm --package merman-core --target wasm32-unknown-unknown --no-default-features --depth 3
cargo run -p xtask -- profile-budget check-deps --profile typst-wasm --package merman-core --target wasm32-unknown-unknown --no-default-features --depth 3
```

Both profiles currently fail on `wasm-bindgen`, `js-sys`, `web-time`, `getrandom`,
`serde-wasm-bindgen`, `wasm-bindgen-futures`, or `console_error_panic_hook` when those crates are
present in the checked tree. Additional blocked crates can be supplied with repeated `--forbid`
flags.

### Import Allowlist Gate

For a Typst plugin or probe wasm:

```bash
wasm-tools print <plugin.wasm> | awk '/^  \(import/{print}'
```

Allowed imports:

```text
(import "typst_env" "wasm_minimal_protocol_write_args_to_buffer" ...)
(import "typst_env" "wasm_minimal_protocol_send_result_to_host" ...)
```

Everything else must be justified as a blocker or removed. In particular, these are failures:

- `__wbindgen_placeholder__`
- `__wbindgen_externref_xform__`
- `js`
- `wasi_snapshot_preview1`
- WebCrypto/getRandomValues
- JS Date/timezone/performance

The repository now has an initial checked gate for this:

```bash
cargo run -p xtask -- profile-budget check-wasm --profile typst-wasm --wasm <plugin.wasm>
cargo run -p xtask -- profile-budget check-imports --profile pure-wasm --wat-file <wasm-tools-print.wat>
cargo run -p xtask -- typst-plugin-smoke --wasm <plugin.wasm>
```

`typst-wasm` allows only the two wasm-minimal-protocol `typst_env` imports and requires exported
`memory` when using `check-wasm` or `check-exports`. `typst-plugin-smoke` additionally proves that
the artifact can be instantiated by a Typst-compatible `wasmi` host and can return SVG JSON bytes.
`pure-wasm` currently allows no imports.

### Export Gate

For a Typst plugin or probe wasm:

```bash
wasm-tools print <plugin.wasm> | awk '/^  \(export/{print}'
```

Required:

- exported `memory`;
- one or more user-facing plugin functions with Typst-compatible signatures.

The `xtask profile-budget check-wasm --profile typst-wasm --wasm <plugin.wasm>` gate enforces the
exported `memory` requirement. Function signature checking remains a follow-up.

### Size Gate

Do not set a hard budget until WFS-020 produces repeatable measurements. Initial observed
baselines:

| Profile | Size | Imports | Notes |
| --- | ---: | ---: | --- |
| parser-only probe | ~4.3 MB | 12 | Not Typst-compatible. |
| `merman-wasm` default `wasm-size` | ~8.5 MB | 49 | Browser package, render + ascii. |
| `merman-wasm --no-default-features` `wasm-size` | ~2.7 MB | 25 | Browser transport only, still JS-bound. |

Proposed acceptance direction:

- pure parser probe should be materially smaller than the current 4.3 MB and have no JS imports;
- browser slim/core package should be documented separately from full render package;
- Typst SVG subset size should be budgeted after admitted families and rough/hand-drawn support are
  decided.
